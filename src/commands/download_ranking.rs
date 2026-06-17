//! `picals-crawler download ranking` 命令。

use crate::{
    cli::download::RankingArgs,
    error::{AppResult, CrawlerError},
};

pub async fn run(_args: RankingArgs) -> AppResult<()> {
    Err(CrawlerError::not_implemented("download ranking 命令").into())
}
