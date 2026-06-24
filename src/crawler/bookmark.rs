//! BookmarkCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{
        download_artworks, export_tags_json, plan_artworks_for_illust_ids, sort_illust_ids,
    },
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
    pixiv::selector::select_bookmark_illust_ids,
};

const BOOKMARK_PAGE_SIZE: usize = 48;

#[derive(Debug, Clone)]
pub struct BookmarkCrawler {
    pub user_id: String,
    pub context: CrawlContext,
}

impl BookmarkCrawler {
    pub fn new(
        user_id: String,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> AppResult<Self> {
        options.mode = DownloadMode::Bookmark;
        Ok(Self {
            user_id,
            context: CrawlContext::new(options, session),
        })
    }

    pub fn new_with_session(
        user_id: String,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> Self {
        options.mode = DownloadMode::Bookmark;
        Self {
            user_id,
            context: CrawlContext::new(options, session),
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let target_count = self.context.options.count;
        let mut offset = 0usize;
        let mut illust_ids = Vec::new();

        loop {
            let value = self
                .context
                .session
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
        let planned = plan_artworks_for_illust_ids(
            &self.context.session,
            illust_ids.clone(),
            &layout,
            &self.context.options,
        )
        .await;
        let mut failure_records = planned.failures;
        failure_records.extend(export_tags_json(
            &planned.detail_cache,
            layout.context_dir(),
            &self.context.options,
        ));

        let mut result = download_artworks(
            self.context.options.clone(),
            layout.context_dir().to_path_buf(),
            Arc::clone(&self.context.session),
            &planned.plans,
        )
        .await?;
        result.failed += failure_records.len();
        result.total += failure_records.len();
        result.failure_records.extend(failure_records);

        Ok(result)
    }
}
