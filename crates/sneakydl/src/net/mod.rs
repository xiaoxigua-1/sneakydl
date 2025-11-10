#[cfg(feature = "reqwest-client")]
pub mod reqwest_client;

use std::{collections::HashMap, ops::Range};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use strum_macros::Display;

/// Represents the response of an HTTP `HEAD` request.
///
/// Usually used to fetch metadata of a resource before actual download,
/// such as file size or whether the server supports Range requests.
#[derive(Debug, Clone)]
pub struct HeadResponse {
    /// Indicates whether the server supports `Range` requests.
    /// If true, partial downloads are possible.
    pub accept_ranges: bool,

    /// Total length of the content in bytes.
    /// `None` if the server did not provide `Content-Length`.
    pub content_length: Option<u64>,
}

#[derive(Debug, Clone, Display)]
pub enum RequestMethod {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

/// Metadata for an HTTP request.
///
/// Describes the HTTP method and custom headers.
#[derive(Debug, Clone)]
pub struct RequestMetadata {
    /// HTTP method (e.g., `"GET"`, `"POST"`, `"HEAD"`).
    pub method: RequestMethod,

    /// Request headers as a key-value map.
    /// Example: `("User-Agent", "MyDownloader")`.
    pub headers: HashMap<String, String>,
}

impl RequestMetadata {
    /// Creates a new [`RequestMetadata`].
    ///
    /// # Parameters
    /// - `method`: The HTTP method string (e.g., `"GET"`).
    /// - `headers`: A map of headers for the request.
    ///
    /// # Example
    /// ```
    /// use sneakydl::net::{RequestMetadata, RequestMethod};
    /// use std::collections::HashMap;
    ///
    /// let metadata = RequestMetadata::new(RequestMethod::GET, HashMap::new());
    /// ```
    pub fn new(method: RequestMethod, headers: HashMap<String, String>) -> Self {
        Self { method, headers }
    }
}

/// HTTP client abstraction.
///
/// Defines a set of common HTTP behaviors, allowing different concrete implementations
/// (e.g., `reqwest`, `hyper`). Useful for async downloaders, API clients, etc.
#[async_trait]
pub trait HttpClient: Send + Sync + 'static {
    /// Type of the returned data stream.
    ///
    /// Typically a `Stream` that produces chunks of [`Bytes`] asynchronously.
    /// Example: `impl Stream<Item = anyhow::Result<Bytes>>`
    type Iter: Stream<Item = anyhow::Result<Bytes>> + Send;

    /// Sends an HTTP `HEAD` request to retrieve metadata about the target resource.
    ///
    /// # Parameters
    /// - `url`: The URL to send the request to.
    ///
    /// # Returns
    /// - [`HeadResponse`] containing content length and range support info.
    async fn head(&self, url: &str) -> anyhow::Result<HeadResponse>;

    /// Sends an HTTP request and returns a stream for reading the response data incrementally.
    ///
    /// # Parameters
    /// - `url`: Target URL to download or request.
    /// - `metadata`: HTTP method and headers for the request.
    /// - `range`: Optional byte range for partial downloads.
    ///
    /// # Returns
    /// - A `Stream` producing [`Bytes`] chunks of the response.
    async fn send_request(
        &self,
        url: String,
        metadata: RequestMetadata,
        range: Option<Range<u64>>,
    ) -> anyhow::Result<Self::Iter>;
}
