use uuid::Uuid;

use std::ops::Range;

use crate::net::RequestMetadata;

#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub task_id: u32,
    pub download_id: Uuid,
    pub url: String,
    pub request_metadata: RequestMetadata,
    /// half-open byte range [start, end). end == start => unknown/streaming
    pub range: Option<Range<u64>>,
    pub max_retries: u32,
    pub write_buffer_limit: u64,
}

impl TaskMetadata {
    pub fn new(
        download_id: Uuid,
        task_id: u32,
        url: String,
        request_metadata: RequestMetadata,
    ) -> Self {
        Self {
            download_id,
            task_id,
            url,
            request_metadata,
            range: None,
            max_retries: 1,
            write_buffer_limit: 1024 * 1024,
        }
    }

    pub fn range(&mut self, range: Range<u64>) {
        self.range = Some(range);
    }

    pub fn max_retries(&mut self, max_retries: u32) {
        self.max_retries = max_retries;
    }

    pub fn write_buffer_limit(&mut self, write_buffer_limit: u64) {
        self.write_buffer_limit = write_buffer_limit;
    }
}
