use std::sync::Arc;

use log::debug;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::result::{Result, SneakydlError};

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending {
        download_id: Uuid,
        task_id: usize,
        content_length: Option<u64>,
    },
    Downloading {
        download_id: Uuid,
        task_id: usize,
        downloaded: u64,
    },
    Paused {
        download_id: Uuid,
        task_id: usize,
    },
    Completed {
        download_id: Uuid,
        task_id: usize,
        total_bytes: u64,
    },
    Failed {
        download_id: Uuid,
        task_id: usize,
    },
}

#[derive(Debug)]
pub struct TaskStatusMonitor {
    tx: Arc<mpsc::Sender<TaskStatus>>,
    rx: mpsc::Receiver<TaskStatus>,
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
    pub status_tx: Arc<mpsc::Sender<TaskStatus>>,
    pub downloaded_bytes: u64,
}

impl TaskStatusMonitor {
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer);

        Self {
            tx: Arc::new(tx),
            rx,
        }
    }
}

impl TaskStatusMonitor {
    pub(crate) fn sender(&self) -> Arc<mpsc::Sender<TaskStatus>> {
        self.tx.clone()
    }

    pub async fn recv(&mut self) -> Option<TaskStatus> {
        self.rx.recv().await
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
    pub fn new(status_tx: Arc<mpsc::Sender<TaskStatus>>) -> Self {
        let (control_tx, control_rx) = watch::channel(ControlCommand::Start);

        Self {
            control_rx,
            control_tx: Arc::new(control_tx),
            status_tx,
            downloaded_bytes: 0,
        }
    }

    pub async fn update_status(&self, status: TaskStatus) -> Result<()> {
        self.status_tx
            .send(status)
            .await
            .map_err(SneakydlError::TaskStatusSendFailed)
    }

    pub async fn add_downloaded(
        &mut self,
        download_id: Uuid,
        task_id: usize,
        downloaded: u64,
    ) -> Result<()> {
        self.downloaded_bytes += downloaded;
        self.update_status(TaskStatus::Downloading {
            download_id,
            task_id,
            downloaded,
        })
        .await
    }

    pub async fn mark_completed(&self, download_id: Uuid, task_id: usize) -> Result<()> {
        debug!(
            "Download [{}] - Task [{}] completed ({} bytes)",
            download_id, task_id, self.downloaded_bytes
        );

        self.update_status(TaskStatus::Completed {
            download_id,
            task_id,
            total_bytes: self.downloaded_bytes,
        })
        .await
    }
}
