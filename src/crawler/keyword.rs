//! KeywordCrawler 骨架。

use crate::{
    crawler::CrawlContext,
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone)]
pub struct KeywordCrawler {
    pub query: String,
    pub context: CrawlContext,
}

impl KeywordCrawler {
    pub async fn run(&self) -> AppResult<()> {
        Err(CrawlerError::not_implemented("KeywordCrawler 主流程").into())
    }
}
