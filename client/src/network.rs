use crate::errors::ClientError;
use bytes::Bytes;
use crossbeam::channel::Sender;
use futures::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerMessage};
use std::thread;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Debug)]
pub enum NetworkCommand {
    Connect {
        addr: String,
        auth_message: ClientMessage,
    },
    Send(ClientMessage),
}

#[derive(Debug)]
pub struct NetworkManager {
    pub network_rx: UnboundedReceiver<NetworkCommand>,
    pub server_tx: Sender<ServerMessage>,
    pub error_tx: Sender<ClientError>,
    pub egui_ctx: egui::Context,
}

impl NetworkManager {
    // Starts the Tokio runtime in a background OS thread
    pub fn start(mut self) {
        thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(NetworkError::TokioRuntime)
            {
                Ok(value) => value,
                Err(error) => {
                    let _ = self.error_tx.send(ClientError::Network(error));
                    return;
                },
            };

            rt.block_on(async move {
                // Network thread runs forever, waiting for a Connect command from UI
                loop {
                    match self.network_rx.recv().await {
                        Some(NetworkCommand::Connect { addr, auth_message }) => {
                            if let Err(error) = Self::handle_connection(
                                &addr,
                                auth_message,
                                &mut self.network_rx,
                                &self.server_tx,
                                &self.egui_ctx,
                            )
                            .await
                            {
                                // Revert UI to Auth screen and show error modal
                                let error_message = ServerMessage::Error(
                                    protocol::ServerError::BadRequest(format!(
                                        "Connection failed: {}",
                                        error,
                                    )),
                                );
                                let _ = self.server_tx.send(error_message);
                                self.egui_ctx.request_repaint(); // Wake up UI
                            }
                        },
                        Some(NetworkCommand::Send(_)) => {
                            let error = NetworkError::TriedSendWhileDisconnected;
                            let _ = self.error_tx.send(ClientError::Network(error));
                            log::error!("Tried to send something while disconnected.");
                        },
                        None => break, // App closed
                    }
                }
            });
        });
    }

    async fn handle_connection(
        addr: &str, auth_msg: ClientMessage,
        network_rx: &mut UnboundedReceiver<NetworkCommand>,
        server_tx: &Sender<ServerMessage>, egui_ctx: &egui::Context,
    ) -> Result<(), NetworkError> {
        let stream = TcpStream::connect(addr)
            .await
            .map_err(NetworkError::InitializeConnection)?;
        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        // Send the initial auth message
        let serialized_auth =
            postcard::to_stdvec(&auth_msg).map_err(NetworkError::Postcard)?;
        framed
            .send(Bytes::from(serialized_auth))
            .await
            .map_err(NetworkError::Send)?;

        // Main event loop
        loop {
            tokio::select! {
                cmd_opt = network_rx.recv() => {
                    match cmd_opt {
                        Some(NetworkCommand::Send(client_msg)) => {
                            let serialized = postcard::to_stdvec(&client_msg)
                                .map_err(NetworkError::Postcard)?;
                            framed.send(Bytes::from(serialized)).await
                                .map_err(NetworkError::Send)?;
                        }
                        Some(NetworkCommand::Connect { .. }) => {
                            log::error!("Already connected.");
                        }
                        None => break,
                    }
                }
                frame_opt = framed.next() => {
                    match frame_opt {
                        Some(Ok(bytes)) => {
                            let server_msg: ServerMessage = postcard::from_bytes(&bytes)
                                .map_err(NetworkError::Postcard)?;
                            let _ = server_tx.send(server_msg);

                            // WAKE UP EGUI TO DRAW THE NEW MESSAGE INSTANTLY
                            egui_ctx.request_repaint();
                        }
                        Some(Err(error)) => return Err(NetworkError::Bytes(error)),
                        None => return Err(NetworkError::ServerClosedConnection),
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Failed to deserialize byte package. {0}")]
    Bytes(std::io::Error),

    #[error("Failed to initialize connection. {0}")]
    InitializeConnection(std::io::Error),

    #[error("Failed to serialize/deserialize structure. {0}")]
    Postcard(postcard::Error),

    #[error("Failed to send packet. {0}")]
    Send(std::io::Error),

    #[error("Server. {0}")]
    Server(#[from] protocol::ServerError),

    #[error("Server closed connection unexpectedly.")]
    ServerClosedConnection,

    #[error("Failed to build Tokio runtime. Please, restart app. {0}")]
    TokioRuntime(std::io::Error),

    #[error("Tried to send message while disconnected.")]
    TriedSendWhileDisconnected,
}
