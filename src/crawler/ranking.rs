//! RankingCrawler。

use std::time::Duration;

use crate::{
    auth::Credential,
    collector::{PixivCollector, resolve_base_url, selector::select_ranking_illust_ids},
    config::ResolvedDownloadOptions,
    crawler::CrawlContext,
    crawler::shared::{
        collect_image_urls_for_illust_ids, download_urls, export_tags_json, sort_illust_ids,
    },
    downloader::DownloadResult,
    error::AppResult,
    utils::retry::retry_async,
};
use url::Url;

#[derive(Debug, Clone)]
pub struct RankingCrawler {
    pub mode: String,
    pub context: CrawlContext,
    base_url: Url,
}

impl RankingCrawler {
    pub fn new(
        mode: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        Ok(Self {
            mode,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        mode: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        Self {
            mode,
            context: CrawlContext::new(credential, options),
            base_url,
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let collector = self.build_collector()?;
        let target_count = self.context.options.count;
        let page_count = if target_count == 0 {
            1
        } else {
            target_count.div_ceil(50)
        };
        let mut illust_ids = Vec::new();

        for page in 1..=page_count {
            let value = retry_async(
                self.context.options.retry,
                Duration::from_millis(200),
                |_| {
                    let collector = collector.clone();
                    let mode = self.mode.clone();

                    async move { collector.fetch_ranking_page(&mode, page).await }
                },
            )
            .await?;
            illust_ids.extend(select_ranking_illust_ids(&value)?);
        }

        sort_illust_ids(&mut illust_ids, self.context.options.sort)?;
        illust_ids.dedup();
        if target_count > 0 && illust_ids.len() > target_count {
            illust_ids.truncate(target_count);
        }

        let (urls, mut failed_units) = collect_image_urls_for_illust_ids(
            &collector,
            illust_ids.clone(),
            &self.context.options,
        )
        .await;
        let output_directory = self
            .context
            .options
            .directory
            .join(format!("ranking-{}", self.mode));
        failed_units += export_tags_json(
            &collector,
            &illust_ids,
            &output_directory,
            &self.context.options,
        )
        .await;
        let mut result = download_urls(
            self.context.options.clone(),
            output_directory,
            self.base_url.clone(),
            &urls,
        )
        .await?;
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
