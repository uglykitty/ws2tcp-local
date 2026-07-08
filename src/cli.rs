use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, ValueEnum};
use serde::Deserialize;

pub(crate) const DEFAULT_BUFFER_SIZE: usize = 16 * 1024;
pub(crate) const DEFAULT_LISTEN: &str = "127.0.0.1:8000";
pub(crate) const DEFAULT_RULE_REFRESH_INTERVAL_SECS: u64 = 60;

#[derive(Debug, Parser)]
#[command(
    name = "ws2tcp-local",
    version,
    about = "Local HTTP proxy for ws2tcp-router"
)]
pub(crate) struct Args {
    /// TOML config file path. CLI arguments override values loaded from this file.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,

    /// Address to bind the local HTTP proxy to.
    #[arg(long)]
    pub(crate) listen: Option<SocketAddr>,

    /// Base WebSocket gateway URL. Example: ws://1.2.3.4:8000
    #[arg(long)]
    pub(crate) gateway: Option<String>,

    /// HTTP Basic auth credential for the remote WebSocket gateway, formatted as user:pass.
    /// Falls back to WS2TCP_LOCAL_BASIC_AUTH when omitted.
    #[arg(long)]
    pub(crate) basic_auth: Option<String>,

    /// TCP read buffer size.
    #[arg(long)]
    pub(crate) buffer_size: Option<usize>,

    /// Logging filter, overriding RUST_LOG. Example: ws2tcp_local=debug
    #[arg(long)]
    pub(crate) log_level: Option<String>,

    /// Custom domain rules file, one Squid dstdomain entry per line.
    #[arg(long)]
    pub(crate) custom_domain_rules: Option<PathBuf>,

    /// Rule list refresh interval in seconds.
    #[arg(long)]
    pub(crate) rule_refresh_interval_secs: Option<u64>,

    /// Proxy mode: auto uses gfwlist rules, global proxies every request.
    #[arg(long)]
    pub(crate) proxy_mode: Option<ProxyMode>,

    /// Verify the remote WebSocket gateway TLS server certificate.
    #[arg(long)]
    pub(crate) verify_server_certificate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ProxyMode {
    Auto,
    Global,
}
