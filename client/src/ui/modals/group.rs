use crate::context::Context;
use crate::network::NetworkCommand;
use crate::ui::modals::{Modal, ModalFields};
use protocol::{ClientMessage, Target, UserId};

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
    some_user_id: String,
    active_chat: Option<Target>,
}

impl Modal for InviteToGroupModal {
    fn show_content(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        ui.horizontal(|ui| {
            ui.label("User ID:");
            ui.text_edit_singleline(&mut self.some_user_id);
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.columns(2, |columns| {
                columns[0].vertical_centered_justified(|ui| {
                    if ui.button("Invite").clicked() {
                        if let Ok(uid) = self.some_user_id.parse::<i64>()
                            && let Some(Target::Group(group_id)) = &self.active_chat
                        {
                            let msg = ClientMessage::InviteToGroup {
                                group_id: group_id.clone(),
                                user_id: UserId(uid),
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
    pub fn new(active_chat: Option<Target>) -> Self {
        Self {
            modal_fields: ModalFields::default()
                .with_title(format!(
                    "{} Invite User to the Group",
                    egui_phosphor::regular::PEN
                ))
                .with_width(300.0),
            active_chat,
            ..Default::default()
        }
    }
}
