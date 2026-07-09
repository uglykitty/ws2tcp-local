use anyhow::Result;
use clap::Parser;
use ws2tcp_local_core::{Settings, init_logging, run_proxy};

mod cli;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = cli::Args::parse();
    let settings = Settings::resolve(args.into())?;
    init_logging(settings.log_level.as_deref())?;
    run_proxy(settings, async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %err, "failed to listen for Ctrl+C");
        }
    })
    .await
}
