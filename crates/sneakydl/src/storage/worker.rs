use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{
    result::{Result, SneakydlError},
    storage::{Storage, StorageWriteRequest, StorageWriter},
};

#[derive(Debug)]
pub struct StorageWorker<T: Storage> {
    storage: Arc<T>,
    request_rx: mpsc::Receiver<Option<StorageWriteRequest>>,
    request_tx: Arc<mpsc::Sender<Option<StorageWriteRequest>>>,
    // update_status_rx: watch::Receiver<>
}

impl<T: Storage> StorageWorker<T> {
    pub fn new(storage: Arc<T>, request_buff: usize) -> Self {
        let (request_tx, request_rx) = mpsc::channel(request_buff);

        Self {
            storage,
            request_tx: Arc::new(request_tx),
            request_rx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut dest = self
            .storage
            .create_dest()
            .await
            .map_err(SneakydlError::IoError)?;

        while let Some(req) = self.request_rx.recv().await {
            if let Some(req) = req {
                self.storage
                    .write_at(&mut dest, req.offset, req.data)
                    .await
                    .map_err(SneakydlError::IoError)?;
                req.done_tx
                    .send(())
                    .map_err(|_| SneakydlError::StorageWriteResponseSendFailed)?;
            } else {
                self.request_rx.close();
            }
        }

        Ok(())
    }

    pub fn storage_writer(&self) -> StorageWriter {
        StorageWriter {
            inner: self.request_tx.clone(),
        }
    }
}
