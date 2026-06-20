//! IllustCrawler。

use std::time::Duration;

use crate::{
    auth::Credential,
    collector::{PixivCollector, resolve_base_url, selector::select_page_original_urls},
    config::ResolvedDownloadOptions,
    crawler::CrawlContext,
    crawler::shared::{download_urls, export_tags_json},
    downloader::DownloadResult,
    error::AppResult,
    utils::retry::retry_async,
};
use url::Url;

#[derive(Debug, Clone)]
pub struct IllustCrawler {
    pub illust_id: String,
    pub context: CrawlContext,
    base_url: Url,
}

impl IllustCrawler {
    pub fn new(
        illust_id: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        Ok(Self {
            illust_id,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        illust_id: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        Self {
            illust_id,
            context: CrawlContext::new(credential, options),
            base_url,
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let collector = self.build_collector()?;
        let pages = retry_async(
            self.context.options.retry,
            Duration::from_millis(200),
            |_| {
                let collector = collector.clone();
                let illust_id = self.illust_id.clone();

                async move { collector.fetch_illust_pages(&illust_id).await }
            },
        )
        .await?;

        let urls = select_page_original_urls(&pages)?;
        let output_directory = self.context.options.directory.join(&self.illust_id);
        let mut result = download_urls(
            self.context.options.clone(),
            output_directory.clone(),
            self.base_url.clone(),
            &urls,
        )
        .await?;

        let failed_units = export_tags_json(
            &collector,
            std::slice::from_ref(&self.illust_id),
            &output_directory,
            &self.context.options,
        )
        .await;
        result.failed += failed_units;
        result.total += failed_units;

        Ok(result)
    }

    fn build_collector(&self) -> AppResult<PixivCollector> {
        PixivCollector::new_with_base_url(
            &self.context.options,
            &self.context.credential,
            self.base_url.clone(),
        )
    }
}
