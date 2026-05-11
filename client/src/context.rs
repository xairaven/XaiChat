use crate::config::Config;
use crate::errors::ClientError;
use crate::network::NetworkCommand;
use crate::ui::modals::Modal;
use crossbeam::channel::{Receiver, Sender};
use protocol::{ServerMessage, UserId};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct Context {
    pub _config: Config,
    pub state: AppState,

    // Channels for local UI errors
    pub errors_tx: Sender<ClientError>,
    pub errors_rx: Receiver<ClientError>,
    // Channels for modals
    pub modals_tx: Sender<Box<dyn Modal>>,
    pub modals_rx: Receiver<Box<dyn Modal>>,

    // Channel: UI sending message -> Network thread reads them and send to the server
    pub network_tx: UnboundedSender<NetworkCommand>,

    // Channel: Network thread reads TCP -> sending messages here -> UI draws
    pub server_rx: Receiver<ServerMessage>,
}

impl Context {
    pub fn new(
        _config: Config, network_tx: UnboundedSender<NetworkCommand>,
        server_rx: Receiver<ServerMessage>,
    ) -> Self {
        let (errors_tx, errors_rx) = crossbeam::channel::unbounded();
        let (modals_tx, modals_rx) = crossbeam::channel::unbounded();

        Self {
            _config,
            state: AppState::default(),
            errors_tx,
            errors_rx,
            modals_tx,
            modals_rx,
            network_tx,
            server_rx,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AppState {
    /// Server address, login and password input or registering
    #[default]
    Auth,
    /// Waiting for server answer
    Connecting,
    /// Main chat window
    Chat { my_user_id: UserId },
}
