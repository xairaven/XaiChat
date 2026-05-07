use crate::group::GroupId;
use crate::message::{Payload, Target};
use crate::user::UserId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerMessage {
    /// Successful auth. Server returning its user ID
    AuthSuccess { user_id: UserId },

    /// Universal error
    Error(ServerError),

    /// New incoming message
    NewMessage {
        from: UserId,
        target: Target,
        content: Payload,
        timestamp: i64,
    },

    /// Status change of some other user
    Presence { user_id: UserId, online: bool },

    /// Batch of message history
    SyncBatch(Vec<ServerMessage>),

    /// Group creation confirmation
    GroupCreated { group_id: GroupId, name: String },

    /// Invitation alert
    AddedToGroup { group_id: GroupId, name: String },

    GroupMembersList {
        group_id: GroupId,
        members: Vec<UserId>,
    },

    /// Group metadata
    GroupInfo { group_id: GroupId, name: String },
}

#[derive(Error, Serialize, Deserialize, Debug, Clone)]
pub enum ServerError {
    #[error("Invalid login or password.")]
    InvalidCredentials,

    #[error("User with username '{0}' already exists.")]
    UsernameTaken(String),

    #[error("User with ID {0} is not found.")]
    UserNotFound(UserId),

    #[error("Group with ID {0} is not found.")]
    GroupNotFound(GroupId),

    #[error("Permission denied. {0}")]
    PermissionDenied(String),

    #[error("Wrong request format: {0}")]
    BadRequest(String),

    #[error("Server internal error.")]
    InternalError,
}
