//! `picals-crawler download keyword` 命令。

use crate::{
    cli::download::KeywordArgs,
    error::{AppResult, CrawlerError},
};

pub async fn run(_args: KeywordArgs) -> AppResult<()> {
    Err(CrawlerError::not_implemented("download keyword 命令").into())
}
