use std::{collections::HashMap, sync::Arc, task::Poll, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::{Stream, stream::BoxStream};
use sneakydl::{
    config::Config,
    net::{HeadResponse, HttpClient, RequestMetadata, RequestMethod},
    storage::Storage,
    worker::{DownloadWorker, metadata::DownloadMetadata},
};
use tokio::test;
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

#[test]
async fn task_worker() {
    env_logger::init();
    let test_size: u64 = 1024 * 1024 - 10;
    let config = Config::default();
    let void_client = Arc::new(VoidClient {
        size: test_size as usize,
    });

    let download_worker_metadata = DownloadMetadata::new(
        Uuid::default(),
        "".to_string(),
        RequestMetadata::new(RequestMethod::GET, HashMap::new()),
        config,
    );
    let worker = DownloadWorker::new(void_client, Arc::new(VoidStorage), download_worker_metadata)
        .await
        .unwrap();

    let _ = worker.run().await;
}
