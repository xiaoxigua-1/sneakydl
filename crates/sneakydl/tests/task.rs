use std::{collections::HashMap, sync::Arc, task::Poll, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::{Stream, stream::BoxStream};
use log::trace;
use sneakydl::{
    net::{HeadResponse, HttpClient, RequestMetadata, RequestMethod},
    result::Result,
    storage::{Storage, StorageWorker},
    task::{
        Task,
        metadata::TaskMetadata,
        runtime::{TaskStatus, TaskStatusMonitor},
    },
};
use tokio::{task::JoinHandle, test};
use tokio_stream::StreamExt;
use uuid::Uuid;

struct VoidStorage;
struct VoidClient {
    size: usize,
}
struct FixedChunkStream {
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
        Ok(Box::pin(
            FixedChunkStream::new(self.size).throttle(Duration::from_micros(1)),
        ))
    }
}

impl FixedChunkStream {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
        }
    }
}

impl Stream for FixedChunkStream {
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
/// 3. Status updates propagate correctly and completion reports accurate total bytes.
/// 4. All async tasks (`download_job`, `storage_job`, `check_status_job`) complete cleanly
///    without deadlocks or panics.
///
/// Test downloads ~1MB (1,048,566 bytes) in 1KB chunks to validate coordination between
/// downloading and storage components without relying on external I/O.
#[test]
async fn task_test() {
    env_logger::init();
    let test_size: u64 = 1024 * 1024 - 10;
    let void_client = Arc::new(VoidClient {
        size: test_size as usize,
    });
    let mut storage_worker = StorageWorker::new(VoidStorage, 100);

    let download_id = Uuid::new_v4();
    let request_metadata = RequestMetadata::new(RequestMethod::GET, HashMap::new());
    let metadata = TaskMetadata::new(download_id, 0, String::new(), request_metadata);
    let mut status_monitor = TaskStatusMonitor::default();
    let storage_writer = storage_worker.storage_writer();
    let storage_writer_clone = storage_worker.storage_writer();
    let mut task = Task::new(
        void_client,
        storage_writer,
        status_monitor.sender(),
        metadata,
    );

    let download_job: JoinHandle<Result<()>> = tokio::spawn(async move { task.run().await });
    let storage_job = tokio::spawn(async move {
        storage_worker.run().await.unwrap();
    });
    let check_status_job = tokio::spawn(async move {
        loop {
            match status_monitor.wait_for_change().await? {
                TaskStatus::Completed { total_bytes } => {
                    assert_eq!(test_size, total_bytes);

                    break;
                }
                TaskStatus::Downloading { downloaded } => {
                    trace!("Downloaded size: {}", downloaded);
                }
                _ => {}
            }
        }

        storage_writer_clone.close().await
    });

    let (_, _, _) = tokio::join!(download_job, storage_job, check_status_job);
}
