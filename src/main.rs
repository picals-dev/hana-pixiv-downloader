use clap::Parser;
use eyre::Result;
use picals_crawler::{cli::Cli, commands};

#[tokio::main]
async fn main() -> Result<()> {
    init_logger();

    let cli = Cli::parse();
    commands::dispatch(cli).await
}

fn init_logger() {
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");

    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();
}
