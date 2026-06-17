//! `picals-crawler download illust` 命令。

use crate::{
    cli::download::IllustArgs,
    error::{AppResult, CrawlerError},
};

pub async fn run(_args: IllustArgs) -> AppResult<()> {
    Err(CrawlerError::not_implemented("download illust 命令").into())
}
