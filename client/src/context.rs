use crate::config::Config;
use crate::errors::ClientError;
use crossbeam::channel::{Receiver, Sender};

#[derive(Debug)]
pub struct Context {
    // Channels
    pub errors_tx: Sender<ClientError>,
    pub errors_rx: Receiver<ClientError>,
}

impl Context {
    pub fn new(_config: Config) -> Self {
        let (errors_tx, errors_rx) = crossbeam::channel::unbounded();

        Self {
            errors_tx,
            errors_rx,
        }
    }
}
