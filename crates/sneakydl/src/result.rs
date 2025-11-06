use tokio::sync::{mpsc, watch};

use crate::{storage::WriteRequest, task::TaskStatus};

pub type Result<T> = std::result::Result<T, SneakydlError>;

#[derive(Debug)]
pub enum SneakydlError {
    Config(&'static str),

    // HttpClient
    RequestError(anyhow::Error),

    // Storage
    IoError(anyhow::Error),

    // Storage Manager
    StorageRequestSendFailed(mpsc::error::SendError<Option<WriteRequest>>),
    StorageNoRequestFailed,

    // Storage notify
    NotifySendFailed,
    NotifyRecvFailed,

    // Task
    TaskUpdateStatusSendFailed(watch::error::SendError<TaskStatus>),
    TaskUpdateStatusRecvFailed,
}
