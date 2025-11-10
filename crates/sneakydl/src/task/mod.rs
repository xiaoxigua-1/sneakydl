pub mod metadata;
pub mod runtime;

use bytes::Bytes;
use log::{debug, trace};
use tokio::{sync::watch, task::JoinHandle};
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
        status_tx: Arc<watch::Sender<TaskStatus>>,
        metadata: TaskMetadata,
    ) -> Self {
        Self {
            http,
            storage_writer,
            metadata,
            runtime: TaskRuntime::new(status_tx),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        for attempt in 1..(self.metadata.max_retries + 1) {
            match self.execute_once().await {
                Ok(_) => break,
                Err(e) => {
                    if attempt == self.metadata.max_retries {
                        self.runtime.update_status(TaskStatus::Failed)?;

                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute_once(&mut self) -> Result<()> {
        debug!(
            "Download [{}] - Task [{}] starting download",
            self.metadata.download_id, self.metadata.task_id
        );

        let metadata = self.metadata.clone();
        let mut bytes: Vec<Bytes> = vec![];
        let mut total_bytes_size: u64 = 0;
        let mut stream = pin!(
            self.http
                .send_request(metadata.url, metadata.request_metadata, metadata.range,)
                .await
                .map_err(SneakydlError::RequestError)?
        );
        let mut storage_notifys: Vec<JoinHandle<Result<()>>> = vec![];

        debug!(
            "Download [{}] - Task [{}] request sent successfully",
            self.metadata.download_id, self.metadata.task_id
        );

        while let Some(item) = stream.next().await {
            let status = self.runtime.control_rx.borrow().clone();

            match status {
                ControlCommand::Start => {
                    let item = item.map_err(SneakydlError::RequestError)?;
                    let item_len = item.len() as u64;

                    total_bytes_size += item_len;
                    bytes.push(item);
                    trace!(
                        "Download [{}] - Task [{}] processing chunk of {} bytes",
                        self.metadata.download_id, self.metadata.task_id, item_len
                    );
                    self.runtime.add_downloaded(item_len)?;

                    if total_bytes_size > self.metadata.write_buffer_limit {
                        // write storage
                        storage_notifys.push(
                            self.create_storage_notify(take(&mut bytes), total_bytes_size)
                                .await?,
                        );

                        total_bytes_size = 0;
                    }
                }
                _ => {
                    self.runtime.update_status(TaskStatus::Paused)?;
                    self.runtime
                        .control_rx
                        .changed()
                        .await
                        .map_err(|_| SneakydlError::TaskStatusRecvFailed)?;
                }
            }
        }

        if total_bytes_size != 0 {
            storage_notifys.push(
                self.create_storage_notify(take(&mut bytes), total_bytes_size)
                    .await?,
            );
        }

        debug!(
            "Download [{}] - Task [{}] WriteRequest sent, awaiting StorageWorker completion",
            self.metadata.download_id, self.metadata.task_id
        );
        for notify in storage_notifys {
            notify
                .await
                .map_err(|_| SneakydlError::StorageWriteResponseRecvFailed)??;
        }

        debug!(
            "Download [{}] - Task [{}] completed ({} bytes)",
            self.metadata.download_id, self.metadata.task_id, self.runtime.downloaded_bytes
        );

        self.runtime.mark_completed()
    }

    async fn create_storage_notify(
        &self,
        bytes: Vec<Bytes>,
        total_bytes_size: u64,
    ) -> Result<JoinHandle<Result<()>>> {
        let storage_notify = StorageNotifier::new(
            self.metadata.task_id,
            self.storage_writer.clone(),
            self.runtime.downloaded_bytes,
            bytes,
        );

        trace!(
            "Download [{}] - Task [{}] sending WriteRequest to StorageWorker ({} bytes)",
            self.metadata.download_id, self.metadata.task_id, total_bytes_size
        );

        Ok(tokio::spawn(storage_notify.send_wait_done()))
    }

    pub fn task_control(&self) -> TaskControl {
        TaskControl::new(self.runtime.control_tx.clone())
    }
}
