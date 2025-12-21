use std::sync::Arc;

use tokio::sync::{mpsc, watch};

use crate::{
    result::{Result, SneakydlError},
    storage::{Storage, StorageWriteRequest, StorageWriter, monitor::StorageStatus},
};

#[derive(Debug)]
pub struct StorageWorker<T: Storage> {
    storage: Arc<T>,
    request_rx: mpsc::Receiver<Option<StorageWriteRequest>>,
    request_tx: Arc<mpsc::Sender<Option<StorageWriteRequest>>>,
    status_monitor_tx: Arc<watch::Sender<StorageStatus>>,
}

impl<T: Storage> StorageWorker<T> {
    pub fn new(
        storage: Arc<T>,
        request_buff: usize,
        status_monitor_tx: Arc<watch::Sender<StorageStatus>>,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel(request_buff);

        Self {
            storage,
            request_tx: Arc::new(request_tx),
            request_rx,
            status_monitor_tx,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut dest = self
            .storage
            .create_dest()
            .await
            .map_err(SneakydlError::IoError)?;

        while let Some(req) = self.request_rx.recv().await {
            if let Some(req) = req {
                let total_bytes = req.data.iter().map(|b| b.len() as u64).sum();

                self.update_status(StorageStatus::WriteProgress {
                    task_id: req.task_id,
                    offset: req.offset,
                    bytes_written: total_bytes,
                })?;

                self.storage
                    .write_at(&mut dest, req.offset, req.data)
                    .await
                    .map_err(SneakydlError::IoError)?;
                req.done_tx
                    .send(())
                    .map_err(|_| SneakydlError::StorageWriteResponseSendFailed)?;

                self.update_status(StorageStatus::WriteCompleted {
                    task_id: req.task_id,
                    offset: req.offset,
                    total_bytes,
                })?;
            } else {
                self.request_rx.close();
            }
        }

        Ok(())
    }

    pub fn update_status(&self, value: StorageStatus) -> Result<()> {
        self.status_monitor_tx
            .send(value)
            .map_err(SneakydlError::StorageStatusSendFailed)
    }

    pub fn storage_writer(&self) -> StorageWriter {
        StorageWriter {
            inner: self.request_tx.clone(),
        }
    }
}
