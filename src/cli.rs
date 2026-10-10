use std::{net::SocketAddr, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use ws2tcp_local_core::{AuthMode, Http3Mode, ProxyMode, SettingsOverrides};

pub const CONFIG_TEMPLATE: &str = include_str!("../examples/ws2tcp-local.toml");

#[derive(Debug, Parser)]
#[command(
    name = "ws2tcp-local",
    version,
    about = "Local HTTP proxy for ws2tcp-router"
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Print a TOML configuration template to stdout and exit.
    #[arg(long)]
    pub generate_config: bool,

    /// TOML config file path. CLI arguments override values loaded from this file.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Address to bind the local HTTP proxy to. Default: 127.0.0.1:3128.
    #[arg(long)]
    pub listen: Option<SocketAddr>,

    /// Also bind a local SOCKS5 (socks5h; hostnames are resolved by the remote
    /// side, not locally) proxy. Pass without a value to use 127.0.0.1:1080.
    /// Omitted entirely, no SOCKS5 listener is started.
    #[arg(long, num_args = 0..=1, default_missing_value = "127.0.0.1:1080")]
    pub socks_listen: Option<SocketAddr>,

    /// Base WebSocket gateway URL. Example: ws://1.2.3.4:8000
    #[arg(long)]
    pub gateway: Option<String>,

    /// HTTP Basic auth credential for the remote WebSocket gateway, formatted as user:pass.
    /// Falls back to WS2TCP_LOCAL_BASIC_AUTH when omitted.
    #[arg(long)]
    pub basic_auth: Option<String>,

    /// How to authenticate to the gateway, one method at a time. token: no health check, log in
    /// once for a short-lived access token (needs a ws2tcp-router with token authentication).
    /// basic: a health check on startup, then Basic Auth on every connection; kept for
    /// compatibility and being phased out. Default: token.
    #[arg(long)]
    pub auth_mode: Option<CliAuthMode>,

    /// TCP read buffer size. Default: 16384 bytes.
    #[arg(long)]
    pub buffer_size: Option<usize>,

    /// Logging filter, overriding RUST_LOG. Example: ws2tcp_local=debug
    #[arg(long)]
    pub log_level: Option<String>,

    /// Custom domain rules file, one Squid dstdomain entry per line.
    #[arg(long)]
    pub custom_domain_rules: Option<PathBuf>,

    /// Rule list refresh interval in seconds. Default: 60.
    #[arg(long)]
    pub rule_refresh_interval_secs: Option<u64>,

    /// Proxy mode: auto uses gfwlist rules, global proxies every request. Default: auto.
    #[arg(long)]
    pub proxy_mode: Option<CliProxyMode>,

    /// Skip verification of the remote WebSocket gateway TLS server certificate. Default: disabled.
    #[arg(long)]
    pub insecure: bool,

    /// How gateway tunnels use HTTP/3 (WebSocket over QUIC, RFC 9220): off is TCP only; on, or no
    /// value, tries HTTP/3 and falls back to HTTP/1.1 over TCP when that fails; only never falls
    /// back, so tunnels fail when HTTP/3 does not work. Needs a wss:// gateway. on is ignored
    /// with --upstream-proxy, and only makes the proxy refuse to start with one. Only tunnels
    /// are affected; token login still uses HTTPS over TCP. Default: off.
    #[arg(
        long,
        value_name = "MODE",
        num_args = 0..=1,
        default_missing_value = "on"
    )]
    pub http3: Option<CliHttp3Mode>,

    /// Send all outgoing connections through this proxy server: http://[user:pass@]host[:port],
    /// socks5h://[user:pass@]host[:port] (the proxy resolves hostnames) or socks5://... (resolved
    /// locally). That covers the gateway, requests that a routing rule sends direct, and the
    /// rule list downloads. Pass an empty value to override a proxy from --config. Default: none.
    #[arg(long, value_name = "URL")]
    pub upstream_proxy: Option<String>,

    /// Unix socket path. While the proxy runs it listens there: `ws2tcp-local netstat`,
    /// `config` and `reset-quic` with the same --control PATH talk to it. Omitted, the proxy creates no socket and the subcommands use
    /// $XDG_RUNTIME_DIR/ws2tcp-local.sock. Linux and other Unix systems only.
    #[arg(long, global = true, value_name = "PATH")]
    pub control: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show the HTTP/3 connections of a running proxy (started with --control), like netstat.
    Netstat {
        /// Print the snapshot as JSON instead of a table.
        #[arg(long)]
        json: bool,

        /// Print again every SECS seconds until interrupted.
        #[arg(long, value_name = "SECS")]
        watch: Option<u64>,
    },
    /// Read or change a setting of a running proxy (started with --control).
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Make a running proxy (started with --control) drop its cached HTTP/3 connection, so the
    /// next tunnel dials a new one.
    ResetQuic,
}

