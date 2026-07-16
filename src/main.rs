use anyhow::Result;
use clap::Parser;
use tracing::warn;
use ws2tcp_local_core::{Settings, init_logging, run_proxy};

mod cli;

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
