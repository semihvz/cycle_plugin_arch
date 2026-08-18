mod analyzer;
mod client;
mod config;
mod models;
mod service;
mod utils;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let mut service = service::ScoutService::new();
    tracing::info!("USDT pariteleri icin tarama servisi baslatildi...");

    if let Err(err) = service.start().await {
        tracing::error!("Servis baslatilamadi: {}", err);
        std::process::exit(1);
    }

    tokio::signal::ctrl_c().await.ok();
    service.stop().await;
}
