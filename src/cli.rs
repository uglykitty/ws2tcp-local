use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, ValueEnum};
use ws2tcp_local_core::{ProxyMode, SettingsOverrides};

#[derive(Debug, Parser)]
#[command(
    name = "ws2tcp-local",
    version,
    about = "Local HTTP proxy for ws2tcp-router"
)]
pub struct Args {
    /// TOML config file path. CLI arguments override values loaded from this file.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Address to bind the local HTTP proxy to.
    #[arg(long)]
    pub listen: Option<SocketAddr>,

    /// Base WebSocket gateway URL. Example: ws://1.2.3.4:8000
    #[arg(long)]
    pub gateway: Option<String>,

    /// HTTP Basic auth credential for the remote WebSocket gateway, formatted as user:pass.
    /// Falls back to WS2TCP_LOCAL_BASIC_AUTH when omitted.
    #[arg(long)]
    pub basic_auth: Option<String>,

    /// TCP read buffer size.
    #[arg(long)]
    pub buffer_size: Option<usize>,

    /// Logging filter, overriding RUST_LOG. Example: ws2tcp_local=debug
    #[arg(long)]
    pub log_level: Option<String>,

    /// Custom domain rules file, one Squid dstdomain entry per line.
    #[arg(long)]
    pub custom_domain_rules: Option<PathBuf>,

    /// Rule list refresh interval in seconds.
    #[arg(long)]
    pub rule_refresh_interval_secs: Option<u64>,

    /// Proxy mode: auto uses gfwlist rules, global proxies every request.
    #[arg(long)]
    pub proxy_mode: Option<CliProxyMode>,

    /// Verify the remote WebSocket gateway TLS server certificate.
    #[arg(long)]
    pub verify_server_certificate: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliProxyMode {
    Auto,
    Global,
}

impl From<CliProxyMode> for ProxyMode {
    fn from(mode: CliProxyMode) -> Self {
        match mode {
            CliProxyMode::Auto => Self::Auto,
            CliProxyMode::Global => Self::Global,
        }
    }
}

impl From<Args> for SettingsOverrides {
    fn from(args: Args) -> Self {
        Self {
            config: args.config,
            listen: args.listen,
            gateway: args.gateway,
            basic_auth: args.basic_auth,
            buffer_size: args.buffer_size,
            log_level: args.log_level,
            custom_domain_rules: args.custom_domain_rules,
            rule_refresh_interval_secs: args.rule_refresh_interval_secs,
            proxy_mode: args.proxy_mode.map(Into::into),
            verify_server_certificate: args.verify_server_certificate,
        }
    }
}
