use crate::context::Context;
use crate::network::NetworkCommand;
use crate::ui::modals::group::{CreateGroupModal, InviteToGroupModal};
use egui::{Color32, Panel, ScrollArea, TextEdit};
use protocol::{ClientMessage, GroupId, Payload, ServerMessage, Target, UserId};
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
        // Left panel: Chats list
        Panel::left("CHAT_SIDEBAR")
            .resizable(false)
            .default_size(200.0)
            .show_inside(ui, |ui| {
                ui.heading("💬 XaiChat");
                ui.separator();

                // Broadcast chat
                let is_broadcast = self.active_chat == Some(Target::Broadcast);
                if ui.selectable_label(is_broadcast, "🌐 Broadcast").clicked() {
                    self.active_chat = Some(Target::Broadcast);
                }
                ui.separator();

                // Groups
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("My Groups:").strong());
                    if ui.button("➕").on_hover_text("Create new group").clicked() {
                        let modal = CreateGroupModal::default();
                        let _ = context.modals_tx.try_send(Box::new(modal));
                    }
                });

                ScrollArea::vertical()
                    .id_salt("groups_scroll")
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for (group_id, name) in &self.groups {
                            let target = Target::Group(group_id.clone());
                            let is_selected = self.active_chat.as_ref() == Some(&target);

                            if ui
                                .selectable_label(is_selected, format!("👥 {}", name))
                                .clicked()
                            {
                                self.active_chat = Some(target);
                            }
                        }
                    });
                ui.separator();

                // Private messages, who is online
                ui.label(egui::RichText::new("Online:").strong());
                ScrollArea::vertical()
                    .id_salt("users_scroll")
                    .show(ui, |ui| {
                        for (id, is_online) in &self.presence {
                            if *is_online {
                                // Do not show myself in online list
                                if let crate::context::AppState::Chat { my_user_id } =
                                    &context.state
                                    && id == my_user_id
                                {
                                    continue;
                                }

                                let target = Target::User(id.clone());
                                let is_selected =
                                    self.active_chat.as_ref() == Some(&target);

                                if ui
                                    .selectable_label(
                                        is_selected,
                                        format!(
                                            "{} User {}",
                                            egui_phosphor::regular::FINN_THE_HUMAN,
                                            id.0
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.active_chat = Some(target);
                                }
                            }
                        }
                    });
            });

        // We check if any chat is selected. If not - we draw a placeholder.
        let active_target = match &self.active_chat {
            Some(t) => t.clone(),
            None => {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.heading("👈 Select chat on the left to start chatting!");
                    });
                });
                return;
            },
        };

        // Bottom panel: Text Input
        Panel::bottom("CHAT_INPUT_PANEL")
            .show_separator_line(true)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    // Text input field
                    let response = ui.add_sized(
                        [ui.available_width() - 85.0, 30.0],
                        TextEdit::singleline(&mut self.draft_message)
                            .hint_text("Write message..."),
                    );

                    // Is user clicked enter or button?
                    let send_clicked = ui.button("Send").clicked();
                    let enter_pressed = response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if (send_clicked || enter_pressed)
                        && !self.draft_message.trim().is_empty()
                    {
                        let payload =
                            Payload::Text(self.draft_message.trim().to_string());

                        let msg = ClientMessage::SendMessage {
                            target: active_target.clone(),
                            content: payload,
                        };

                        // Sending to the network thread
                        let _ = context.network_tx.send(NetworkCommand::Send(msg));

                        // Clearing text input
                        self.draft_message.clear();

                        // Returning focus to the field
                        response.request_focus();
                    }
                });
                ui.add_space(8.0);
            });

        // History and handling
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(match &active_target {
                    Target::Group(_) => "👥 Group Chat".to_string(),
                    Target::Broadcast => "🌐 Broadcast Chat".to_string(),
                    Target::User(id) => format!("👤 Chat with user {}", id.0),
                });

                // If it is a group, draw "Invite" button
                if let Target::Group(_) = active_target {
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.button("Invite by ID").clicked() {
                                let modal =
                                    InviteToGroupModal::new(self.active_chat.clone());
                                let _ = context.modals_tx.try_send(Box::new(modal));
                            }
                        },
                    );
                }
            });
            ui.separator();

            // Scroll zone of message history
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if let Some(history) = self.messages.get(&active_target) {
                        for msg in history {
                            if let ServerMessage::NewMessage { from, content, .. } = msg {
                                let text_content = match content {
                                    Payload::Text(text) => text.clone(),
                                    Payload::File { filename, .. } => {
                                        format!("📎 File: {}", filename)
                                    },
                                };

                                // Draw message
                                ui.horizontal(|ui| {
                                    // Is this our message?
                                    let is_my_msg =
                                        if let crate::context::AppState::Chat {
                                            my_user_id,
                                        } = &context.state
                                        {
                                            from == my_user_id
                                        } else {
                                            false
                                        };

                                    if is_my_msg {
                                        ui.label(
                                            egui::RichText::new("Me:")
                                                .color(Color32::GREEN)
                                                .strong(),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "User {}:",
                                                from.0
                                            ))
                                            .color(Color32::LIGHT_BLUE)
                                            .strong(),
                                        );
                                    }
                                    ui.label(text_content);
                                });
                            }
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("There are no messages yet...").italics(),
                        );
                    }
                });
        });
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
