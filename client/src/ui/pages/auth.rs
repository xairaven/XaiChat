use crate::context::Context;

#[derive(Debug, Default)]
pub struct AuthPage;

impl AuthPage {
    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut Context) {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.heading("Welcome to XaiChat");
            ui.label("Please connect to a server.");
            // TODO: Login, Register
        });
    }
}
