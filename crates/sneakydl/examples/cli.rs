use std::{collections::HashMap, sync::Arc};

use sneakydl::{
    config::{Config, SplitStrategy},
    net::{RequestMetadata, RequestMethod, reqwest_client::ReqwestClient},
    storage::tokio_file::TokioStorage,
    task::runtime::TaskStatus,
    worker::{DownloadWorker, metadata::DownloadMetadata},
};

use clap::Parser;
use indicatif::ProgressBar;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    url: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();
    let cli = Args::parse();
    let mut config = Config::default();
    let reqwest_client = ReqwestClient::new(reqwest::Client::new());
    let tokio_file = TokioStorage::new("./(MMA)-100MB.zip");

    config.split_strategy = SplitStrategy::Single;

    let download_worker_metadata = DownloadMetadata::new(
        Uuid::default(),
        cli.url,
        RequestMetadata::new(RequestMethod::GET, HashMap::new()),
        config,
    );
    let mut worker = DownloadWorker::new(
        Arc::new(reqwest_client),
        Arc::new(tokio_file),
        download_worker_metadata,
    )
    .await
    .unwrap();

    let task_monitor = worker.subscribe_task_status();
    let status_monitor_job = task_monitor.map(|mut monitor| {
        tokio::spawn(async move {
            let mut pb: Option<ProgressBar> = None;
            while let Some(status) = monitor.recv().await {
                match status {
                    TaskStatus::Pending {
                        download_id: _,
                        task_id: _,
                        content_length,
                    } => {
                        if let Some(content_length) = content_length {
                            pb = Some(ProgressBar::new(content_length));
                        } else {
                            pb = Some(ProgressBar::new_spinner());
                        };
                    }
                    TaskStatus::Downloading {
                        download_id: _,
                        task_id: _,
                        downloaded,
                    } => {
                        if let Some(pb) = &pb {
                            pb.inc(downloaded);
                        }
                    }

                    TaskStatus::Completed {
                        download_id: _,
                        task_id: _,
                        total_bytes: _,
                    } => {
                        break;
                    }

                    _ => {}
                }
            }
        })
    });

    let _ = worker.run().await;
    if let Some(status_monitor_job) = status_monitor_job {
        status_monitor_job.await.unwrap();
    }
}
