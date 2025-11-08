use tokio::sync::{mpsc, watch};

use crate::{
    storage::WriteRequest,
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
    WriteRequestSendFailed(mpsc::error::SendError<Option<WriteRequest>>),
    WriteRequestReceiveFailed,
    WriteResponseSendFailed,
    WriteResponseReceiveFailed,

    // Task
    TaskUpdateControlCommandSendFailed(watch::error::SendError<ControlCommand>),
    TaskUpdateControlCommandRecvFailed,
    TaskUpdateStatusSendFailed(watch::error::SendError<TaskStatus>),
    TaskUpdateStatusRecvFailed,
}
