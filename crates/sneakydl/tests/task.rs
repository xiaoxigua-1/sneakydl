use std::{collections::HashMap, sync::Arc, task::Poll};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::{Stream, stream::BoxStream};
use sneakydl::{
    net::{HeadResponse, HttpClient, RequestMetadata},
    storage::{Storage, StorageWorker},
    task::{Task, TaskMetadata},
};
use tokio::test;
use uuid::Uuid;

struct VoidStorage;
struct VoidClient;
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

    async fn get_range(
        &self,
        _: &str,
        _: RequestMetadata,
        _: Option<std::ops::Range<u64>>,
    ) -> anyhow::Result<Self::Iter> {
        Ok(Box::pin(VoidStream::default()))
    }
}

impl Default for VoidStream {
    fn default() -> Self {
        Self {
            data: vec![0u8; 1024 * 1024],
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

/*
 * Simplify single-Task test with tokio::select!
 *
 * In this single-Task test, we use `tokio::select!` to simplify the code:
 *  - `select!` works here because each Task will only complete after all its writes have been fully processed.
 *  - When the Task finishes, it guarantees that the StorageWorker has completed all write operations.
 *  - If the StorageWorker future is dropped (due to select!), it's safe since all necessary writes for this Task are already done.
 * This keeps the test simple while preserving correctness.
 */
#[test]
async fn task_test() {
    env_logger::init();
    let void_client = Arc::new(VoidClient);
    let mut storage_worker = StorageWorker::new(VoidStorage, 100);

    let download_id = Uuid::new_v4();
    // Example url
    let url = "http://localhost";
    let request_metadata = RequestMetadata::new("GET", HashMap::new());
    let metadata = TaskMetadata {
        task_id: 0,
        download_id,
        url,
        request_metadata,
        range: None,
        max_retries: 1,
    };
    let request_tx = storage_worker.get_request_tx();
    let mut task = Task::new(void_client, request_tx, metadata);

    let download_job = tokio::spawn(async move { task.job().await });
    let storage_job = tokio::spawn(async move {
        storage_worker.run().await.unwrap();
    });

    tokio::select! {
        Ok(r) = download_job => {
            assert!(r.is_ok());
        }
        _ = storage_job => {}
    }
}
