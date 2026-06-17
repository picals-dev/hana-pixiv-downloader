//! IllustCrawler 骨架。

use crate::{
    crawler::CrawlContext,
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone)]
pub struct IllustCrawler {
    pub illust_id: String,
    pub context: CrawlContext,
}

impl IllustCrawler {
    pub async fn run(&self) -> AppResult<()> {
        Err(CrawlerError::not_implemented("IllustCrawler 主流程").into())
    }
}
