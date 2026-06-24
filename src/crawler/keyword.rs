//! KeywordCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions, SortOrder},
    crawler::CrawlContext,
    crawler::shared::{
        download_artworks, export_tags_json, plan_artworks_for_illust_ids, sort_illust_ids,
    },
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
    pixiv::selector::select_keyword_illust_ids,
};

#[derive(Debug, Clone)]
pub struct KeywordCrawler {
    pub query: String,
    pub mode: KeywordMode,
    pub context: CrawlContext,
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
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> AppResult<Self> {
        options.mode = DownloadMode::Keyword;
        Ok(Self {
            query,
            mode,
            context: CrawlContext::new(options, session),
        })
    }

    pub fn new_with_session(
        query: String,
        mode: KeywordMode,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> Self {
        options.mode = DownloadMode::Keyword;
        Self {
            query,
            mode,
            context: CrawlContext::new(options, session),
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
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
            let value = self
                .context
                .session
                .fetch_keyword_page(&self.query, order, mode, page, self.context.options.ai)
                .await?;
            illust_ids.extend(select_keyword_illust_ids(&value)?);
        }

        sort_illust_ids(&mut illust_ids, self.context.options.sort)?;
        illust_ids.dedup();
        if target_count > 0 && illust_ids.len() > target_count {
            illust_ids.truncate(target_count);
        }

        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.query,
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
