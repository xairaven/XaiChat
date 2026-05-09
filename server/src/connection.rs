use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use sqlx::PgPool;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::router::RouterEvent;
use anyhow::Result;
use protocol::{ClientMessage, ServerMessage, UserId};

pub struct ConnectionActor;

impl ConnectionActor {
    // Handles a single TCP connection lifecycle
    pub async fn handle_connection(
        socket: TcpStream, router_tx: mpsc::Sender<RouterEvent>, pool: PgPool,
    ) -> Result<()> {
        // Wrap socket to handle framing automatically
        let mut framed = Framed::new(socket, LengthDelimitedCodec::new());

        // Await the first frame, which must be a Login packet
        let first_frame = match framed.next().await {
            Some(Ok(bytes)) => bytes,
            _ => return Ok(()),
        };

        let login_msg: ClientMessage = postcard::from_bytes(&first_frame)?;

        let (username, password) = match login_msg {
            ClientMessage::Login {
                username,
                password_plain,
            } => (username, password_plain),
            _ => return Ok(()),
        };

        // TODO: Placeholder for actual Argon2 hash verification against PostgreSQL
        // For now, we simulate success and assign a mock ID
        let user_id = Self::authenticate_user(&username, &password, &pool).await?;

        // Channel for the Router to send messages to this specific connection
        let (tx_to_client, mut rx_from_router) = mpsc::channel::<ServerMessage>(100);

        // Notify Router that a new user has joined
        let connect_event = RouterEvent::UserConnected {
            user_id: user_id.clone(),
            sender: tx_to_client,
        };

        if router_tx.send(connect_event).await.is_err() {
            return Ok(());
        }

        // Acknowledge successful login to the client
        let success_msg = ServerMessage::AuthSuccess {
            user_id: user_id.clone(),
        };
        let serialized = postcard::to_stdvec(&success_msg)?;
        framed.send(Bytes::from(serialized)).await?;

        // Main event loop
        loop {
            tokio::select! {
                // Read incoming data from the network
                result = framed.next() => {
                    match result {
                        Some(Ok(bytes)) => {
                            if let Ok(client_msg) = postcard::from_bytes::<ClientMessage>(&bytes) {
                                Self::handle_client_message(client_msg, &user_id, &router_tx).await;
                            }
                        }
                        // Connection closed or broken
                        _ => break,
                    }
                }
                // Read incoming messages routed from the server core
                Some(server_msg) = rx_from_router.recv() => {
                    if let Ok(serialized) = postcard::to_stdvec(&server_msg) {
                        if framed.send(Bytes::from(serialized)).await.is_err() {
                            break;
                        }
                        // TODO
                    }
                }
            }
        }

        // Cleanup when connection drops
        let disconnect_event = RouterEvent::UserDisconnected { user_id };
        let _ = router_tx.send(disconnect_event).await;

        Ok(())
    }

    // Routes valid client messages to the central broker
    async fn handle_client_message(
        msg: ClientMessage, user_id: &UserId, router_tx: &mpsc::Sender<RouterEvent>,
    ) {
        match msg {
            ClientMessage::SendMessage { target, content } => {
                let server_timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let server_msg = ServerMessage::NewMessage {
                    from: user_id.clone(),
                    target: target.clone(),
                    content,
                    timestamp: server_timestamp,
                };

                let route_event = RouterEvent::RouteMessage {
                    from: user_id.clone(),
                    target,
                    message: server_msg,
                };

                let _ = router_tx.send(route_event).await;
            },
            // TODO: Other commands (Sync, CreateGroup) will be handled here
            _ => {},
        }
    }

    // TODO: Mock database function
    async fn authenticate_user(
        _user: &str, _pass: &str, _pool: &PgPool,
    ) -> Result<UserId> {
        Ok(UserId(1))
    }
}
