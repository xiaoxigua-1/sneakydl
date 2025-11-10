use std::sync::Arc;

use tokio::sync::watch;

use crate::result::{Result, SneakydlError};

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending,
    Downloading { downloaded: u64 },
    Paused,
    Completed { total_bytes: u64 },
    Failed,
}

#[derive(Debug)]
pub struct TaskStatusMonitor {
    tx: Arc<watch::Sender<TaskStatus>>,
    rx: watch::Receiver<TaskStatus>,
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
    pub downloaded_bytes: u64,
}

impl Default for TaskStatusMonitor {
    fn default() -> Self {
        let (tx, rx) = watch::channel(TaskStatus::Pending);

        Self {
            tx: Arc::new(tx),
            rx,
        }
    }
}

impl TaskStatusMonitor {
    pub fn sender(&self) -> Arc<watch::Sender<TaskStatus>> {
        self.tx.clone()
    }

    pub async fn wait_for_change(&mut self) -> Result<TaskStatus> {
        self.rx
            .changed()
            .await
            .map_err(|_| SneakydlError::TaskStatusRecvFailed)?;

        Ok(self.rx.borrow().clone())
    }
}

impl TaskControl {
    pub fn new(inner: Arc<watch::Sender<ControlCommand>>) -> Self {
        Self { inner }
    }

    fn send(&self, value: ControlCommand) -> Result<()> {
        self.inner
            .send(value)
            .map_err(SneakydlError::TaskControlCommandSendFailed)
    }

    pub fn start(&self) -> Result<()> {
        self.send(ControlCommand::Start)
    }

    pub fn pause(&self) -> Result<()> {
        self.send(ControlCommand::Pause)
    }
}

impl TaskRuntime {
    pub fn new(status_tx: Arc<watch::Sender<TaskStatus>>) -> Self {
        let (control_tx, control_rx) = watch::channel(ControlCommand::Start);

        Self {
            control_rx,
            control_tx: Arc::new(control_tx),
            status_tx,
            downloaded_bytes: 0,
        }
    }

    pub fn update_status(&self, status: TaskStatus) -> Result<()> {
        self.status_tx
            .send(status)
            .map_err(SneakydlError::TaskStatusSendFailed)
    }

    pub fn add_downloaded(&mut self, downloaded: u64) -> Result<()> {
        self.downloaded_bytes += downloaded;
        self.update_status(TaskStatus::Downloading { downloaded })
    }

    pub fn mark_completed(&self) -> Result<()> {
        self.update_status(TaskStatus::Completed {
            total_bytes: self.downloaded_bytes,
        })
    }
}
