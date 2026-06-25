use clap::Parser;
use eyre::Result;
use picals_crawler::{cli::Cli, commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logger(cli.global.verbose);

    commands::dispatch(cli).await
}

fn init_logger(verbose: bool) {
    let default_level = if verbose { "debug" } else { "info" };
    let env = env_logger::Env::default().filter_or("RUST_LOG", default_level);

    env_logger::Builder::from_env(env)
        .format_timestamp(None)
        .init();
}
