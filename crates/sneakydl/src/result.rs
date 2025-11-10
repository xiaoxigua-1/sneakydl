use tokio::sync::{mpsc, watch};

use crate::{
    storage::StorageWriteRequest,
    task::runtime::{ControlCommand, TaskStatus},
};

pub type Result<T> = std::result::Result<T, SneakydlError>;

#[derive(Debug)]
pub enum SneakydlError {
    Config(&'static str),

    // HttpClient
    RequestError(anyhow::Error),

    // Storage
    IoError(anyhow::Error),
    StorageWriteRequestSendFailed(mpsc::error::SendError<Option<StorageWriteRequest>>),
    StorageWriteResponseSendFailed,
    StorageWriteRequestRecvFailed,
    StorageWriteResponseRecvFailed,

    // Task - Control
    TaskControlCommandSendFailed(watch::error::SendError<ControlCommand>),
    TaskControlCommandRecvFailed,

    // Task - Status
    TaskStatusSendFailed(watch::error::SendError<TaskStatus>),
    TaskStatusRecvFailed,
}