/// The socket the subcommands use when --control is omitted: `ws2tcp-local.sock` in
/// `$XDG_RUNTIME_DIR`, where the proxy is usually started with `--control`.
pub fn default_control_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty())?;
    Some(PathBuf::from(dir).join("ws2tcp-local.sock"))
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print a setting, or all of them when KEY is omitted.
    Get { key: Option<ConfigKey> },
    /// Change a setting for as long as the proxy runs; the config file is not touched.
    Set {
        key: ConfigKey,
        /// mode: auto or global. http3: off, on (HTTP/3 first, TCP as the fallback) or only.
        value: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ConfigKey {
    /// The proxy mode.
    Mode,
    /// How tunnels use HTTP/3.
    Http3,
}

impl ConfigKey {
    pub fn name(self) -> &'static str {
        match self {
            ConfigKey::Mode => "mode",
            ConfigKey::Http3 => "http3",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliHttp3Mode {
    Off,
    On,
    Only,
}

impl From<CliHttp3Mode> for Http3Mode {
    fn from(mode: CliHttp3Mode) -> Self {
        match mode {
            CliHttp3Mode::Off => Http3Mode::Off,
            CliHttp3Mode::On => Http3Mode::Preferred,
            CliHttp3Mode::Only => Http3Mode::Only,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliAuthMode {
    Basic,
    Token,
}

impl From<CliAuthMode> for AuthMode {
    fn from(mode: CliAuthMode) -> Self {
        match mode {
            CliAuthMode::Basic => Self::Basic,
            CliAuthMode::Token => Self::Token,
        }
    }
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
            socks_listen: args.socks_listen,
            gateway: args.gateway,
            basic_auth: args.basic_auth,
            auth_mode: args.auth_mode.map(Into::into),
            buffer_size: args.buffer_size,
            log_level: args.log_level,
            custom_domain_rules: args.custom_domain_rules,
            rule_refresh_interval_secs: args.rule_refresh_interval_secs,
            proxy_mode: args.proxy_mode.map(Into::into),
            insecure: args.insecure,
            http3: args.http3.map(Into::into),
            upstream_proxy: args.upstream_proxy,
        }
    }
}

#[cfg(test)]
mod tests {
    use ws2tcp_local_core::Settings;

    use super::*;

    #[test]
    fn parses_generate_config_without_gateway() {
        let args = Args::try_parse_from(["ws2tcp-local", "--generate-config"]).unwrap();

        assert!(args.generate_config);
        assert!(args.gateway.is_none());
    }

    #[test]
    fn config_template_contains_required_gateway() {
        assert!(CONFIG_TEMPLATE.contains("gateway = \"wss://example.com\""));
    }

    #[test]
    fn parses_the_upstream_proxy_into_the_settings() {
        let args = Args::try_parse_from([
            "ws2tcp-local",
            "--gateway",
            "wss://example.com",
            "--upstream-proxy",
            "socks5h://user:pass@127.0.0.1:1080",
        ])
        .unwrap();
        let settings = Settings::resolve(args.into()).unwrap();
        assert_eq!(
            settings.upstream_proxy.unwrap().to_string(),
            "socks5h://127.0.0.1:1080"
        );

        let args = Args::try_parse_from([
            "ws2tcp-local",
            "--gateway",
            "wss://example.com",
            "--upstream-proxy",
            "ftp://127.0.0.1:21",
        ])
        .unwrap();
        assert!(Settings::resolve(args.into()).is_err());
    }

    #[test]
    fn help_shows_parameter_defaults() {
        let error = Args::try_parse_from(["ws2tcp-local", "--help"]).unwrap_err();
        let help = error.to_string();

        assert!(help.contains("Default: 127.0.0.1:3128"));
        assert!(help.contains("Default: 16384 bytes"));
        assert!(help.contains("Default: 60"));
        assert!(help.contains("Default: auto"));
        assert!(help.contains("Default: disabled"));
        assert!(help.contains("--upstream-proxy"));
        assert!(help.contains("--insecure"));
        assert!(help.contains("--http3"));
        assert!(!help.contains("--verify-server-certificate"));
    }

    #[test]
    fn parses_the_netstat_subcommand() {
        let args = Args::try_parse_from([
            "ws2tcp-local",
            "netstat",
            "--control",
            "/tmp/ws2tcp.sock",
            "--json",
            "--watch",
            "2",
        ])
        .unwrap();

        assert_eq!(
            args.control.as_deref(),
            Some(std::path::Path::new("/tmp/ws2tcp.sock"))
        );
        match args.command {
            Some(Command::Netstat { json, watch }) => {
                assert!(json);
                assert_eq!(watch, Some(2));
            }
            _ => panic!("expected the netstat subcommand"),
        }
    }

    #[test]
    fn running_the_proxy_needs_no_subcommand() {
        let args = Args::try_parse_from([
            "ws2tcp-local",
            "--gateway",
            "wss://example.com",
            "--control",
            "/tmp/ws2tcp.sock",
        ])
        .unwrap();

        assert!(args.command.is_none());
        assert!(args.control.is_some());
    }

    fn http3_of(extra: &[&str]) -> (bool, bool) {
        let mut argv = vec!["ws2tcp-local", "--gateway", "wss://example.com"];
        argv.extend_from_slice(extra);
        let settings = Settings::resolve(Args::try_parse_from(argv).unwrap().into()).unwrap();
        (settings.http3, settings.http3_only)
    }

    #[test]
    fn http3_takes_off_on_or_only() {
        assert_eq!(http3_of(&[]), (false, false));
        assert_eq!(http3_of(&["--http3", "off"]), (false, false));
        assert_eq!(http3_of(&["--http3", "on"]), (true, false));
        assert_eq!(http3_of(&["--http3=only"]), (true, true));
        // Without a value it is on, as it was when it was a plain flag.
        assert_eq!(http3_of(&["--http3"]), (true, false));
        assert_eq!(http3_of(&["--http3", "--insecure"]), (true, false));
    }

    #[test]
    fn http3_only_is_no_longer_a_flag() {
        assert!(
            Args::try_parse_from([
                "ws2tcp-local",
                "--gateway",
                "wss://example.com",
                "--http3-only"
            ])
            .is_err()
        );
        assert!(
            Args::try_parse_from([
                "ws2tcp-local",
                "--gateway",
                "wss://example.com",
                "--http3=x"
            ])
            .is_err()
        );
    }
}
