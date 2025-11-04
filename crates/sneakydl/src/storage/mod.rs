pub mod tokio_file;

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::{Notify, mpsc, oneshot};

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
    pub offset: u64,
    pub data: Vec<Bytes>,
    pub done_tx: oneshot::Sender<()>,
}

pub struct StorageNotifier {
    write_request: Option<WriteRequest>,
    request_tx: Arc<mpsc::Sender<WriteRequest>>,
    done_rx: oneshot::Receiver<()>,
}

pub struct StorageWorker<T: Storage> {
    storage: T,
    done: Notify,
    request_rx: mpsc::Receiver<WriteRequest>,
    request_tx: Arc<mpsc::Sender<WriteRequest>>,
}

impl StorageNotifier {
    pub fn new(request_tx: Arc<mpsc::Sender<WriteRequest>>, offset: u64, data: Vec<Bytes>) -> Self {
        let (done_tx, done_rx) = oneshot::channel();

        Self {
            request_tx,
            write_request: Some(WriteRequest {
                offset,
                data,
                done_tx,
            }),
            done_rx,
        }
    }

    pub async fn send(&mut self) -> Result<()> {
        if let Some(req) = self.write_request.take() {
            self.request_tx
                .send(req)
                .await
                .map_err(SneakydlError::StorageRequestSendFailed)
        } else {
            Err(SneakydlError::StorageNoRequestFailed)
        }
    }

    pub async fn wait_done(self) -> Result<()> {
        self.done_rx
            .await
            .map_err(|_| SneakydlError::NotifyRecvFailed)
    }
}

impl<T: Storage> StorageWorker<T> {
    pub fn new(storage: T, request_buff: usize) -> Self {
        let (request_tx, request_rx) = mpsc::channel(request_buff);
        let done = Notify::new();

        Self {
            storage,
            done,
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

        loop {
            tokio::select! {
                Some(req) = self.request_rx.recv() => {
                    self.storage
                        .write_at(&mut dest, req.offset, req.data)
                        .await
                        .map_err(SneakydlError::IoError)?;
                    req.done_tx
                        .send(())
                        .map_err(|_| SneakydlError::NotifySendFailed)?;
                }
                _ = self.done.notified() => {
                    break;
                }
            }
        }

        Ok(())
    }

    pub async fn done(&self) {
        self.done.notify_one();
    }

    pub fn get_request_tx(&self) -> Arc<mpsc::Sender<WriteRequest>> {
        self.request_tx.clone()
    }
}
