#[cfg(feature = "reqwest")]
pub mod reqwest_client;

use std::{collections::HashMap, ops::Range};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;

#[derive(Debug, Clone)]
pub struct HeadResponse {
    pub accept_ranges: bool,
    pub content_length: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RequestMetadata {
    pub method: &'static str,
    pub headers: HashMap<String, String>,
}

impl RequestMetadata {
    pub fn new(method: &'static str, headers: HashMap<String, String>) -> Self {
        Self { method, headers }
    }
}

#[async_trait]
pub trait HttpClient: Send + Sync + 'static {
    type Iter: Stream<Item = anyhow::Result<Bytes>>;

    async fn head(&self, url: &str) -> anyhow::Result<HeadResponse>;

    async fn get_range(
        &self,
        url: &str,
        metadata: RequestMetadata,
        range: Option<Range<u64>>,
    ) -> anyhow::Result<Self::Iter>;
}
