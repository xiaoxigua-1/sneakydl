use std::str::FromStr;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{StreamExt, TryStreamExt, stream::BoxStream};
use reqwest::{
    Client, Method,
    header::{ACCEPT_RANGES, CONTENT_LENGTH, RANGE},
};

use crate::net::{HttpClient, RequestMetadata};

pub struct ReqwestClient {
    client: Client,
}

#[async_trait]
impl HttpClient for ReqwestClient {
    type Iter = BoxStream<'static, anyhow::Result<Bytes>>;

    async fn head(&self, url: &str) -> anyhow::Result<super::HeadResponse> {
        let response = self.client.head(url).send().await?;
        let headers = response.headers();

        Ok(super::HeadResponse {
            accept_ranges: headers
                .get(ACCEPT_RANGES)
                .map(|v| v == "bytes")
                .unwrap_or(false),
            content_length: headers
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok()),
        })
    }

    async fn get_range(
        &self,
        url: &str,
        metadata: RequestMetadata,
        range: Option<std::ops::Range<u64>>,
    ) -> anyhow::Result<Self::Iter> {
        let method = Method::from_str(metadata.method)?;
        let mut request = self.client.request(method, url);

        if let Some(range) = range {
            request = request.header(RANGE, format!("bytes={}-{}", range.start, range.end));
        }

        for (k, v) in metadata.headers {
            request = request.header(k, v);
        }

        let response = request.send().await?;

        Ok(response.bytes_stream().map_err(anyhow::Error::from).boxed())
    }
}
