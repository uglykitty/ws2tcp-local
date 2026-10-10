use std::sync::Arc;

use anyhow::{Result, anyhow};
use clap::Parser;
use tokio::sync::mpsc;
use tracing::{error, warn};
use ws2tcp_local_core::{GatewayCheckError, Settings, run_proxy_with_updates};

mod cli;
mod control;
mod netstat;

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

    if let Some(command) = &args.command {
        let path = args
            .control
            .clone()
            .or_else(cli::default_control_path)
            .ok_or_else(|| {
                anyhow!("this command needs --control PATH, the socket of the running proxy")
            })?;
        let path = path.as_path();
        return match command {
            cli::Command::Netstat { json, watch } => netstat::run(path, *json, *watch).await,
            cli::Command::Config { action } => {
                let command = match action {
                    cli::ConfigAction::Get { key: None } => "get".to_owned(),
                    cli::ConfigAction::Get { key: Some(key) } => format!("get {}", key.name()),
                    cli::ConfigAction::Set { key, value } => {
                        format!("set {} {}", key.name(), value)
                    }
                };
                let reply = control::request(path, &command).await?;
                print!("{reply}");
                if reply.starts_with("error") {
                    std::process::exit(1);
                }
                Ok(())
            }
            cli::Command::ResetQuic => {
                print!("{}", control::request(path, "reset-quic").await?);
                Ok(())
            }
        };
    }

    let control_path = args.control.clone();
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
    let (http3_updates_tx, http3_updates_rx) = mpsc::unbounded_channel();
    let controller = Arc::new(control::Controller::new(
        &settings,
        mode_updates_tx,
        http3_updates_tx,
    ));
    let _control = match control_path {
        Some(path) => Some(control::serve(path, controller)?),
        None => None,
    };

    let result = run_proxy_with_updates(
        settings,
        async {
            if let Err(err) = tokio::signal::ctrl_c().await {
                tracing::warn!(error = %err, "failed to listen for Ctrl+C");
            }
        },
        mode_updates_rx,
        http3_updates_rx,
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
