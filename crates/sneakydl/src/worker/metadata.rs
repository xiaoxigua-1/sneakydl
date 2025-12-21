use uuid::Uuid;

use crate::{
    config::{Config, SplitStrategy},
    net::RequestMetadata,
};

pub struct DownloadMetadata {
    pub(crate) id: Uuid,
    pub(crate) url: String,
    pub(crate) request_metadata: RequestMetadata,
    pub(crate) split_strategy: SplitStrategy,
    pub(crate) task_concurrency: usize,
}

impl DownloadMetadata {
    pub fn new(id: Uuid, url: String, request_metadata: RequestMetadata, config: Config) -> Self {
        Self {
            id,
            url,
            request_metadata,
            split_strategy: config.split_strategy,
            task_concurrency: config.max_concurrent as usize,
        }
    }
}
