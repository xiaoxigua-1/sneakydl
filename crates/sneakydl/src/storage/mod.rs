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

pub struct WriteRequest {
    pub task_id: u32,
    pub offset: u64,
    pub data: Vec<Bytes>,
    pub done_tx: oneshot::Sender<()>,
}

pub struct StorageNotifier {
    write_request: Option<WriteRequest>,
    request_tx: Arc<mpsc::Sender<Option<WriteRequest>>>,
    done_rx: oneshot::Receiver<()>,
}

pub struct StorageWorker<T: Storage> {
    storage: T,
    request_rx: mpsc::Receiver<Option<WriteRequest>>,
    request_tx: Arc<mpsc::Sender<Option<WriteRequest>>>,
    // update_status_rx: watch::Receiver<>
}

impl StorageNotifier {
    pub fn new(
        task_id: u32,
        request_tx: Arc<mpsc::Sender<Option<WriteRequest>>>,
        offset: u64,
        data: Vec<Bytes>,
    ) -> Self {
        let (done_tx, done_rx) = oneshot::channel();

        Self {
            request_tx,
            write_request: Some(WriteRequest {
                task_id,
                offset,
                data,
                done_tx,
            }),
            done_rx,
        }
    }

    pub async fn send_wait_done(self) -> Result<()> {
        self.request_tx
            .send(self.write_request)
            .await
            .map_err(SneakydlError::WriteRequestSendFailed)?;
        self.done_rx
            .await
            .map_err(|_| SneakydlError::WriteResponseReceiveFailed)
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

    pub fn get_request_tx(&self) -> Arc<mpsc::Sender<Option<WriteRequest>>> {
        self.request_tx.clone()
    }
}
