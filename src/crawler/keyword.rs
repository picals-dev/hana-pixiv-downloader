//! KeywordCrawler。

use std::time::Duration;

use crate::{
    auth::Credential,
    collector::{PixivCollector, resolve_base_url, selector::select_keyword_illust_ids},
    config::{ResolvedDownloadOptions, SortOrder},
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
pub struct KeywordCrawler {
    pub query: String,
    pub mode: KeywordMode,
    pub context: CrawlContext,
    base_url: Url,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordMode {
    Safe,
    R18,
}

impl KeywordCrawler {
    pub fn new(
        query: String,
        mode: KeywordMode,
        credential: Credential,
        options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        Ok(Self {
            query,
            mode,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        query: String,
        mode: KeywordMode,
        credential: Credential,
        options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        Self {
            query,
            mode,
            context: CrawlContext::new(credential, options),
            base_url,
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let collector = self.build_collector()?;
        let order = match self.context.options.sort {
            SortOrder::DateDesc => "date_d",
            SortOrder::DateAsc => "date",
            SortOrder::PopularDesc => "popular_d",
        };
        let mode = match self.mode {
            KeywordMode::Safe => "safe",
            KeywordMode::R18 => "r18",
        };
        let target_count = self.context.options.count;
        let page_count = if target_count == 0 {
            1
        } else {
            target_count.div_ceil(60)
        };

        let mut illust_ids = Vec::new();
        for page in 1..=page_count {
            let value = retry_async(
                self.context.options.retry,
                Duration::from_millis(200),
                |_| {
                    let collector = collector.clone();
                    let query = self.query.clone();

                    async move {
                        collector
                            .fetch_keyword_page(&query, order, mode, page, self.context.options.ai)
                            .await
                    }
                },
            )
            .await?;
            illust_ids.extend(select_keyword_illust_ids(&value)?);
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
        let output_directory = self.context.options.directory.join(&self.query);
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
