use bytes::Bytes;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio_stream::StreamExt;

use reqwest::Client;
use std::io::{IoSlice, Seek, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch::{Receiver, Sender, channel};

use crate::downloader::task::DownloadTask;
use crate::result::{Result, SneakydlError};

#[derive(Debug, Clone)]
pub enum DownloadStatus {
    Pending,
    Running {
        progress: Option<f32>,
        downloaded_bytes: usize,
    },
    Completed {
        downloaded_bytes: usize,
    },
}

#[derive(Debug)]
pub struct DownloadJob {
    pub task: DownloadTask,
    pub status: (Sender<DownloadStatus>, Mutex<Receiver<DownloadStatus>>),
    pub status_update_interval: Duration,
    pub chunk_size: usize,
}

impl DownloadJob {
    pub fn new(task: DownloadTask, status_update_interval: Duration, chunk_size: usize) -> Self {
        let (tx, rx) = channel(DownloadStatus::Pending);

        DownloadJob {
            task,
            status: (tx, Mutex::new(rx)),
            status_update_interval,
            chunk_size,
        }
    }

    pub async fn run(&self, client: Client) -> Result<Vec<JoinHandle<Result<()>>>> {
        let response = client
            .request(self.task.request_method.clone(), self.task.url.clone())
            .headers(self.task.headers.clone())
            .send()
            .await
            .map_err(SneakydlError::ReqwestError)?;

        let size = response.content_length();
        let start_offset = self.task.range.clone().map(|r| r.start).unwrap_or(0);
        let current_offset = Arc::new(RwLock::new(start_offset as usize));

        let mut stream = response.bytes_stream();
        let mut chunks: Vec<Bytes> = vec![];
        let mut wire_tasks: Vec<JoinHandle<Result<()>>> = vec![];

        let status_tx = self.status.0.clone();
        let status_update_interval = self.status_update_interval;
        let current_offset_clone = current_offset.clone();

        let status_updater: JoinHandle<Result<()>> = tokio::spawn(async move {
            let mut interval = interval(status_update_interval);

            loop {
                let current_offset = *current_offset_clone.read().await;

                status_tx
                    .send(DownloadStatus::Running {
                        progress: size.map(|size| current_offset as f32 / size as f32),
                        downloaded_bytes: current_offset,
                    })
                    .map_err(SneakydlError::NotifyTaskStatusError)?;
                interval.tick().await;
            }
        });

        while let Some(bytes) = stream.next().await {
            let bytes = bytes.map_err(SneakydlError::ReqwestError)?;

            *current_offset.write().await += bytes.len();
            chunks.push(bytes);

            let current_offset = *current_offset.read().await;
            if chunks.iter().map(|b| b.len()).sum::<usize>() >= self.chunk_size {
                wire_tasks.push(self.wire_file(chunks.clone(), current_offset));
                chunks.clear();
            }
        }

        if chunks.iter().map(|b| b.len()).sum::<usize>() >= self.chunk_size {
            wire_tasks.push(self.wire_file(chunks.clone(), *current_offset.read().await));
        }

        status_updater.abort();
        self.status
            .0
            .send(DownloadStatus::Completed {
                downloaded_bytes: *current_offset.read().await,
            })
            .map_err(SneakydlError::NotifyTaskStatusError)?;

        Ok(wire_tasks)
    }

    fn wire_file(&self, bufs: Vec<Bytes>, offset: usize) -> JoinHandle<Result<()>> {
        let file = self.task.file.clone();

        tokio::spawn(async move {
            let mut file = file.lock().await;
            let io_slices = bufs
                .iter()
                .map(|b| IoSlice::new(b))
                .collect::<Vec<IoSlice>>();

            file.seek(std::io::SeekFrom::Start(offset as u64))
                .map_err(SneakydlError::IoError)?;
            file.write_vectored(&io_slices)
                .map_err(SneakydlError::IoError)?;

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;

    use crate::downloader::job::DownloadJob;

    use super::*;

    #[tokio::test]
    async fn test() {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .open("/dev/null")
            .unwrap();
        let task = DownloadTask::builder(
            "http://127.0.0.1:8080".to_string(),
            Arc::new(Mutex::new(file)),
        )
        .header("User-Agent", "curl/8.0".parse().unwrap())
        .build();
        let job = Arc::new(DownloadJob::new(
            task,
            Duration::from_secs(1),
            5 * 1024 * 1024,
        ));
        let job_clone = job.clone();
        let client = Client::builder().build().unwrap();

        let status_updaer = tokio::spawn(async move {
            let mut rx = job_clone.status.1.lock().await;
            let mut last_downladed: usize = 0;
            loop {
                rx.changed().await.unwrap();

                if let DownloadStatus::Running {
                    progress,
                    downloaded_bytes,
                } = rx.borrow_and_update().clone()
                {
                    println!(
                        "{}MB/s {:?}",
                        (downloaded_bytes - last_downladed) as f32 / 1024.0 / 1024.0,
                        progress
                    );

                    last_downladed = downloaded_bytes;
                }
            }
        });
        tokio::spawn(async move {
            let task = job.run(client).await.unwrap();
            status_updaer.abort();

            println!("{:?}", &task);
        })
        .await
        .unwrap();
    }
}
