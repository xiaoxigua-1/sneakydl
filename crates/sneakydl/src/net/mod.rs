#[cfg(feature = "reqwest-client")]
pub mod reqwest_client;

use std::{fmt::Debug, ops::Range};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use url::Url;

/// Metadata information about a downloadable resource.
///
/// This struct contains information retrieved from a remote resource
/// that helps determine how to transfer it efficiently.
#[derive(Debug, Clone)]
pub struct ResourceMetadata {
    /// Whether the server supports HTTP range requests.
    ///
    /// If true, the resource can be downloaded in multiple partial requests,
    /// enabling features like resumable downloads and parallel transfers.
    pub support_range: bool,

    /// The total size of the resource in bytes, if available.
    ///
    /// This is typically obtained from the "Content-Length" HTTP header.
    /// None indicates that the size is unknown or not provided by the server.
    pub content_length: Option<u64>,

    /// The suggested filename for the resource, if available.
    ///
    /// This is typically obtained from the "Content-Disposition" HTTP header.
    /// Can be used as a default filename when saving the downloaded resource.
    pub filename: Option<String>,
}

/// A trait for handling resource downloads and metadata retrieval.
///
/// This trait defines the interface for downloading resources from various sources
/// (e.g., HTTP, FTP, S3) with support for range requests and custom options.
///
/// # Associated Types
///
/// - `Iter`: A stream that yields chunks of bytes representing the downloaded data
/// - `RequestOptions`: Configuration options specific to the transfer implementation
#[async_trait]
pub trait TransferSource: Send + Sync + 'static {
    /// A stream type that yields chunks of the downloaded resource.
    ///
    /// Each item is a Result containing either a Bytes chunk or an error.
    /// The stream is required to be Send and compatible with async/await.
    type Iter: Stream<Item = anyhow::Result<Bytes>> + Send;

    /// Configuration options for customizing resource download requests.
    ///
    /// Must be Clone, Debug, Send, and Sync to work across async boundaries.
    type RequestOptions: Clone + Debug + Send + Sync + 'static;

    /// Retrieves metadata information about a remote resource.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the resource to query
    ///
    /// # Returns
    ///
    /// A Result containing the resource's metadata, or an error if the metadata
    /// cannot be retrieved (e.g., network failure, invalid URL, server error).
    async fn get_metadata(&self, url: &Url) -> anyhow::Result<ResourceMetadata>;

    /// Downloads a resource, optionally specifying a byte range.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL of the resource to download
    /// * `range` - Optional byte range to download (e.g., bytes 0-99).
    ///            If None, the entire resource is downloaded.
    ///            Only works if the server supports range requests.
    /// * `options` - Optional transfer-specific configuration options.
    ///              If None, default options are used.
    ///
    /// # Returns
    ///
    /// A Result containing a stream that yields data chunks, or an error if the
    /// download cannot be initiated (e.g., invalid URL, server error, range not supported).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let iter = source.download_range(
    ///     Url::parse("https://example.com/file.bin")?,
    ///     Some(0..1024),  // Download first 1KB
    ///     None,
    /// ).await?;
    /// ```
    async fn download_range(
        &self,
        url: Url,
        range: Option<Range<u64>>,
        options: Option<Self::RequestOptions>,
    ) -> anyhow::Result<Self::Iter>;
}
