use crate::context::Context;
use protocol::{GroupId, ServerMessage, Target, UserId};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct ChatPage {
    // Group list where we are (ID, Label)
    pub groups: Vec<(GroupId, String)>,

    // Group users cache list (Key is ID of group, value - user list)
    pub group_members: HashMap<GroupId, Vec<UserId>>,

    // Message history. Key -- Target (Group, Broadcast or User)
    pub messages: HashMap<Target, Vec<ServerMessage>>,

    // Who is online now
    pub presence: HashMap<UserId, bool>,

    // In what chat we are reading/writing now (If None - none is open)
    pub active_chat: Option<Target>,

    // String for text input of new message
    pub draft_message: String,
}

impl ChatPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut Context) {
        ui.heading("Chat Workspace");
        // TODO: Chat interface
    }

    pub fn handle_server_message(&mut self, msg: ServerMessage, my_user_id: &UserId) {
        match msg {
            ServerMessage::GroupsList(list) => {
                self.groups = list;
            },
            ServerMessage::NewMessage {
                from,
                target,
                content,
                timestamp,
            } => {
                // Deciding in what "tab" of chat we will put this message
                let chat_key = match &target {
                    Target::Group(_) | Target::Broadcast => target.clone(),
                    Target::User(target_id) => {
                        // If this is private message:
                        // If sender - me, then I wrote someone (chat with target_id)
                        if &from == my_user_id {
                            Target::User(target_id.clone())
                        } else {
                            // If sender is someone else, then someone wrote to me (chat with from)
                            Target::User(from.clone())
                        }
                    },
                };

                let msg_obj = ServerMessage::NewMessage {
                    from,
                    target,
                    content,
                    timestamp,
                };
                self.messages.entry(chat_key).or_default().push(msg_obj);
            },
            ServerMessage::SyncBatch(batch) => {
                // Recursive unpacking of history batch
                for m in batch {
                    self.handle_server_message(m, my_user_id);
                }
            },
            ServerMessage::Presence { user_id, online } => {
                self.presence.insert(user_id, online);
            },
            ServerMessage::GroupCreated { group_id, name }
            | ServerMessage::AddedToGroup { group_id, name } => {
                // Adding new group to the list, if it's not there yet
                if !self.groups.iter().any(|(id, _)| id == &group_id) {
                    self.groups.push((group_id, name));
                }
            },
            ServerMessage::GroupMembersList { group_id, members } => {
                self.group_members.insert(group_id, members);
            },
            ServerMessage::GroupInfo { group_id, name } => {
                // Searching for group in list. If we found that - updating label
                if let Some(group) =
                    self.groups.iter_mut().find(|(id, _)| id == &group_id)
                {
                    group.1 = name;
                } else {
                    // If it's not there -- adding
                    self.groups.push((group_id, name));
                }
            },
            ServerMessage::AuthSuccess { .. } => {
                unreachable!("ServerMessage::AuthSuccess");
            },
            ServerMessage::Error(_) => {
                unreachable!("ServerMessage::Error");
            },
        }
    }
}
