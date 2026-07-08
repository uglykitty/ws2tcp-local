use std::sync::Arc;

use anyhow::{Result, anyhow};
use clap::Parser;
use tokio::net::TcpListener;
use tracing::{info, warn};

mod auth;
mod cli;
mod gateway;
mod http_proxy;
mod routing_rules;
mod settings;
mod tls;
mod tunnel;

use auth::remote_basic_auth;
use cli::Args;
use gateway::Gateway;
use routing_rules::RoutingRules;
use settings::Settings;
use tunnel::{Config, handle_client};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args = Args::parse();
    let settings = Settings::resolve(args)?;
    init_logging(settings.log_level.as_deref())?;
    let routing_rules = RoutingRules::load(
        settings.proxy_mode,
        settings.custom_domain_rules.as_deref(),
        settings.rule_refresh_interval,
    )
    .await;

    let config = Arc::new(Config {
        gateway: Gateway::parse(&settings.gateway)?,
        basic_auth: remote_basic_auth(settings.basic_auth)?,
        buffer_size: settings.buffer_size,
        routing_rules,
        verify_server_certificate: settings.verify_server_certificate,
    });
    let listener = TcpListener::bind(settings.listen)
        .await
        .map_err(|err| anyhow!("failed to bind {}: {err}", settings.listen))?;

    info!(
        listen = %settings.listen,
        gateway = %config.gateway.base(),
        verify_server_certificate = config.verify_server_certificate,
        rule_refresh_interval_secs = settings.rule_refresh_interval.as_secs(),
        routing_rules = %config.routing_rules,
        routing_rules_detail = %config.routing_rules.describe(),
        "listening"
    );
    if !config.verify_server_certificate {
        warn!(
            "remote gateway TLS server certificate verification is disabled; use --verify-server-certificate or verify_server_certificate = true to enable it"
        );
    }

    loop {
        let (stream, peer_addr) = listener
            .accept()
            .await
            .map_err(|err| anyhow!("accept failed: {err}"))?;
        let config = Arc::clone(&config);

        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, peer_addr, config).await {
                warn!(%peer_addr, error = %format_args!("{err:#}"), "connection closed with error");
            }
        });
    }
}

fn init_logging(log_level: Option<&str>) -> Result<()> {
    let filter = match log_level {
        Some(filter) => filter.to_owned(),
        None => std::env::var("RUST_LOG").unwrap_or_else(|_| "ws2tcp_local=info".to_owned()),
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|err| anyhow!("failed to initialize logging: {err}"))
}
