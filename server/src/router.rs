use protocol::{GroupId, ServerMessage, Target, UserId};
use sqlx::{Pool, Postgres};
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;

// Event sent from a Connection Actor to the Router Actor
#[derive(Debug)]
pub enum RouterEvent {
    // New user successfully authenticated
    UserConnected {
        user_id: UserId,
        groups: Vec<GroupId>,
        sender: mpsc::Sender<ServerMessage>,
    },
    // User dropped connection
    UserDisconnected {
        user_id: UserId,
    },
    // User wants to route a message
    RouteMessage {
        from: UserId,
        target: Target,
        message: ServerMessage,
    },
    // Dynamically subscribe to a group while online
    UserJoinedGroup {
        user_id: UserId,
        group_id: GroupId,
    },
}

// The core engine that holds state without Mutex locks
pub struct Router {
    // Channel to receive events from all active connections
    receiver: mpsc::Receiver<RouterEvent>,

    // Maps a user to their active TCP connection channel
    sessions: HashMap<UserId, mpsc::Sender<ServerMessage>>,
    // Pub/Sub: Maps a Group ID to a set of User IDs
    group_subscriptions: HashMap<GroupId, HashSet<UserId>>,

    // Database pool
    pool: Pool<Postgres>,
}

impl Router {
    pub fn new(receiver: mpsc::Receiver<RouterEvent>, pool: Pool<Postgres>) -> Self {
        Self {
            receiver,
            sessions: HashMap::new(),
            group_subscriptions: HashMap::new(),
            pool,
        }
    }

    // Main event loop for the Router Actor
    pub async fn run(mut self) {
        // Process events sequentially as they arrive from connections
        while let Some(event) = self.receiver.recv().await {
            match event {
                RouterEvent::UserConnected {
                    user_id,
                    groups,
                    sender,
                } => {
                    self.sessions.insert(user_id.clone(), sender.clone());

                    // Subscribe user to all their groups in RAM
                    for group_id in groups {
                        self.group_subscriptions
                            .entry(group_id)
                            .or_default()
                            .insert(user_id.clone());
                    }

                    // Tell others that we are online
                    let presence_msg = ServerMessage::Presence {
                        user_id: user_id.clone(),
                        online: true,
                    };
                    for (uid, channel) in &self.sessions {
                        if uid != &user_id {
                            let _ = channel.send(presence_msg.clone()).await;
                        }
                    }

                    // Tell us about everyone who was already online before us
                    for uid in self.sessions.keys() {
                        if uid != &user_id {
                            let presence_msg = ServerMessage::Presence {
                                user_id: uid.clone(),
                                online: true,
                            };
                            let _ = sender.send(presence_msg).await;
                        }
                    }

                    println!("User connected: {}", user_id);
                },
                RouterEvent::UserDisconnected { user_id } => {
                    // We only remove from active sessions.
                    // Leaving them in `group_subscriptions` acts as an offline cache.
                    self.sessions.remove(&user_id);

                    let presence_msg = ServerMessage::Presence {
                        user_id: user_id.clone(),
                        online: false,
                    };
                    for channel in self.sessions.values() {
                        let _ = channel.send(presence_msg.clone()).await;
                    }

                    println!("User disconnected: {}", user_id);
                },
                RouterEvent::UserJoinedGroup { user_id, group_id } => {
                    self.group_subscriptions
                        .entry(group_id)
                        .or_default()
                        .insert(user_id);
                },
                RouterEvent::RouteMessage {
                    from,
                    target,
                    message,
                } => {
                    // Background non-blocking DB save
                    self.save_message(&from, &target, &message);

                    // Instant RAM-based routing
                    match target {
                        Target::User(ref target_user_id) => {
                            if let Some(client_channel) =
                                self.sessions.get(target_user_id)
                            {
                                let _ = client_channel.send(message).await;
                            }
                        },
                        Target::Broadcast => {
                            for channel in self.sessions.values() {
                                let _ = channel.send(message.clone()).await;
                            }
                        },
                        Target::Group(ref group_id) => {
                            if let Some(members) = self.group_subscriptions.get(group_id)
                            {
                                for uid in members {
                                    // Only send if the user is currently online
                                    if let Some(client_channel) = self.sessions.get(uid) {
                                        let _ =
                                            client_channel.send(message.clone()).await;
                                    }
                                }
                            }
                        },
                    }
                },
            }
        }
    }

    fn save_message(&self, from: &UserId, target: &Target, message: &ServerMessage) {
        let pool = self.pool.clone();
        let msg_clone = message.clone();
        let from_id = from.0;

        let mut target_user_id = None;
        let mut target_group_id = None;
        let mut is_broadcast = false;

        match target {
            Target::User(uid) => target_user_id = Some(uid.0),
            Target::Group(gid) => target_group_id = Some(gid.0),
            Target::Broadcast => is_broadcast = true,
        }

        tokio::spawn(async move {
            if let ServerMessage::NewMessage {
                content, timestamp, ..
            } = msg_clone
                && let Ok(payload_bytes) = postcard::to_stdvec(&content)
            {
                let _ = sqlx::query!(
                    "INSERT INTO messages (sender_id, target_user_id, target_group_id, is_broadcast, content_payload, created_at) \
                    VALUES ($1, $2, $3, $4, $5, $6)",
                    from_id, target_user_id, target_group_id, is_broadcast, payload_bytes, timestamp
                )
                    .execute(&pool)
                    .await;
            }
        });
    }
}
