pub mod metadata;
pub mod runtime;

use bytes::Bytes;
use log::{debug, error, trace};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_stream::StreamExt;

use std::{mem::take, pin::pin, sync::Arc};

use crate::{
    net::HttpClient,
    result::{Result, SneakydlError},
    storage::{StorageNotifier, StorageWriter},
    task::{
        metadata::TaskMetadata,
        runtime::{ControlCommand, TaskControl, TaskRuntime, TaskStatus},
    },
};

#[derive(Debug)]
pub struct Task<C: HttpClient> {
    http: Arc<C>,
    storage_writer: StorageWriter,
    metadata: TaskMetadata,
    runtime: TaskRuntime,
}

impl<C: HttpClient> Task<C> {
    pub fn new(
        http: Arc<C>,
        storage_writer: StorageWriter,
        status_tx: Arc<mpsc::Sender<TaskStatus>>,
        metadata: TaskMetadata,
    ) -> Self {
        Self {
            http,
            storage_writer,
            metadata,
            runtime: TaskRuntime::new(status_tx),
        }
    }

    pub async fn run(mut self) -> Result<()> {
        for attempt in 1..(self.metadata.max_retries + 1) {
            match self.execute_once().await {
                Ok(_) => break,
                Err(e) => {
                    error!(
                        "Download [{}] - Task [{}] attempt {}/{} failed: {:?}",
                        self.metadata.download_id,
                        self.metadata.task_id,
                        attempt,
                        self.metadata.max_retries,
                        e
                    );
                    if attempt == self.metadata.max_retries {
                        self.runtime
                            .update_status(TaskStatus::Failed {
                                task_id: self.metadata.task_id,
                                download_id: self.metadata.download_id,
                            })
                            .await?;

                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute_once(&mut self) -> Result<()> {
        let metadata = self.metadata.clone();
        let download_id = metadata.download_id;
        let task_id = metadata.task_id;

        debug!(
            "Download [{}] - Task [{}] starting download",
            download_id, task_id
        );

        let mut bytes: Vec<Bytes> = vec![];
        let mut buffet_bytes_size: u64 = 0;
        let mut last_wirete_bytes_size: u64 = 0;
        let mut stream = pin!(
            self.http
                .send_request(metadata.url, metadata.request_metadata, metadata.range,)
                .await
                .map_err(SneakydlError::RequestError)?
        );
        let mut storage_notifys: Vec<JoinHandle<Result<()>>> = vec![];

        debug!(
            "Download [{}] - Task [{}] request sent successfully",
            download_id, task_id
        );

        while let Some(item) = stream.next().await {
            let status = self.runtime.control_rx.borrow().clone();

            match status {
                ControlCommand::Start => {
                    let item = item.map_err(SneakydlError::RequestError)?;
                    let item_len = item.len() as u64;

                    buffet_bytes_size += item_len;
                    bytes.push(item);
                    trace!(
                        "Download [{}] - Task [{}] processing chunk of {} bytes",
                        download_id, task_id, item_len
                    );
                    self.runtime
                        .add_downloaded(download_id, task_id, item_len)
                        .await?;

                    if buffet_bytes_size > self.metadata.write_buffer_limit {
                        // write storage
                        storage_notifys.push(
                            self.create_storage_notify(take(&mut bytes), last_wirete_bytes_size)
                                .await?,
                        );

                        last_wirete_bytes_size += buffet_bytes_size;
                        buffet_bytes_size = 0;
                    }
                }
                _ => {
                    self.runtime
                        .update_status(TaskStatus::Paused {
                            download_id,
                            task_id,
                        })
                        .await?;
                    self.runtime
                        .control_rx
                        .changed()
                        .await
                        .map_err(|_| SneakydlError::TaskStatusRecvFailed)?;
                }
            }
        }

        if buffet_bytes_size != 0 {
            storage_notifys.push(
                self.create_storage_notify(take(&mut bytes), last_wirete_bytes_size)
                    .await?,
            );
        }

        debug!(
            "Download [{}] - Task [{}] WriteRequest sent, awaiting StorageWorker completion",
            download_id, task_id
        );
        for notify in storage_notifys {
            notify
                .await
                .map_err(|_| SneakydlError::StorageWriteResponseRecvFailed)??;
        }

        self.runtime.mark_completed(download_id, task_id).await
    }

    async fn create_storage_notify(
        &self,
        bytes: Vec<Bytes>,
        offset: u64,
    ) -> Result<JoinHandle<Result<()>>> {
        let storage_notify = StorageNotifier::new(
            self.metadata.task_id,
            self.storage_writer.clone(),
            offset,
            bytes,
        );

        trace!(
            "Download [{}] - Task [{}] sending WriteRequest to StorageWorker ({} bytes)",
            self.metadata.download_id, self.metadata.task_id, self.runtime.downloaded_bytes
        );

        Ok(tokio::spawn(storage_notify.send_wait_done()))
    }

    pub fn task_control(&self) -> TaskControl {
        TaskControl::new(self.runtime.control_tx.clone())
    }
}
