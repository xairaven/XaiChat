use crate::connection::ConnectionActor;
use anyhow::{Context, Result};
use router::{Router, RouterEvent};
use sqlx::postgres::PgPoolOptions;
use std::env;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

const LISTEN_ON: &str = "127.0.0.1:8080";
const POSTGRES_MAX_CONNECTIONS: u32 = 5;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables for PostgreSQL connection
    dotenvy::dotenv().ok();
    let db_url = env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    let pool = PgPoolOptions::new()
        .max_connections(POSTGRES_MAX_CONNECTIONS)
        .connect(&db_url)
        .await?;

    let (router_tx, router_rx) = mpsc::channel::<RouterEvent>(1024);

    let router = Router::new(router_rx, pool.clone());
    tokio::spawn(async move {
        router.run().await;
    });

    let listener = TcpListener::bind(LISTEN_ON).await?;
    println!("Server listening on {}", LISTEN_ON);

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from: {}", addr);

        let router_tx_clone = router_tx.clone();
        let pool_clone = pool.clone();

        tokio::spawn(async move {
            let actor_result =
                ConnectionActor::handle_connection(socket, router_tx_clone, pool_clone)
                    .await;
            if let Err(e) = actor_result {
                eprintln!("Connection error: {}", e);
            }
        });
    }
}

mod connection;
mod router;
