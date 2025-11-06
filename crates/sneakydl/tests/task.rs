use std::{collections::HashMap, sync::Arc, task::Poll};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::{Stream, stream::BoxStream};
use sneakydl::{
    net::{HeadResponse, HttpClient, RequestMetadata, RequestMethod},
    result::{Result, SneakydlError},
    storage::{Storage, StorageWorker},
    task::{Task, TaskMetadata},
};
use tokio::{task::JoinHandle, test};
use uuid::Uuid;

struct VoidStorage;
struct VoidClient {
    size: usize,
}
struct VoidStream {
    data: Vec<u8>,
}

#[async_trait]
impl Storage for VoidStorage {
    type Dest = ();

    async fn create_dest(&self) -> anyhow::Result<Self::Dest> {
        Ok(())
    }

    async fn write_at(
        &self,
        _: &mut Self::Dest,
        _: u64,
        _: Vec<bytes::Bytes>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl HttpClient for VoidClient {
    type Iter = BoxStream<'static, anyhow::Result<Bytes>>;

    async fn head(&self, _: &str) -> anyhow::Result<HeadResponse> {
        Ok(HeadResponse {
            accept_ranges: false,
            content_length: Some(0),
        })
    }

    async fn send_request(
        &self,
        _: String,
        _: RequestMetadata,
        _: Option<std::ops::Range<u64>>,
    ) -> anyhow::Result<Self::Iter> {
        Ok(Box::pin(VoidStream::new(self.size)))
    }
}

impl VoidStream {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
        }
    }
}

impl Stream for VoidStream {
    type Item = anyhow::Result<Bytes>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let last_split_size = self.data.len().saturating_sub(1024);
        let data = self.get_mut().data.split_off(last_split_size);

        Poll::Ready((!data.is_empty()).then(|| Ok(Bytes::from(data))))
    }
}

/// Integration-like test for verifying the end-to-end workflow of a download task.
///
/// This test sets up a simulated environment using:
/// - `VoidClient`: a mock HTTP client that generates fake byte data without real networking.
/// - `VoidStorage`: a mock storage backend that discards all writes.
/// - `StorageWorker` and `Task`: the real logic under test, communicating via async channels.
///
/// The test ensures that:
/// 1. A `Task` can stream data from the mock client and send write requests correctly.
/// 2. The `StorageWorker` can receive and process these requests without errors.
/// 3. Both async tasks (`download_job` and `storage_job`) complete cleanly without deadlocks or panics.
///
/// It effectively validates the coordination between downloading and storage components
/// without relying on external I/O.
#[test]
async fn task_test() {
    env_logger::init();
    let void_client = Arc::new(VoidClient {
        size: 1024 * 1024 - 10,
    });
    let mut storage_worker = StorageWorker::new(VoidStorage, 100);

    let download_id = Uuid::new_v4();
    let request_metadata = RequestMetadata::new(RequestMethod::GET, HashMap::new());
    let metadata = TaskMetadata::new(download_id, 0, String::new(), request_metadata);
    let request_tx = storage_worker.get_request_tx();
    let request_tx_clone = storage_worker.get_request_tx();
    let mut task = Task::new(void_client, request_tx, metadata);

    let download_job: JoinHandle<Result<()>> = tokio::spawn(async move {
        task.job().await?;
        request_tx_clone
            .send(None)
            .await
            .map_err(SneakydlError::StorageRequestSendFailed)
    });
    let storage_job = tokio::spawn(async move {
        storage_worker.run().await.unwrap();
    });

    let (_, _) = tokio::join!(download_job, storage_job);
}
