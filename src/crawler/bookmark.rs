//! BookmarkCrawler。

use std::time::Duration;

use crate::{
    auth::Credential,
    collector::{PixivCollector, resolve_base_url, selector::select_bookmark_illust_ids},
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

const BOOKMARK_PAGE_SIZE: usize = 48;

#[derive(Debug, Clone)]
pub struct BookmarkCrawler {
    pub user_id: String,
    pub context: CrawlContext,
    base_url: Url,
}

impl BookmarkCrawler {
    pub fn new(
        user_id: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        Ok(Self {
            user_id,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        user_id: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        Self {
            user_id,
            context: CrawlContext::new(credential, options),
            base_url,
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let collector = self.build_collector()?;
        let target_count = self.context.options.count;
        let mut offset = 0usize;
        let mut illust_ids = Vec::new();

        loop {
            let value = retry_async(
                self.context.options.retry,
                Duration::from_millis(200),
                |_| {
                    let collector = collector.clone();
                    let user_id = self.user_id.clone();

                    async move {
                        collector
                            .fetch_bookmark_page(&user_id, offset, BOOKMARK_PAGE_SIZE)
                            .await
                    }
                },
            )
            .await?;

            let page_ids = select_bookmark_illust_ids(&value)?;
            let page_len = value
                .pointer("/body/works")
                .and_then(serde_json::Value::as_array)
                .map_or(0, |items| items.len());

            if page_ids.is_empty() && page_len == 0 {
                break;
            }

            illust_ids.extend(page_ids);
            illust_ids.sort();
            illust_ids.dedup();

            if target_count > 0 && illust_ids.len() >= target_count {
                break;
            }

            if page_len < BOOKMARK_PAGE_SIZE {
                break;
            }

            offset += BOOKMARK_PAGE_SIZE;
        }

        sort_illust_ids(&mut illust_ids, self.context.options.sort)?;
        if target_count > 0 && illust_ids.len() > target_count {
            illust_ids.truncate(target_count);
        }

        let (urls, mut failed_units) = collect_image_urls_for_illust_ids(
            &collector,
            illust_ids.clone(),
            &self.context.options,
        )
        .await;
        let output_directory = self.context.options.directory.join("bookmark");
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
