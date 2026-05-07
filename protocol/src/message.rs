use crate::group::GroupId;
use crate::user::UserId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    User(UserId),
    Group(GroupId),
    Broadcast,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Payload {
    Text(String),
    File { filename: String, data: Vec<u8> },
}
