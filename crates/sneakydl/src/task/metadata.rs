use url::Url;
use uuid::Uuid;

use std::ops::Range;

#[derive(Debug, Clone)]
pub struct TaskMetadata<T> {
    pub task_id: usize,
    pub download_id: Uuid,
    pub url: Url,
    pub request_metadata: Option<T>,
    /// half-open byte range [start, end). end == start => unknown/streaming
    pub(crate) content_length: Option<u64>,
    pub(crate) range: Option<Range<u64>>,
    pub(crate) max_retries: u32,
    pub(crate) write_buffer_limit: u64,
}

impl<T> TaskMetadata<T> {
    pub fn new(download_id: Uuid, task_id: usize, url: Url, request_metadata: Option<T>) -> Self {
        Self {
            download_id,
            task_id,
            url,
            content_length: None,
            request_metadata,
            range: None,
            max_retries: 1,
            write_buffer_limit: 1024 * 1024,
        }
    }

    pub fn range(mut self, range: Range<u64>) -> Self {
        self.range = Some(range);

        self
    }

    pub fn content_length(mut self, content_length: Option<u64>) -> Self {
        self.content_length = content_length;

        self
    }

    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;

        self
    }

    pub fn write_buffer_limit(mut self, write_buffer_limit: u64) -> Self {
        self.write_buffer_limit = write_buffer_limit;

        self
    }
}
