use crate::config::Config;
use crate::context::Context;
use crate::ui::modals::ModalsHandler;
use crate::ui::workspace::Workspace;
use eframe::Frame;
use egui::{CentralPanel, Ui};

pub struct AppCreator {
    pub context: Context,
    pub workspace: Workspace,

    modals_handler: ModalsHandler,
}

impl AppCreator {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        Self::set_fonts(cc);
        Self::set_theme(cc, &config);

        let context = Context::new(config);
        let workspace = Workspace::new(&context);

        Self {
            context,
            workspace,
            modals_handler: ModalsHandler::default(),
        }
    }

    fn set_theme(cc: &eframe::CreationContext<'_>, config: &Config) {
        cc.egui_ctx.set_theme(config.theme);
    }

    fn set_fonts(cc: &eframe::CreationContext<'_>) {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
    }
}

impl eframe::App for AppCreator {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        CentralPanel::default().show_inside(ui, |ui| {
            self.workspace.show(ui, &mut self.context);

            self.modals_handler.handle_errors(ui, &self.context);
        });
    }
}
