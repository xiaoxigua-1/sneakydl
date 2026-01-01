use crate::{
    net::TransferSource,
    storage::{Storage, monitor::StorageMonitor, worker::StorageWorker},
    task::{
        Task,
        runtime::{TaskControl, TaskStatusMonitor},
    },
};

#[derive(Debug)]
pub(crate) struct DownloadWorkerRuntime<C: TransferSource, S: Storage> {
    pub tasks: Vec<Task<C>>,
    pub storage_worker: StorageWorker<S>,
    pub status_monitor: Option<TaskStatusMonitor>,
    pub task_controls: Vec<TaskControl>,
    pub storage_monitor: Option<StorageMonitor>,
}
