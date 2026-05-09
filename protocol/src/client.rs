use crate::group::GroupId;
use crate::message::{Payload, Target};
use crate::user::UserId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientMessage {
    Login {
        username: String,
        password_plain: String,
    },

    Register {
        username: String,
        password_plain: String,
    },

    SendMessage {
        target: Target,
        content: Payload,
    },

    /// Request for synchronization after disconnection or offline
    Sync {
        last_timestamp: i64,
    },

    /// Creating new group
    CreateGroup {
        name: String,
    },

    /// Inviting someone to the group
    InviteToGroup {
        group_id: GroupId,
        user_id: UserId,
    },

    FetchGroupMembers {
        group_id: GroupId,
    },

    /// Group info -- title, etc.
    FetchGroupInfo {
        group_id: GroupId,
    },
}
