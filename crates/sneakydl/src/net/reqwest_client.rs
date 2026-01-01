use async_trait::async_trait;
use bytes::Bytes;
use futures_core::stream::BoxStream;
use reqwest::{
    Client, Method,
    header::{ACCEPT_RANGES, CONTENT_LENGTH, HeaderMap, RANGE},
};
use tokio_stream::StreamExt;
use url::Url;

use crate::net::TransferSource;

pub struct ReqwestClient {
    client: Client,
}

impl ReqwestClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

/// Metadata for an HTTP request.
///
/// Describes the HTTP method and custom headers.
#[derive(Debug, Clone)]
pub struct ReqwestOptions {
    /// HTTP method (e.g., `GET`, `POST`, `HEAD`).
    pub method: Method,

    /// Request headers as a key-value map.
    /// Example: `("User-Agent", "MyDownloader")`.
    pub headers: HeaderMap,
}

impl ReqwestOptions {
    /// Creates a new [`ReqwestOptions`].
    ///
    /// # Parameters
    /// - `method`: The HTTP method string (e.g., `"GET"`).
    /// - `headers`: A map of headers for the request.
    pub fn new(method: Method, headers: HeaderMap) -> Self {
        Self { method, headers }
    }
}

impl Default for ReqwestOptions {
    fn default() -> Self {
        Self::new(Method::GET, HeaderMap::new())
    }
}

#[async_trait]
impl TransferSource for ReqwestClient {
    type Iter = BoxStream<'static, anyhow::Result<Bytes>>;

    type RequestOptions = ReqwestOptions;

    async fn get_metadata(&self, url: &Url) -> anyhow::Result<super::ResourceMetadata> {
        let response = self.client.head(url.clone()).send().await?;
        let filename = url
            .path_segments()
            .into_iter()
            .flatten()
            .next_back()
            .map(|f| f.to_string());
        let headers = response.headers();

        Ok(super::ResourceMetadata {
            support_range: headers
                .get(ACCEPT_RANGES)
                .map(|v| v == "bytes")
                .unwrap_or(false),
            content_length: headers
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok()),
            filename,
        })
    }

    async fn download_range(
        &self,
        url: Url,
        range: Option<std::ops::Range<u64>>,
        options: Option<Self::RequestOptions>,
    ) -> anyhow::Result<Self::Iter> {
        let options = options.unwrap_or(ReqwestOptions::new(Method::GET, HeaderMap::new()));
        let mut request = self
            .client
            .request(options.method, url)
            .headers(options.headers);

        if let Some(range) = range {
            request = request.header(RANGE, format!("bytes={}-{}", range.start, range.end));
        }

        let response = request.send().await?;

        Ok(Box::pin(
            response.bytes_stream().map(|r| r.map_err(|e| e.into())),
        ))
    }
}
