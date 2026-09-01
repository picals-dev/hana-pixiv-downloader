//! SeriesCrawler。

use std::sync::Arc;

use eyre::WrapErr;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions, SortOrder},
    crawler::CrawlContext,
    crawler::shared::plan_tags_and_download,
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
    pixiv::selector::{SeriesMeta, select_series_illust_orders, select_series_meta},
};

/// Pixiv 系列分页大小：`/ajax/series/{id}?p={page}` 每页返回 12 话。
const SERIES_PAGE_SIZE: usize = 12;

#[derive(Debug, Clone)]
pub struct SeriesCrawler {
    pub series_id: String,
    pub user_id: Option<String>,
    pub(crate) context: CrawlContext,
}

impl SeriesCrawler {
    pub fn new(
        series_id: String,
        user_id: Option<String>,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> Self {
        options.mode = DownloadMode::Series;
        Self {
            series_id,
            user_id,
            context: CrawlContext::new(options, session),
        }
    }

    /// 拉取首页并返回系列元信息，用于下载前的确认对话。
    pub async fn probe(&self) -> AppResult<SeriesMeta> {
        let value = self.fetch_page(1).await?;
        Ok(select_series_meta(&value)?)
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let meta = self.probe().await?;
        let target_count = self.context.options.count;
        let max_page = meta.total.div_ceil(SERIES_PAGE_SIZE).max(1);

        let mut entries = Vec::new();
        for page in 1..=max_page {
            let page_entries = select_series_illust_orders(&self.fetch_page(page).await?)?;
            if page_entries.is_empty() {
                break;
            }
            entries.extend(page_entries);
        }

        // 同一话不会跨页重复，但按 (order, workId) 排序去重以彻底稳住顺序。
        entries.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        entries.dedup();

        // 系列以 order 为准：默认从新到旧（order 降序），date_asc 则从第 1 话开始。
        let mut illust_ids: Vec<String> = entries.into_iter().map(|(_, work_id)| work_id).collect();
        if self.context.options.sort == SortOrder::DateDesc {
            illust_ids.reverse();
        }
        if target_count > 0 && illust_ids.len() > target_count {
            illust_ids.truncate(target_count);
        }

        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.series_id,
        )?;
        plan_tags_and_download(
            &self.context.session,
            illust_ids,
            &layout,
            &self.context.options,
        )
        .await
    }

    async fn fetch_page(&self, page: usize) -> AppResult<serde_json::Value> {
        self.context
            .session
            .fetch_series_page(&self.series_id, self.user_id.as_deref(), page)
            .await
            .wrap_err(format!(
                "获取系列 {} 第 {page} 页失败（系列不存在、已删除，或为不支持的小说系列）",
                self.series_id
            ))
    }
}
