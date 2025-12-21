use std::sync::Arc;

use tokio::sync::watch;

use crate::result::{Result, SneakydlError};

#[derive(Debug, Clone)]
pub enum StorageStatus {
    Idle,
    WriteProgress {
        task_id: usize,
        offset: u64,
        bytes_written: u64,
    },
    WriteCompleted {
        task_id: usize,
        offset: u64,
        total_bytes: u64,
    },
}

#[derive(Debug)]
pub struct StorageMonitor {
    rx: watch::Receiver<StorageStatus>,
    tx: Arc<watch::Sender<StorageStatus>>,
}

impl Default for StorageMonitor {
    fn default() -> Self {
        let (tx, rx) = watch::channel(StorageStatus::Idle);

        Self {
            rx,
            tx: Arc::new(tx),
        }
    }
}

impl StorageMonitor {
    pub fn sender(&self) -> Arc<watch::Sender<StorageStatus>> {
        self.tx.clone()
    }

    pub async fn wait_for_change(&mut self) -> Result<StorageStatus> {
        self.rx
            .changed()
            .await
            .map_err(|_| SneakydlError::StorageStatusRecvFailed)?;

        Ok(self.rx.borrow().clone())
    }
}
