use crate::context::Context;
use crate::network::NetworkCommand;
use crate::ui::modals::{Modal, ModalFields};
use protocol::{ClientMessage, Target, UserId};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CreateGroupModal {
    modal_fields: ModalFields,
    new_group_name: String,
}

impl Default for CreateGroupModal {
    fn default() -> Self {
        Self {
            modal_fields: ModalFields::default()
                .with_title(format!("{} Create Group", egui_phosphor::regular::PENCIL))
                .with_width(300.0),
            new_group_name: "".to_string(),
        }
    }
}

impl Modal for CreateGroupModal {
    fn show_content(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            ui.label("Title:");
            ui.text_edit_singleline(&mut self.new_group_name);
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.columns(2, |columns| {
                columns[0].vertical_centered_justified(|ui| {
                    if ui.button("Create").clicked()
                        && !self.new_group_name.trim().is_empty()
                    {
                        let msg = ClientMessage::CreateGroup {
                            name: self.new_group_name.trim().to_string(),
                        };
                        let _ = ctx.network_tx.send(NetworkCommand::Send(msg));
                        self.close();
                    }
                });
                columns[1].vertical_centered_justified(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.close();
                    }
                });
            });
        });
    }

    fn close(&mut self) {
        self.modal_fields.is_open = false;
    }

    fn modal_fields(&self) -> &ModalFields {
        &self.modal_fields
    }
}

#[derive(Debug, Default, Clone)]
pub struct InviteToGroupModal {
    modal_fields: ModalFields,
    desired_user_nickname: String,
    active_chat: Option<Target>,
    address_book: HashMap<UserId, String>,
}

impl Modal for InviteToGroupModal {
    fn show_content(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            ui.label("User Nickname:");
            ui.text_edit_singleline(&mut self.desired_user_nickname);
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.columns(2, |columns| {
                columns[0].vertical_centered_justified(|ui| {
                    if ui.button("Invite").clicked() {
                        let target_user = self
                            .address_book
                            .iter()
                            .find(|(_, name)| name == &&self.desired_user_nickname)
                            .map(|(id, _)| id.clone());

                        if let Some(uid) = target_user
                            && let Some(Target::Group(group_id)) = &self.active_chat
                        {
                            let msg = ClientMessage::InviteToGroup {
                                group_id: group_id.clone(),
                                user_id: uid,
                            };
                            let _ = ctx.network_tx.send(NetworkCommand::Send(msg));
                        }
                        self.close();
                    }
                });
                columns[1].vertical_centered_justified(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.close();
                    }
                });
            });
        });
    }

    fn close(&mut self) {
        self.modal_fields.is_open = false;
    }

    fn modal_fields(&self) -> &ModalFields {
        &self.modal_fields
    }
}

impl InviteToGroupModal {
    pub fn new(
        active_chat: Option<Target>, address_book: HashMap<UserId, String>,
    ) -> Self {
        Self {
            modal_fields: ModalFields::default()
                .with_title(format!(
                    "{} Invite User to the Group",
                    egui_phosphor::regular::PEN
                ))
                .with_width(300.0),
            active_chat,
            address_book,
            ..Default::default()
        }
    }
}
