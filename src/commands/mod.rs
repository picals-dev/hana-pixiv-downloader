//! 命令调度层。

mod config_cmd;
mod download_bookmark;
mod download_common;
mod download_direct;
mod download_illust;
mod download_keyword;
mod download_ranking;
mod download_user;
mod organize;
mod prompt_support;
mod retry_cmd;
mod setup;
mod update;

use crate::{
    cli::{Cli, Command, config::ConfigAction, download::DownloadSubcommand},
    error::AppResult,
};

pub async fn dispatch(cli: Cli) -> AppResult<()> {
    match cli.command {
        Command::Setup => setup::run().await,
        Command::Download(download) => match download.target {
            Some(DownloadSubcommand::User(args)) => download_user::run(args).await,
            Some(DownloadSubcommand::Keyword(args)) => download_keyword::run(args).await,
            Some(DownloadSubcommand::Ranking(args)) => download_ranking::run(args).await,
            Some(DownloadSubcommand::Illust(args)) => download_illust::run(args).await,
            Some(DownloadSubcommand::Bookmark(args)) => download_bookmark::run(args).await,
            None => download_direct::run(download.direct).await,
        },
        Command::Retry(args) => retry_cmd::run(args).await,
        Command::Config(config) => match config.action {
            ConfigAction::Show => config_cmd::show().await,
            ConfigAction::Clean => config_cmd::clean().await,
            ConfigAction::Set(args) => config_cmd::set(args).await,
        },
        Command::Organize(args) => organize::run(args).await,
        Command::Update => update::run().await,
    }
}
