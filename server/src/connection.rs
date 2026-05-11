use crate::router::RouterEvent;
use anyhow::{Context, Result};
use argon2::password_hash::phc::PasswordHash;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use protocol::{ClientMessage, ServerError, ServerMessage, Target, UserId};
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
        let user_id: Result<UserId, ServerError> = match auth_msg {
            ClientMessage::Login {
                username,
                password_plain,
            } => {
                match Self::login_user(&username, &password_plain, &pool).await {
                    Ok(id) => Ok(id),
                    Err(_) => {
                        // Notify client about bad login before dropping connection
                        Err(ServerError::InvalidCredentials)
                    },
                }
            },
            ClientMessage::Register {
                username,
                password_plain,
            } => match Self::register_user(&username, &password_plain, &pool).await {
                Ok(id) => Ok(id),
                Err(_) => Err(ServerError::UsernameTaken(username)),
            },
            _ => Err(ServerError::FirstMessageNotAuth),
        };
        let user_id = match user_id {
            Ok(id) => id,
            Err(error) => {
                let error_msg = ServerMessage::Error(error.clone());
                if let Ok(bytes) = postcard::to_stdvec(&error_msg) {
                    let _ = framed.send(Bytes::from(bytes)).await;

                    let mut socket = framed.into_inner();
                    use tokio::io::AsyncWriteExt;
                    let _ = socket.shutdown().await;
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                return Err(error.into());
            },
        };

        // Fetch all groups the user is a member of, ALONG WITH THEIR NAMES
        let groups_records = sqlx::query!(
            "SELECT g.id, g.name
             FROM groups g
             INNER JOIN group_members gm ON g.id = gm.group_id
             WHERE gm.user_id = $1",
            user_id.0
        )
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        let mut groups_for_router = Vec::new();
        let mut groups_for_client = Vec::new();

        for r in groups_records {
            let gid = protocol::GroupId(r.id);
            groups_for_router.push(gid.clone());
            groups_for_client.push((gid, r.name));
        }

        // Channel for the Router to send messages to this specific connection
        let (tx_to_client, mut rx_from_router) = mpsc::channel::<ServerMessage>(100);

        // Notify Router that a new user has joined
        let connect_event = RouterEvent::UserConnected {
            user_id: user_id.clone(),
            groups: groups_for_router,
            sender: tx_to_client.clone(),
        };

        if router_tx.send(connect_event).await.is_err() {
            return Ok(());
        }

        // Acknowledge successful login
        let success_msg = ServerMessage::AuthSuccess {
            user_id: user_id.clone(),
        };
        let serialized_auth = postcard::to_stdvec(&success_msg)?;
        framed.send(Bytes::from(serialized_auth)).await?;

        // Synchronize the user's groups state
        let sync_groups_msg = ServerMessage::GroupsList(groups_for_client);
        let serialized_groups = postcard::to_stdvec(&sync_groups_msg)?;
        framed.send(Bytes::from(serialized_groups)).await?;

        // Sync contact book (all registered users)
        let users_records = sqlx::query!("SELECT id, username FROM users")
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

        let users_list = users_records
            .into_iter()
            .map(|r| (UserId(r.id), r.username))
            .collect();

        let sync_users_msg = ServerMessage::UsersList(users_list);
        let serialized_users = postcard::to_stdvec(&sync_users_msg)?;
        framed.send(Bytes::from(serialized_users)).await?;

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
                    if let Ok(serialized) = postcard::to_stdvec(&server_msg)
                    && framed.send(Bytes::from(serialized)).await.is_err() {
                            break;
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
            ClientMessage::Login { .. } | ClientMessage::Register { .. } => {
                // User is already authenticated. Sending this again is a protocol violation.
                let err = ServerMessage::Error(ServerError::AlreadyAuthenticated);
                let _ = tx_to_client.send(err).await;
            },

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
            ClientMessage::InviteToGroup {
                group_id,
                user_id: invited_user,
            } => {
                if sqlx::query!("INSERT INTO group_members (group_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING", group_id.0, invited_user.0)
                    .execute(pool).await.is_ok()
                {
                    // Tell router to update its internal Pub/Sub state
                    let _ = router_tx.send(RouterEvent::UserJoinedGroup {
                        user_id: invited_user.clone(),
                        group_id: group_id.clone(),
                    }).await;

                    // Alert the user they were added
                    if let Ok(record) = sqlx::query!("SELECT name FROM groups WHERE id = $1", group_id.0).fetch_one(pool).await {
                        let alert = ServerMessage::AddedToGroup {
                            group_id,
                            name: record.name,
                        };
                        let _ = router_tx.send(RouterEvent::RouteMessage {
                            from: user_id.clone(),
                            target: Target::User(invited_user),
                            message: alert,
                        }).await;
                    }
                }
            },
            ClientMessage::CreateGroup { name } => {
                if let Ok(record) = sqlx::query!(
                    "INSERT INTO groups (name) VALUES ($1) RETURNING id",
                    name
                )
                .fetch_one(pool)
                .await
                {
                    // Add creator to the group
                    let _ = sqlx::query!(
                        "INSERT INTO group_members (group_id, user_id) VALUES ($1, $2)",
                        record.id,
                        user_id.0
                    )
                    .execute(pool)
                    .await;

                    let group_id = protocol::GroupId(record.id);

                    // Subscribe the user in the Router's RAM immediately (Pub/Sub)
                    let _ = router_tx
                        .send(RouterEvent::UserJoinedGroup {
                            user_id: user_id.clone(),
                            group_id: group_id.clone(),
                        })
                        .await;

                    // Confirm creation to the client
                    let msg = ServerMessage::GroupCreated { group_id, name };
                    let _ = tx_to_client.send(msg).await;
                }
            },
            ClientMessage::FetchGroupMembers { group_id } => {
                if let Ok(records) = sqlx::query!(
                    "SELECT user_id FROM group_members WHERE group_id = $1",
                    group_id.0
                )
                .fetch_all(pool)
                .await
                {
                    let members =
                        records.into_iter().map(|r| UserId(r.user_id)).collect();
                    let _ = tx_to_client
                        .send(ServerMessage::GroupMembersList { group_id, members })
                        .await;
                }
            },
            ClientMessage::FetchGroupInfo { group_id } => {
                let record =
                    sqlx::query!("SELECT name FROM groups WHERE id = $1", group_id.0)
                        .fetch_optional(pool)
                        .await;

                let message: ServerMessage = match record {
                    Ok(Some(rec)) => ServerMessage::GroupInfo {
                        group_id,
                        name: rec.name,
                    },
                    Ok(None) => {
                        // Group is not found in the database
                        ServerMessage::Error(ServerError::GroupNotFound(group_id))
                    },
                    Err(_) => {
                        // Database error
                        ServerMessage::Error(ServerError::InternalError)
                    },
                };
                let _ = tx_to_client.send(message).await;
            },
        }
    }

    // Fetches missing messages from the database and sends them to the client
    async fn sync_messages(
        user_id: &UserId, last_timestamp: i64, pool: &PgPool,
        tx_to_client: &mpsc::Sender<ServerMessage>,
    ) {
        // Query modified to use LEFT JOIN to find messages in groups where user is a member
        let query_result = sqlx::query!(
            r#"
            SELECT m.sender_id, m.target_user_id, m.target_group_id, m.is_broadcast, m.content_payload, m.created_at
            FROM messages m
            LEFT JOIN group_members gm ON m.target_group_id = gm.group_id
            WHERE m.created_at > $1
              AND (m.target_user_id = $2
                   OR m.is_broadcast = true
                   OR m.sender_id = $2
                   OR gm.user_id = $2)
            ORDER BY m.created_at ASC
            "#,
            last_timestamp,
            user_id.0
        )
            .fetch_all(pool)
            .await;

        if let Ok(records) = query_result {
            let mut batch = Vec::new();

            for record in records {
                if let Ok(payload) = postcard::from_bytes(&record.content_payload) {
                    let is_broadcast = record.is_broadcast.unwrap_or(false);

                    let target = if is_broadcast {
                        Target::Broadcast
                    } else if let Some(t_id) = record.target_user_id {
                        Target::User(UserId(t_id))
                    } else if let Some(g_id) = record.target_group_id {
                        Target::Group(protocol::GroupId(g_id))
                    } else {
                        continue;
                    };

                    let msg = ServerMessage::NewMessage {
                        from: UserId(record.sender_id.unwrap_or(0)),
                        target,
                        content: payload,
                        timestamp: record.created_at,
                    };

                    batch.push(msg);
                }
            }

            if !batch.is_empty() {
                let _ = tx_to_client.send(ServerMessage::SyncBatch(batch)).await;
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
