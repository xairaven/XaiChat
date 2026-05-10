use crate::context::{AppState, Context};
use crate::network::NetworkCommand;
use egui::{Grid, TextEdit};
use protocol::ClientMessage;

#[derive(Debug)]
pub struct AuthPage {
    username: String,
    password_plain: String,
    server_address: String,
}

impl Default for AuthPage {
    fn default() -> Self {
        Self {
            username: "".to_string(),
            password_plain: "".to_string(),
            server_address: "127.0.0.1:8080".to_string(),
        }
    }
}

impl AuthPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut Context) {
        ui.columns(3, |columns| {
            columns[1].vertical_centered(|ui| {
                ui.add_space(100.0);

                ui.heading("Welcome to XaiChat");
                ui.label("Please connect to a server.");

                ui.add_space(20.0);

                Grid::new("AUTH_GRID").num_columns(2).show(ui, |ui| {
                    ui.label("Server Address:");
                    ui.add(
                        TextEdit::singleline(&mut self.server_address)
                            .desired_width(ui.available_width()),
                    );
                    ui.end_row();

                    ui.label("Username:");
                    ui.add(
                        TextEdit::singleline(&mut self.username)
                            .desired_width(ui.available_width()),
                    );
                    ui.end_row();

                    ui.label("Password:");
                    ui.add(
                        TextEdit::singleline(&mut self.password_plain)
                            .password(true)
                            .desired_width(ui.available_width()),
                    );
                    ui.end_row();
                });

                ui.add_space(30.0);

                ui.horizontal(|ui| {
                    ui.columns(2, |columns| {
                        columns[0].vertical_centered_justified(|ui| {
                            if ui.button("Login").clicked() {
                                let auth_message = ClientMessage::Login {
                                    username: self.username.trim().to_string(),
                                    password_plain: self
                                        .password_plain
                                        .trim()
                                        .to_string(),
                                };

                                // Tell the network thread to connect and send the auth message
                                let _ =
                                    context.network_tx.send(NetworkCommand::Connect {
                                        addr: self.server_address.trim().to_string(),
                                        auth_message,
                                    });

                                // Show the spinner while waiting for server response
                                context.state = AppState::Connecting;
                            }
                        });
                        columns[1].vertical_centered_justified(|ui| {
                            if ui.button("Register").clicked() {
                                let auth_message = ClientMessage::Register {
                                    username: self.username.trim().to_string(),
                                    password_plain: self
                                        .password_plain
                                        .trim()
                                        .to_string(),
                                };

                                // Tell the network thread to connect and send the auth message
                                let _ =
                                    context.network_tx.send(NetworkCommand::Connect {
                                        addr: self.server_address.trim().to_string(),
                                        auth_message,
                                    });

                                // Show the spinner while waiting for server response
                                context.state = AppState::Connecting;
                            }
                        });
                    });
                });
            });
        });
    }
}
