pub mod metadata;
pub mod runtime;

use std::sync::Arc;

use tokio::sync::{Semaphore, watch};

use crate::{
    config::SplitStrategy,
    net::HttpClient,
    result::{Result, SneakydlError},
    storage::{Storage, worker::StorageWorker},
    task::{Task, metadata::TaskMetadata, runtime::TaskStatusMonitor},
    worker::{metadata::DownloadMetadata, runtime::DownloadWorkerRuntime},
};

pub enum WorkerStatus {}

pub struct DownloadWorker<C: HttpClient, S: Storage> {
    worker_status: Arc<watch::Sender<WorkerStatus>>,
    metadata: DownloadMetadata,
    runtime: DownloadWorkerRuntime<C, S>,
}

impl<C: HttpClient, S: Storage> DownloadWorker<C, S> {
    pub async fn new(
        http: Arc<C>,
        storage: Arc<S>,
        worker_status: Arc<watch::Sender<WorkerStatus>>,
        metadata: DownloadMetadata,
    ) -> Result<Self> {
        Ok(Self {
            runtime: Self::create_runtime(http, storage, &metadata).await?,
            worker_status,
            metadata,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        let mut task_handles = vec![];

        let semaphore = Arc::new(Semaphore::new(self.metadata.task_concurrency));
        let storage_writer = self.runtime.storage_worker.storage_writer();
        let storage_worker_job =
            tokio::spawn(async move { self.runtime.storage_worker.run().await });

        for mut task in self.runtime.tasks {
            let sem = semaphore.clone();

            self.runtime.task_controls.push(task.task_control());
            task_handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                task.run().await
            }));
        }

        for handle in task_handles {
            handle.await.map_err(SneakydlError::JoinError)??;
        }

        storage_writer.close().await?;
        storage_worker_job.await.map_err(SneakydlError::JoinError)?
    }

    async fn create_runtime(
        http: Arc<C>,
        storage: Arc<S>,
        metadata: &DownloadMetadata,
    ) -> Result<DownloadWorkerRuntime<C, S>> {
        let mut tasks = vec![];
        let mut status_monitors = vec![];
        let mut task_controls = vec![];

        let storage_worker = StorageWorker::new(storage.clone(), 100);
        let storage_writer = storage_worker.storage_writer();
        let task_metadatas = Self::create_task_metadata(http.clone(), metadata).await?;

        for metadata in task_metadatas {
            let status_monitor = TaskStatusMonitor::default();
            let task = Task::new(
                http.clone(),
                storage_writer.clone(),
                status_monitor.sender(),
                metadata,
            );

            task_controls.push(task.task_control());
            tasks.push(task);
            status_monitors.push(status_monitor);
        }

        Ok(DownloadWorkerRuntime {
            tasks,
            storage_worker,
            status_monitors,
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
                vec![TaskMetadata::new(
                    metadata.id,
                    0,
                    metadata.url.clone(),
                    metadata.request_metadata.clone(),
                )]
            }
        };

        Ok(task_metadatas)
    }
}
