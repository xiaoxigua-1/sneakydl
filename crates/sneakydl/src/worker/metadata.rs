use url::Url;
use uuid::Uuid;

use crate::config::{Config, SplitStrategy};

pub struct DownloadMetadata<T> {
    pub(crate) id: Uuid,
    pub(crate) url: Url,
    pub(crate) request_metadata: Option<T>,
    pub(crate) split_strategy: SplitStrategy,
    pub(crate) task_concurrency: usize,
}

impl<T> DownloadMetadata<T> {
    pub fn new(id: Uuid, url: Url, request_metadata: Option<T>, config: Config) -> Self {
        Self {
            id,
            url,
            request_metadata,
            split_strategy: config.split_strategy,
            task_concurrency: config.max_concurrent as usize,
        }
    }
}
