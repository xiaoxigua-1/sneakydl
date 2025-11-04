use bytes::Bytes;
use futures_util::StreamExt;
use tokio::sync::{
    mpsc,
    watch::{Receiver, Sender, channel},
};
use uuid::Uuid;

use std::{mem::take, ops::Range, pin::pin, sync::Arc};

use crate::{
    net::{HttpClient, RequestMetadata},
    result::{Result, SneakydlError},
    storage::{StorageNotifier, WriteRequest},
};

#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub task_id: u64,
    pub download_id: Uuid,
    pub url: &'static str,
    pub request_metadata: RequestMetadata,
    /// half-open byte range [start, end). end == start => unknown/streaming
    pub range: Range<u64>,
    pub max_retries: u32,
}

#[derive(Debug, Clone)]
pub struct TaskRuntime {
    pub status: (Sender<TaskStatus>, Receiver<TaskStatus>),
    pub download_bytes: u64,
    pub completed_bytes: u64,
}

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed,
}

#[derive(Debug)]
pub struct Task<C: HttpClient> {
    http: Arc<C>,
    storage_request_tx: Arc<mpsc::Sender<WriteRequest>>,
    metadata: TaskMetadata,
    runtime: TaskRuntime,
}

impl TaskMetadata {
    pub fn new(
        download_id: Uuid,
        task_id: u64,
        url: &'static str,
        request_metadata: RequestMetadata,
        range: Range<u64>,
        max_retries: u32,
    ) -> Self {
        Self {
            download_id,
            task_id,
            url,
            request_metadata,
            range,
            max_retries,
        }
    }
}

impl<C: HttpClient> Task<C> {
    pub fn new(
        http: Arc<C>,
        storage_request_tx: Arc<mpsc::Sender<WriteRequest>>,
        metadata: TaskMetadata,
    ) -> Self {
        Self {
            http,
            storage_request_tx,
            metadata,
            runtime: TaskRuntime {
                status: channel(TaskStatus::Pending),
                download_bytes: 0,
                completed_bytes: 0,
            },
        }
    }

    pub async fn retry_job(&mut self) -> Result<()> {
        for attempt in 1..(self.metadata.max_retries + 1) {
            match self.job().await {
                Ok(_) => break,
                Err(e) => {
                    self.update_status(TaskStatus::Failed)?;
                    if attempt == self.metadata.max_retries {
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn job(&mut self) -> Result<()> {
        self.update_status(TaskStatus::Downloading)?;

        let metadata = self.metadata.clone();
        let mut bytes: Vec<Bytes> = vec![];
        let mut total_bytes_size: u64 = 0;
        let mut stream = pin!(
            self.http
                .get_range(
                    metadata.url,
                    metadata.request_metadata,
                    Some(metadata.range),
                )
                .await
                .map_err(SneakydlError::RequestError)?
        );

        while let Some(item) = stream.next().await {
            let status_receiver = self.runtime.status.clone().1;
            match *status_receiver.borrow() {
                TaskStatus::Downloading => {
                    let item = item.map_err(SneakydlError::RequestError)?;
                    let item_len = item.len() as u64;

                    total_bytes_size += item_len;
                    bytes.push(item);

                    if total_bytes_size > 3000 {
                        // write storage
                        let mut storage_notify = StorageNotifier::new(
                            self.storage_request_tx.clone(),
                            self.runtime.completed_bytes,
                            take(&mut bytes),
                        );

                        storage_notify.send().await?;
                        storage_notify.wait_done().await?;

                        self.runtime.completed_bytes += total_bytes_size;
                        total_bytes_size = 0;
                    }
                    self.runtime.download_bytes += item_len;
                }
                _ => {
                    self.runtime
                        .status
                        .1
                        .changed()
                        .await
                        .map_err(|_| SneakydlError::TaskUpdateStatusRecvFailed)?;
                }
            }
        }

        self.update_status(TaskStatus::Completed)
    }

    fn update_status(&self, status: TaskStatus) -> Result<()> {
        self.runtime
            .status
            .0
            .send(status)
            .map_err(SneakydlError::TaskUpdateStatusSendFailed)
    }

    pub fn pause(&self) -> Result<()> {
        self.update_status(TaskStatus::Paused)
    }

    pub fn start(&self) -> Result<()> {
        self.update_status(TaskStatus::Downloading)
    }
}
