//! BookmarkCrawler 骨架。

use crate::{
    crawler::CrawlContext,
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone)]
pub struct BookmarkCrawler {
    pub context: CrawlContext,
}

impl BookmarkCrawler {
    pub async fn run(&self) -> AppResult<()> {
        Err(CrawlerError::not_implemented("BookmarkCrawler 主流程").into())
    }
}
