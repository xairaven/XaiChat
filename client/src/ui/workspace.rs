use crate::context::{AppState, Context};
use crate::errors::ClientError;
use crate::network::NetworkError;
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
        self.process_server_messages(context);

        match &context.state {
            AppState::Auth => self.page_auth.show(ui, context),
            AppState::Connecting => self.page_connecting.show(ui, context),
            AppState::Chat { .. } => self.page_chat.show(ui, context),
        }
    }

    fn process_server_messages(&mut self, context: &mut Context) {
        // Read all pending messages from the server
        while let Ok(msg) = context.server_rx.try_recv() {
            match msg {
                protocol::ServerMessage::AuthSuccess { user_id } => {
                    context.state = AppState::Chat {
                        my_user_id: user_id,
                    };
                },
                protocol::ServerMessage::Error(error) => {
                    // Show error modal and go back to auth screen
                    let error = NetworkError::Server(error);
                    let _ = context.errors_tx.send(ClientError::Network(error));
                    context.state = AppState::Auth;
                },
                // All other messages are related to chat
                other_msg => {
                    if let AppState::Chat { my_user_id } = &context.state {
                        self.page_chat.handle_server_message(other_msg, my_user_id);
                    }
                },
            }
        }
    }
}
