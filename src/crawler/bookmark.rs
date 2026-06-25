//! BookmarkCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{plan_tags_and_download, sort_illust_ids},
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
    pub(crate) context: CrawlContext,
}

impl BookmarkCrawler {
    pub fn new(
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
        plan_tags_and_download(
            &self.context.session,
            illust_ids,
            &layout,
            &self.context.options,
        )
        .await
    }
}
