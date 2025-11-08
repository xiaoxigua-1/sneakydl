use std::sync::Arc;

use tokio::sync::watch;

use crate::result::{Result, SneakydlError};

#[derive(Debug)]
pub enum TaskStatus {
    Pending,
    Downloading { download_bytes: u64 },
    Paused,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub enum ControlCommand {
    Start,
    Pause,
}

#[derive(Debug, Clone)]
pub struct TaskControl {
    inner: Arc<watch::Sender<ControlCommand>>,
}

#[derive(Debug)]
pub(crate) struct TaskRuntime {
    pub control_tx: Arc<watch::Sender<ControlCommand>>,
    pub control_rx: watch::Receiver<ControlCommand>,
    pub status_tx: Arc<watch::Sender<TaskStatus>>,
    pub download_bytes: u64,
}

impl TaskControl {
    pub fn new(inner: Arc<watch::Sender<ControlCommand>>) -> Self {
        Self { inner }
    }

    pub fn start(&self) -> Result<()> {
        self.inner
            .send(ControlCommand::Start)
            .map_err(SneakydlError::TaskUpdateControlCommandSendFailed)
    }

    pub fn pause(&self) -> Result<()> {
        self.inner
            .send(ControlCommand::Pause)
            .map_err(SneakydlError::TaskUpdateControlCommandSendFailed)
    }
}

impl TaskRuntime {
    pub fn new(status_tx: Arc<watch::Sender<TaskStatus>>) -> Self {
        let (control_tx, control_rx) = watch::channel(ControlCommand::Start);

        Self {
            control_rx,
            control_tx: Arc::new(control_tx),
            status_tx,
            download_bytes: 0,
        }
    }

    pub fn update_status(&self, status: TaskStatus) -> Result<()> {
        self.status_tx
            .send(status)
            .map_err(SneakydlError::TaskUpdateStatusSendFailed)
    }

    pub fn update_downloading(&mut self, add_size: u64) -> Result<()> {
        self.download_bytes += add_size;
        self.update_status(TaskStatus::Downloading {
            download_bytes: self.download_bytes,
        })
    }
}
