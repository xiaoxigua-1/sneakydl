#[cfg(feature = "tokio-storage")]
pub mod tokio_file;
pub mod worker;

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

use crate::result::{Result, SneakydlError};

#[async_trait]
pub trait Storage: Send + Sync + 'static {
    type Dest: Send;

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
    pub task_id: usize,
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

impl StorageNotifier {
    pub fn new(
        task_id: usize,
        storage_writer: StorageWriter,
        offset: u64,
        data: Vec<Bytes>,
    ) -> Self {
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
            .map_err(|_| SneakydlError::StorageWriteRequestRecvFailed)
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
            .map_err(SneakydlError::StorageWriteRequestSendFailed)
    }
}
