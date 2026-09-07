use anyhow::{Result, anyhow};
use clap::Parser;
use tracing::warn;
use ws2tcp_local_core::{Settings, run_proxy};

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
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init()
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
    let settings = Settings::resolve(args.into())?;
    let basic_auth_from_environment = !basic_auth_from_cli
        && settings.basic_auth.is_none()
        && std::env::var("WS2TCP_LOCAL_BASIC_AUTH").is_ok();

    init_logging(settings.log_level.as_deref())?;
    warn_if_basic_auth_may_leak(basic_auth_from_cli, basic_auth_from_environment);

    run_proxy(settings, async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %err, "failed to listen for Ctrl+C");
        }
    })
    .await
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
