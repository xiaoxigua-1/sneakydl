pub mod metadata;
pub mod runtime;

use std::sync::Arc;

use log::{error, trace};
use tokio::sync::Semaphore;

use crate::{
    config::SplitStrategy,
    net::HttpClient,
    result::{Result, SneakydlError},
    storage::{Storage, monitor::StorageMonitor, worker::StorageWorker},
    task::{
        Task,
        metadata::TaskMetadata,
        runtime::{TaskStatus, TaskStatusMonitor},
    },
    worker::{metadata::DownloadMetadata, runtime::DownloadWorkerRuntime},
};

pub enum WorkerStatus {}

pub struct DownloadWorker<C: HttpClient, S: Storage> {
    metadata: DownloadMetadata,
    runtime: DownloadWorkerRuntime<C, S>,
}

impl<C: HttpClient, S: Storage> DownloadWorker<C, S> {
    pub async fn new(http: Arc<C>, storage: Arc<S>, metadata: DownloadMetadata) -> Result<Self> {
        Ok(Self {
            runtime: Self::create_runtime(http, storage, &metadata).await?,
            metadata,
        })
    }

    pub fn subscribe_task_status(&mut self) -> Option<TaskStatusMonitor> {
        self.runtime.status_monitor.take()
    }

    pub fn subscribe_storage_status(&mut self) -> Option<StorageMonitor> {
        self.runtime.storage_monitor.take()
    }

    pub async fn run(self) -> Result<()> {
        let mut task_handles = vec![];

        let semaphore = Arc::new(Semaphore::new(self.metadata.task_concurrency));
        let storage_writer = self.runtime.storage_worker.storage_writer();
        let status_monitor = self.runtime.status_monitor;
        let storage_worker_job =
            tokio::spawn(async move { self.runtime.storage_worker.run().await });

        let status_monitor_job = status_monitor.map(|mut monitor| {
            tokio::spawn(async move {
                while let Some(status) = monitor.recv().await {
                    match status {
                        TaskStatus::Downloading {
                            download_id: _,
                            task_id: _,
                            downloaded,
                        } => {
                            trace!("Downloaded size: {}", downloaded);
                        }
                        TaskStatus::Failed {
                            download_id,
                            task_id,
                        } => {
                            error!("Task {} of download {} failed", task_id, download_id);
                        }
                        _ => {}
                    }
                }
            })
        });

        for task in self.runtime.tasks {
            let sem = semaphore.clone();

            task_handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.map_err(SneakydlError::AcquireError)?;
                let result = task.run().await;

                drop(_permit);
                result
            }));
        }

        for handle in task_handles {
            handle.await.map_err(SneakydlError::JoinError)??;
        }

        storage_writer.close().await?;
        if let Some(status_monitor_job) = status_monitor_job {
            status_monitor_job.abort();
        }
        storage_worker_job.await.map_err(SneakydlError::JoinError)?
    }

    async fn create_runtime(
        http: Arc<C>,
        storage: Arc<S>,
        metadata: &DownloadMetadata,
    ) -> Result<DownloadWorkerRuntime<C, S>> {
        let mut tasks = vec![];
        let status_monitor = TaskStatusMonitor::new(100);
        let mut task_controls = vec![];

        let storage_monitor = StorageMonitor::default();
        let storage_worker = StorageWorker::new(storage.clone(), 100, storage_monitor.sender());
        let storage_writer = storage_worker.storage_writer();
        let task_metadatas = Self::create_task_metadata(http.clone(), metadata).await?;

        for metadata in task_metadatas {
            status_monitor
                .sender()
                .send(TaskStatus::Pending {
                    download_id: metadata.download_id,
                    task_id: metadata.task_id,
                    content_length: metadata.content_length,
                })
                .await
                .map_err(SneakydlError::TaskStatusSendFailed)?;
            let task = Task::new(
                http.clone(),
                storage_writer.clone(),
                status_monitor.sender(),
                metadata,
            );

            task_controls.push(task.task_control());
            tasks.push(task);
        }

        Ok(DownloadWorkerRuntime {
            tasks,
            storage_worker,
            storage_monitor: Some(storage_monitor),
            status_monitor: Some(status_monitor),
            task_controls,
        })
    }

    async fn create_task_metadata(
        http: Arc<C>,
        metadata: &DownloadMetadata,
    ) -> Result<Vec<TaskMetadata>> {
        let header = http
            .head(&metadata.url)
            .await
            .map_err(SneakydlError::RequestError)?;
        let can_split = header.content_length.is_some() && header.accept_ranges;

        let task_metadatas = match metadata.split_strategy {
            SplitStrategy::BySize(chunk_size) if can_split => {
                let total_size = header.content_length.unwrap();
                (0..total_size)
                    .step_by(chunk_size)
                    .enumerate()
                    .map(|(index, start)| {
                        let end = (start + chunk_size as u64).min(total_size);

                        TaskMetadata::new(
                            metadata.id,
                            index,
                            metadata.url.clone(),
                            metadata.request_metadata.clone(),
                        )
                        .range(start..end)
                    })
                    .collect()
            }
            SplitStrategy::ByCount(count) if can_split => {
                let total_size = header.content_length.unwrap();
                (0..count)
                    .enumerate()
                    .map(|(index, i)| {
                        let start = total_size * i as u64 / count as u64;
                        let end = total_size * (i + 1) as u64 / count as u64;

                        TaskMetadata::new(
                            metadata.id,
                            index,
                            metadata.url.clone(),
                            metadata.request_metadata.clone(),
                        )
                        .range(start..end)
                    })
                    .collect()
            }
            _ => {
                vec![
                    TaskMetadata::new(
                        metadata.id,
                        0,
                        metadata.url.clone(),
                        metadata.request_metadata.clone(),
                    )
                    .content_length(header.content_length),
                ]
            }
        };

        Ok(task_metadatas)
    }
}
