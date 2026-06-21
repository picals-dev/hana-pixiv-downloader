//! BookmarkCrawler。

use crate::{
    auth::Credential,
    collector::{PixivCollector, resolve_base_url, selector::select_bookmark_illust_ids},
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{
        collect_download_items_for_illust_ids, download_items, export_tags_json, sort_illust_ids,
    },
    downloader::DownloadResult,
    error::AppResult,
    output::resolve_output_layout,
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
        mut options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        options.mode = DownloadMode::Bookmark;
        Ok(Self {
            user_id,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        user_id: String,
        credential: Credential,
        mut options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        options.mode = DownloadMode::Bookmark;
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
            let value = collector
                .fetch_bookmark_page(&self.user_id, offset, BOOKMARK_PAGE_SIZE)
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

        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.user_id,
        )?;
        let (items, mut failure_records) = collect_download_items_for_illust_ids(
            &collector,
            illust_ids.clone(),
            &layout,
            &self.context.options,
        )
        .await;
        failure_records.extend(
            export_tags_json(
                &collector,
                &illust_ids,
                layout.context_dir(),
                &self.context.options,
            )
            .await,
        );

        let mut result = download_items(
            &self.context.credential,
            self.context.options.clone(),
            layout.context_dir().to_path_buf(),
            self.base_url.clone(),
            &items,
        )
        .await?;
        result.failed += failure_records.len();
        result.total += failure_records.len();
        result.failure_records.extend(failure_records);

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
