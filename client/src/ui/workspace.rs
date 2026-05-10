use crate::context::{AppState, Context};
use crate::ui::pages::auth::AuthPage;
use crate::ui::pages::chat::ChatPage;
use crate::ui::pages::connecting::ConnectingPage;

#[derive(Debug)]
pub struct Workspace {
    page_auth: AuthPage,
    page_connecting: ConnectingPage,
    page_chat: ChatPage,
}

impl Workspace {
    pub fn new(_context: &Context) -> Self {
        Self {
            page_auth: Default::default(),
            page_connecting: Default::default(),
            page_chat: Default::default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, context: &mut Context) {
        match &context.state {
            AppState::Auth => self.page_auth.show(ui, context),
            AppState::Connecting => self.page_connecting.show(ui, context),
            AppState::Chat { .. } => self.page_chat.show(ui, context),
        }
    }
}
