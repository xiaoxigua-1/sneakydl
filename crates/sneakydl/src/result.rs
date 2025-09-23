use tokio::{
    io,
    sync::{mpsc, watch},
};

use crate::downloader::job::DownloadStatus;

pub type Result<T> = std::result::Result<T, SneakydlError>;

#[derive(Debug)]
pub enum SneakydlError {
    Config(&'static str),
    AddQueueError(mpsc::error::SendError<u64>),
    NotifyTaskStatusError(watch::error::SendError<DownloadStatus>),
    ReqwestError(reqwest::Error),
    IoError(io::Error),
}
