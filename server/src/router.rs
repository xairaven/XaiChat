use protocol::{ServerMessage, Target, UserId};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tokio::sync::mpsc;

// Event sent from a Connection Actor to the Router Actor
#[derive(Debug)]
pub enum RouterEvent {
    // New user successfully authenticated
    UserConnected {
        user_id: UserId,
        sender: mpsc::Sender<ServerMessage>,
    },
    // User dropped connection
    UserDisconnected {
        user_id: UserId,
    },
    // User wants to route a message to someone or a group
    RouteMessage {
        from: UserId,
        target: Target,
        message: ServerMessage,
    },
}

// The core engine that holds state without Mutex locks
pub struct Router {
    // Channel to receive events from all active connections
    receiver: mpsc::Receiver<RouterEvent>,
    // Registry of online users and their direct channels
    active_connections: HashMap<UserId, mpsc::Sender<ServerMessage>>,
    // Database pool
    pool: Pool<Postgres>,
}

impl Router {
    pub fn new(receiver: mpsc::Receiver<RouterEvent>, pool: Pool<Postgres>) -> Self {
        Self {
            receiver,
            active_connections: HashMap::new(),
            pool,
        }
    }

    // Main event loop for the Router Actor
    pub async fn run(mut self) {
        // Process events sequentially as they arrive from connections
        while let Some(event) = self.receiver.recv().await {
            match event {
                RouterEvent::UserConnected { user_id, sender } => {
                    self.active_connections.insert(user_id.clone(), sender);
                    println!("User connected: {}", user_id);
                },
                RouterEvent::UserDisconnected { user_id } => {
                    self.active_connections.remove(&user_id);
                    println!("User disconnected: {}", user_id);
                },
                RouterEvent::RouteMessage {
                    from,
                    target,
                    message,
                } => {
                    match target {
                        Target::User(ref target_user_id) => {
                            if let Some(client_channel) =
                                self.active_connections.get(target_user_id)
                            {
                                // Online: Send immediately
                                let _ = client_channel.send(message.clone()).await;
                            } else {
                                // Offline: Just print log, DB logic handles saving below
                                println!(
                                    "User {} is offline. Message will be queued.",
                                    target_user_id.0
                                );
                            }

                            // DB INSERTS MUST NOT BLOCK THE ROUTER
                            // Spawn a background task to save the message to Postgres
                            self.save_message(&from, Some(target_user_id), &message);
                        },
                        Target::Broadcast => {
                            // Send to everyone who is online
                            for channel in self.active_connections.values() {
                                let _ = channel.send(message.clone()).await;
                            }
                            self.save_message(&from, None, &message);
                        },
                        Target::Group(_) => {
                            // TODO: Group routing logic and DB saving
                            println!("Group routing not yet implemented");
                        },
                    }
                },
            }
        }
    }

    fn save_message(
        &self, from: &UserId, target: Option<&UserId>, message: &ServerMessage,
    ) {
        // Save broadcast message to DB
        let pool = self.pool.clone();
        let msg_clone = message.clone();
        let from_id = from.0;
        let target_id = target.map(|user_id| user_id.0);
        let is_broadcast = target_id.is_none();

        tokio::spawn(async move {
            if let ServerMessage::NewMessage {
                content, timestamp, ..
            } = msg_clone
                && let Ok(payload_bytes) = postcard::to_stdvec(&content)
            {
                let _ = sqlx::query!(
                        "INSERT INTO messages (sender_id, target_user_id, is_broadcast, content_payload, created_at) \
                        VALUES ($1, $2, $3, $4, $5)",
                        from_id,
                        target_id,
                        is_broadcast,
                        payload_bytes,
                        timestamp
                    )
                    .execute(&pool)
                    .await;
            }
        });
    }
}
