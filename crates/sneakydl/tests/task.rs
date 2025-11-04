use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use sneakydl::{
    net::{RequestMetadata, reqwest_client::ReqwestClient},
    storage::{Storage, StorageWorker},
    task::Task,
};
use tokio::test;
use uuid::Uuid;

struct VoidStorage;

#[async_trait]
impl Storage for VoidStorage {
    type Dest = ();

    async fn create_dest(&self) -> anyhow::Result<Self::Dest> {
        Ok(())
    }

    async fn write_at(
        &self,
        _: &mut Self::Dest,
        _: u64,
        _: Vec<bytes::Bytes>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[test]
async fn task_test() {
    env_logger::init();
    let reqwest_client = Arc::new(ReqwestClient::default());
    let mut storage_worker = StorageWorker::new(VoidStorage, 100);

    let download_id = Uuid::new_v4();
    let url = "http://localhost:8080";
    let request_metadata = RequestMetadata::new("GET", HashMap::new());
    let metadata = sneakydl::task::TaskMetadata {
        task_id: 0,
        download_id,
        url,
        request_metadata,
        range: None,
        max_retries: 1,
    };
    let request_tx = storage_worker.get_request_tx();
    let mut task = Task::new(reqwest_client, request_tx, metadata);

    let download_job = tokio::spawn(async move { task.job().await });
    let storage_job = tokio::spawn(async move {
        storage_worker.run().await.unwrap();
    });

    /*
     * Simplify single-Task test with tokio::select!
     *
     * In this single-Task test, we use `tokio::select!` to simplify the code:
     *  - `select!` works here because each Task will only complete after all its writes have been fully processed.
     *  - When the Task finishes, it guarantees that the StorageWorker has completed all write operations.
     *  - If the StorageWorker future is dropped (due to select!), it's safe since all necessary writes for this Task are already done.
     * This keeps the test simple while preserving correctness.
     */
    tokio::select! {
        Ok(r) = download_job => {
            if let Err(e) = r {
                panic!("{:?}", e);
            }
        }
        _ = storage_job => {}
    }
}
