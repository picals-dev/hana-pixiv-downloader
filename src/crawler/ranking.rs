//! RankingCrawler 骨架。

use crate::{
    crawler::CrawlContext,
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone)]
pub struct RankingCrawler {
    pub mode: String,
    pub context: CrawlContext,
}

impl RankingCrawler {
    pub async fn run(&self) -> AppResult<()> {
        Err(CrawlerError::not_implemented("RankingCrawler 主流程").into())
    }
}
