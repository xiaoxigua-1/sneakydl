pub mod tokio_file;

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::result::{Result, SneakydlError};

#[async_trait]
pub trait Storage: Send + Sync + 'static {
    type Dest;

    async fn create_dest(&self) -> anyhow::Result<Self::Dest>;

    async fn write_at(
        &self,
        dest: &mut Self::Dest,
        offset: u64,
        data: Vec<Bytes>,
    ) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct StorageWriteRequest {
    pub task_id: u32,
    pub offset: u64,
    pub data: Vec<Bytes>,
    pub done_tx: oneshot::Sender<()>,
}

#[derive(Debug, Clone)]
pub struct StorageWriter {
    inner: Arc<mpsc::Sender<Option<StorageWriteRequest>>>,
}

#[derive(Debug)]
pub struct StorageNotifier {
    write_request: Option<StorageWriteRequest>,
    storage_writer: StorageWriter,
    done_rx: oneshot::Receiver<()>,
}

#[derive(Debug)]
pub struct StorageWorker<T: Storage> {
    storage: T,
    request_rx: mpsc::Receiver<Option<StorageWriteRequest>>,
    request_tx: Arc<mpsc::Sender<Option<StorageWriteRequest>>>,
    // update_status_rx: watch::Receiver<>
}

impl StorageNotifier {
    pub fn new(task_id: u32, storage_writer: StorageWriter, offset: u64, data: Vec<Bytes>) -> Self {
        let (done_tx, done_rx) = oneshot::channel();

        Self {
            storage_writer,
            write_request: Some(StorageWriteRequest {
                task_id,
                offset,
                data,
                done_tx,
            }),
            done_rx,
        }
    }

    pub async fn send_wait_done(self) -> Result<()> {
        self.storage_writer.send(self.write_request).await?;
        self.done_rx
            .await
            .map_err(|_| SneakydlError::WriteResponseReceiveFailed)
    }
}

impl StorageWriter {
    pub async fn close(&self) -> Result<()> {
        self.send(None).await
    }

    pub async fn send(&self, value: Option<StorageWriteRequest>) -> Result<()> {
        self.inner
            .send(value)
            .await
            .map_err(SneakydlError::WriteRequestSendFailed)
    }
}

impl<T: Storage> StorageWorker<T> {
    pub fn new(storage: T, request_buff: usize) -> Self {
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
                    .map_err(|_| SneakydlError::WriteResponseSendFailed)?;
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
