use tokio::sync::{AcquireError, mpsc, watch};

use crate::{
    storage::{StorageWriteRequest, monitor::StorageStatus},
    task::runtime::{ControlCommand, TaskStatus},
};

pub type Result<T> = std::result::Result<T, SneakydlError>;

#[derive(Debug)]
pub enum SneakydlError {
    Config(&'static str),

    JoinError(tokio::task::JoinError),

    // HttpClient
    RequestError(anyhow::Error),

    // Storage
    IoError(anyhow::Error),
    StorageWriteRequestSendFailed(mpsc::error::SendError<Option<StorageWriteRequest>>),
    StorageWriteResponseSendFailed,
    StorageWriteRequestRecvFailed,
    StorageWriteResponseRecvFailed,
    StorageStatusSendFailed(watch::error::SendError<StorageStatus>),
    StorageStatusRecvFailed,

    // Task - Control
    TaskControlCommandSendFailed(watch::error::SendError<ControlCommand>),
    TaskControlCommandRecvFailed,

    // Task - Status
    TaskStatusSendFailed(mpsc::error::SendError<TaskStatus>),
    TaskStatusRecvFailed,

    AcquireError(AcquireError),
}
