use uuid::Uuid;

use crate::{config::SplitStrategy, net::RequestMetadata};

pub struct DownloadMetadata {
    pub(crate) id: Uuid,
    pub(crate) url: String,
    pub(crate) request_metadata: RequestMetadata,
    pub(crate) split_strategy: SplitStrategy,
    pub(crate) task_concurrency: usize,
}
