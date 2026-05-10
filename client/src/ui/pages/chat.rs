use crate::context::Context;
use crate::network::NetworkCommand;
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
            .resizable(true)
            .default_size(200.0)
            .show_inside(ui, |ui| {
                ui.heading("💬 XaiChat");
                ui.separator();

                ui.label(egui::RichText::new("My Groups:").strong());
                ui.add_space(5.0);

                ScrollArea::vertical().show(ui, |ui| {
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

                ui.add_space(20.0);
                ui.separator();

                // Broadcast
                let is_broadcast = self.active_chat == Some(Target::Broadcast);
                if ui.selectable_label(is_broadcast, "🌐 Broadcast").clicked() {
                    self.active_chat = Some(Target::Broadcast);
                }
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

        // Central panel: Message history
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Heading of opened chat
            ui.heading(match &active_target {
                Target::Group(_) => "👥 Group chat",
                Target::Broadcast => "🌐 Broadcast chat",
                Target::User(_) => "👤 Private chat",
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
                                    ui.label(
                                        egui::RichText::new(format!("ID {}:", from.0))
                                            .color(Color32::LIGHT_BLUE)
                                            .strong(),
                                    );
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
