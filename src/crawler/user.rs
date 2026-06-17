//! UserCrawler 骨架。

use crate::{
    auth::Credential,
    config::ResolvedDownloadOptions,
    crawler::CrawlContext,
    downloader::DownloadResult,
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone)]
pub struct UserCrawler {
    pub artist_id: String,
    pub context: CrawlContext,
}

impl UserCrawler {
    pub fn new(
        artist_id: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
    ) -> Self {
        Self {
            artist_id,
            context: CrawlContext::new(credential, options),
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        Err(CrawlerError::not_implemented("UserCrawler 主流程").into())
    }
}
