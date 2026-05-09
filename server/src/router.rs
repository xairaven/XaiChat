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
                    target, message, ..
                } => {
                    // Logic to find the recipient channel and send the message
                    match target {
                        Target::User(target_user_id) => {
                            if let Some(client_channel) =
                                self.active_connections.get(&target_user_id)
                            {
                                // Ignore error if the client disconnected right before we send
                                let _ = client_channel.send(message).await;
                            } else {
                                // TODO: Here we will save it to Postgres offline queue later
                                println!(
                                    "User {} is offline, message not delivered immediately",
                                    target_user_id
                                );
                            }
                        },
                        Target::Broadcast => {
                            for channel in self.active_connections.values() {
                                let _ = channel.send(message.clone()).await;
                            }
                        },
                        Target::Group(_) => {
                            // TODO: Here we will query DB for members and route to their channels
                            println!("Group routing not yet implemented");
                        },
                    }
                },
            }
        }
    }
}
