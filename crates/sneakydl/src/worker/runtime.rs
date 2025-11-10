use crate::{
    net::HttpClient,
    storage::{Storage, worker::StorageWorker},
    task::{
        Task,
        runtime::{TaskControl, TaskStatusMonitor},
    },
};

#[derive(Debug)]
pub(crate) struct DownloadWorkerRuntime<C: HttpClient, S: Storage> {
    pub tasks: Vec<Task<C>>,
    pub storage_worker: StorageWorker<S>,
    pub status_monitors: Vec<TaskStatusMonitor>,
    pub task_controls: Vec<TaskControl>,
}
