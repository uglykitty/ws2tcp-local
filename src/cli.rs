use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;

pub(crate) const DEFAULT_BUFFER_SIZE: usize = 16 * 1024;
pub(crate) const DEFAULT_LISTEN: &str = "127.0.0.1:8000";

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
}
