use crate::router::RouterEvent;
use anyhow::{Context, Result};
use argon2::password_hash::phc::PasswordHash;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerMessage, Target, UserId};
use sqlx::PgPool;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

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

        let auth_msg: ClientMessage = postcard::from_bytes(&first_frame)
            .context("Failed to parse initial auth message")?;

        // Handle both Login and Register flows
        let user_id = match auth_msg {
            ClientMessage::Login {
                username,
                password_plain,
            } => Self::login_user(&username, &password_plain, &pool).await?,
            ClientMessage::Register {
                username,
                password_plain,
            } => Self::register_user(&username, &password_plain, &pool).await?,
            _ => {
                // Disconnect if the client tries to send messages before authenticating
                return Err(anyhow::anyhow!("First message must be Login or Register"));
            },
        };

        // Channel for the Router to send messages to this specific connection
        let (tx_to_client, mut rx_from_router) = mpsc::channel::<ServerMessage>(100);

        // Notify Router that a new user has joined
        let connect_event = RouterEvent::UserConnected {
            user_id: user_id.clone(),
            sender: tx_to_client.clone(),
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
                                Self::handle_client_message(
                                    client_msg,
                                    &user_id,
                                    &router_tx,
                                    &pool,
                                    &tx_to_client
                                ).await;
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
                        todo!()
                    }
                }
            }
        }

        // Cleanup when connection drops
        let disconnect_event = RouterEvent::UserDisconnected { user_id };
        let _ = router_tx.send(disconnect_event).await;

        Ok(())
    }

    // Routes valid client messages to the central broker or handles them directly
    async fn handle_client_message(
        msg: ClientMessage, user_id: &UserId, router_tx: &mpsc::Sender<RouterEvent>,
        pool: &PgPool, tx_to_client: &mpsc::Sender<ServerMessage>,
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
            ClientMessage::Sync { last_timestamp } => {
                Self::sync_messages(user_id, last_timestamp, pool, tx_to_client).await;
            },
            // TODO: Other commands (CreateGroup) will be handled here
            _ => {},
        }
    }

    // Fetches missing messages from the database and sends them to the client
    async fn sync_messages(
        user_id: &UserId, last_timestamp: i64, pool: &PgPool,
        tx_to_client: &mpsc::Sender<ServerMessage>,
    ) {
        // We select messages where the user is either the sender, the direct target,
        // or it's a global broadcast message, strictly ordered by time
        let query_result = sqlx::query!(
            r#"
            SELECT sender_id, target_user_id, is_broadcast, content_payload, created_at
            FROM messages
            WHERE created_at > $1
              AND (target_user_id = $2 OR is_broadcast = true OR sender_id = $2)
            ORDER BY created_at ASC
            "#,
            last_timestamp,
            user_id.0
        )
        .fetch_all(pool)
        .await;

        if let Ok(records) = query_result {
            for record in records {
                if let Ok(payload) = postcard::from_bytes(&record.content_payload) {
                    let is_broadcast = record.is_broadcast.unwrap_or(false);

                    let target = if is_broadcast {
                        Target::Broadcast
                    } else if let Some(t_id) = record.target_user_id {
                        Target::User(UserId(t_id))
                    } else {
                        continue;
                    };

                    let msg = ServerMessage::NewMessage {
                        from: UserId(record.sender_id.unwrap_or(0)),
                        target,
                        content: payload,
                        timestamp: record.created_at,
                    };

                    // Send directly to the connection actor's local channel
                    let _ = tx_to_client.send(msg).await;
                }
            }
        }
    }

    // Authenticates an existing user
    async fn login_user(username: &str, password: &str, pool: &PgPool) -> Result<UserId> {
        let record = sqlx::query!(
            "SELECT id, password_hash FROM users WHERE username = $1",
            username
        )
        .fetch_optional(pool)
        .await?;

        match record {
            Some(user) => {
                let parsed_hash = PasswordHash::new(&user.password_hash)
                    .map_err(|e| anyhow::anyhow!("Invalid hash format: {}", e))?;

                let is_valid = Argon2::default()
                    .verify_password(password.as_bytes(), &parsed_hash)
                    .is_ok();

                if !is_valid {
                    return Err(anyhow::anyhow!(
                        "Invalid password for user: {}",
                        username
                    ));
                }

                println!("User logged in: {}", username);
                Ok(UserId(user.id))
            },
            None => Err(anyhow::anyhow!("User not found: {}", username)),
        }
    }

    // Registers a new user and hashes their password
    async fn register_user(
        username: &str, password: &str, pool: &PgPool,
    ) -> Result<UserId> {
        // First, check if user already exists to avoid database panic
        let exists = sqlx::query!("SELECT id FROM users WHERE username = $1", username)
            .fetch_optional(pool)
            .await?;

        if exists.is_some() {
            return Err(anyhow::anyhow!("Username '{}' is already taken", username));
        }

        let password_hash = Argon2::default()
            .hash_password(password.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
            .to_string();

        let new_user = sqlx::query!(
            "INSERT INTO users (username, password_hash) VALUES ($1, $2) RETURNING id",
            username,
            password_hash
        )
        .fetch_one(pool)
        .await?;

        println!("New user registered: {}", username);
        Ok(UserId(new_user.id))
    }
}
