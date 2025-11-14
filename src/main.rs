mod db;
mod http;
mod smtp;

use anyhow::Result;
use std::net::SocketAddr;
use tokio::task;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tempmail_rs=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/tempmail".to_string());
    
    let smtp_domain = std::env::var("SMTP_DOMAIN")
        .unwrap_or_else(|_| "tempmail.local".to_string());

    // Initialize database
    tracing::info!("Connecting to database...");
    let db = db::Db::new(&database_url).await?;
    db.run_migrations().await?;
    tracing::info!("Database connected and migrations applied");

    // Clone db for both servers
    let smtp_db = db.clone();
    let http_db = db.clone();

    // Start SMTP server
    let smtp_addr: SocketAddr = "0.0.0.0:2525".parse()?;
    let smtp_domain_clone = smtp_domain.clone();
    
    tracing::info!("Starting SMTP server on {}", smtp_addr);
    let smtp_handle = task::spawn(async move {
        if let Err(e) = smtp::start_server(smtp_addr, smtp_domain_clone, smtp_db).await {
            tracing::error!("SMTP server error: {}", e);
        }
    });

    // Start HTTP server
    let http_addr: SocketAddr = "0.0.0.0:3000".parse()?;
    tracing::info!("Starting HTTP server on {}", http_addr);
    tracing::info!("Domain: {}", smtp_domain);
    
    let http_handle = task::spawn(async move {
        if let Err(e) = http::start_server(http_addr, smtp_domain, http_db).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    // Wait for both servers
    tokio::select! {
        _ = smtp_handle => tracing::info!("SMTP server stopped"),
        _ = http_handle => tracing::info!("HTTP server stopped"),
    }

    Ok(())
}
