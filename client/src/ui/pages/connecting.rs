use crate::context::Context;

#[derive(Debug, Default)]
pub struct ConnectingPage;

impl ConnectingPage {
    pub fn show(&mut self, ui: &mut egui::Ui, _context: &mut Context) {
        ui.centered_and_justified(|ui| {
            ui.spinner();
            ui.heading("Connecting to server...");
        });
    }
}
