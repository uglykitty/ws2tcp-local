use anyhow::{Result, anyhow};
use clap::Parser;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use ws2tcp_local_core::{GatewayCheckError, ProxyMode, Settings, run_proxy_with_mode_updates};

mod cli;

fn init_logging(log_level: Option<&str>) -> Result<()> {
    let filter = match log_level {
        Some(filter) => filter.to_owned(),
        None => std::env::var("RUST_LOG").unwrap_or_else(|_| "ws2tcp_local=info".to_owned()),
    };

    // When systemd captures our stdout/stderr into the journal (StandardOutput=journal,
    // the default since systemd 246), it sets JOURNAL_STREAM and journalctl already
    // prepends its own reception timestamp to every line. Emitting our own timestamp too
    // would show up as a duplicate, so drop it in that case; interactive terminal runs
    // keep the timestamp since JOURNAL_STREAM won't be set there.
    let running_under_systemd_journal = std::env::var_os("JOURNAL_STREAM").is_some();

    let init_result = if running_under_systemd_journal {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .without_time()
            .try_init()
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).try_init()
    };

    init_result.map_err(|err| anyhow!("failed to initialize logging: {err}"))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = cli::Args::parse();
    if args.generate_config {
        print!("{}", cli::CONFIG_TEMPLATE);
        return Ok(());
    }

    let basic_auth_from_cli = args.basic_auth.is_some();
    let mut settings = Settings::resolve(args.into())?;
    settings.add_header(
        "User-Agent",
        &format!("ws2tcp-local/{}", env!("CARGO_PKG_VERSION")),
    )?;
    let basic_auth_from_environment = !basic_auth_from_cli
        && settings.basic_auth.is_none()
        && std::env::var("WS2TCP_LOCAL_BASIC_AUTH").is_ok();

    init_logging(settings.log_level.as_deref())?;
    warn_if_basic_auth_may_leak(basic_auth_from_cli, basic_auth_from_environment);

    let (mode_updates_tx, mode_updates_rx) = mpsc::unbounded_channel();
    spawn_proxy_mode_toggle(settings.proxy_mode, mode_updates_tx);

    let result = run_proxy_with_mode_updates(
        settings,
        async {
            if let Err(err) = tokio::signal::ctrl_c().await {
                tracing::warn!(error = %err, "failed to listen for Ctrl+C");
            }
        },
        mode_updates_rx,
    )
    .await;

    // The gateway is checked before anything is served; tell the user what to fix and exit
    // instead of starting a proxy that could not tunnel anything.
    if let Err(err) = &result
        && let Some(check_error) = err.downcast_ref::<GatewayCheckError>()
    {
        error!("{check_error}");
        std::process::exit(1);
    }

    result
}

fn toggled(mode: ProxyMode) -> ProxyMode {
    match mode {
        ProxyMode::Auto => ProxyMode::Global,
        ProxyMode::Global => ProxyMode::Auto,
    }
}

fn mode_name(mode: ProxyMode) -> &'static str {
    match mode {
        ProxyMode::Auto => "auto",
        ProxyMode::Global => "global",
    }
}

/// Toggle between auto and global proxy mode every time the process receives SIGUSR1.
#[cfg(unix)]
fn spawn_proxy_mode_toggle(initial: ProxyMode, updates: mpsc::UnboundedSender<ProxyMode>) {
    use tokio::signal::unix::{SignalKind, signal};

    // Register the handler right away, before the gateway login and rule loading run, so an
    // early SIGUSR1 is queued instead of hitting the default action and terminating the process.
    let mut sigusr1 = match signal(SignalKind::user_defined1()) {
        Ok(sigusr1) => sigusr1,
        Err(err) => {
            warn!(error = %err, "failed to listen for SIGUSR1; the proxy mode cannot be toggled by signal");
            return;
        }
    };

    tokio::spawn(async move {
        let mut mode = initial;
        while sigusr1.recv().await.is_some() {
            mode = toggled(mode);
            info!(
                mode = mode_name(mode),
                "received SIGUSR1; switching proxy mode"
            );
            if updates.send(mode).is_err() {
                break;
            }
        }
    });
}

#[cfg(not(unix))]
fn spawn_proxy_mode_toggle(_initial: ProxyMode, _updates: mpsc::UnboundedSender<ProxyMode>) {}

fn warn_if_basic_auth_may_leak(basic_auth_from_cli: bool, basic_auth_from_environment: bool) {
    if basic_auth_from_cli {
        warn!(
            "Basic Auth credentials supplied with --basic-auth may be exposed in shell history and process arguments; continuing startup"
        );
    } else if basic_auth_from_environment {
        warn!(
            "Basic Auth credentials supplied through WS2TCP_LOCAL_BASIC_AUTH may be exposed in shell history or the process environment; continuing startup"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_between_auto_and_global() {
        assert_eq!(toggled(ProxyMode::Auto), ProxyMode::Global);
        assert_eq!(toggled(ProxyMode::Global), ProxyMode::Auto);
    }
}
