//! `picals-crawler download bookmark` 命令。

use crate::{
    cli::download::BookmarkArgs,
    error::{AppResult, CrawlerError},
};

pub async fn run(_args: BookmarkArgs) -> AppResult<()> {
    Err(CrawlerError::not_implemented("download bookmark 命令").into())
}
