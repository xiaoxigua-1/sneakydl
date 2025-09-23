use reqwest::{
    Method,
    header::{HeaderMap, HeaderValue, IntoHeaderName},
};
use tokio::sync::Mutex;

use std::{fs::File, ops::Range, sync::Arc};

#[derive(Debug)]
pub struct DownloadTask {
    pub url: String,
    pub range: Option<Range<u64>>,
    pub file: Arc<Mutex<File>>,
    pub headers: HeaderMap,
    pub request_method: Method,
}

pub struct DownloadTaskBuilder {
    inner: DownloadTask,
}

impl DownloadTask {
    pub fn builder(url: String, file: Arc<Mutex<File>>) -> DownloadTaskBuilder {
        DownloadTaskBuilder {
            inner: DownloadTask {
                url,
                file,
                range: None,
                headers: HeaderMap::new(),
                request_method: Method::GET,
            },
        }
    }
}

impl DownloadTaskBuilder {
    pub fn range(mut self, range: Range<u64>) -> Self {
        self.inner.range = Some(range);

        self
    }

    pub fn header<K>(mut self, key: K, val: HeaderValue) -> Self
    where
        K: IntoHeaderName,
    {
        self.inner.headers.insert(key, val);

        self
    }

    pub fn request_method(mut self, method: Method) -> Self {
        self.inner.request_method = method;

        self
    }

    pub fn build(self) -> DownloadTask {
        self.inner
    }
}
