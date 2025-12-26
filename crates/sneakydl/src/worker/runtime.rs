use crate::{
    net::HttpClient,
    storage::{Storage, monitor::StorageMonitor, worker::StorageWorker},
    task::{
        Task,
        runtime::{TaskControl, TaskStatusMonitor},
    },
};

#[derive(Debug)]
pub(crate) struct DownloadWorkerRuntime<C: HttpClient, S: Storage> {
    pub tasks: Vec<Task<C>>,
    pub storage_worker: StorageWorker<S>,
    pub status_monitor: Option<TaskStatusMonitor>,
    pub task_controls: Vec<TaskControl>,
    pub storage_monitor: Option<StorageMonitor>,
}
