//! RankingCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{
        collect_download_items_for_illust_ids, download_items, export_tags_json, sort_illust_ids,
    },
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
    pixiv::selector::select_ranking_illust_ids,
};

#[derive(Debug, Clone)]
pub struct RankingCrawler {
    pub mode: String,
    pub context: CrawlContext,
}

impl RankingCrawler {
    pub fn new(
        mode: String,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> AppResult<Self> {
        options.mode = DownloadMode::Ranking;
        Ok(Self {
            mode,
            context: CrawlContext::new(options, session),
        })
    }

    pub fn new_with_session(
        mode: String,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> Self {
        options.mode = DownloadMode::Ranking;
        Self {
            mode,
            context: CrawlContext::new(options, session),
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let target_count = self.context.options.count;
        let page_count = if target_count == 0 {
            1
        } else {
            target_count.div_ceil(50)
        };
        let mut illust_ids = Vec::new();

        for page in 1..=page_count {
            let value = self
                .context
                .session
                .fetch_ranking_page(&self.mode, page)
                .await?;
            illust_ids.extend(select_ranking_illust_ids(&value)?);
        }

        sort_illust_ids(&mut illust_ids, self.context.options.sort)?;
        illust_ids.dedup();
        if target_count > 0 && illust_ids.len() > target_count {
            illust_ids.truncate(target_count);
        }

        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.mode,
        )?;
        let (items, mut failure_records) = collect_download_items_for_illust_ids(
            &self.context.session,
            illust_ids.clone(),
            &layout,
            &self.context.options,
        )
        .await;
        failure_records.extend(
            export_tags_json(
                &self.context.session,
                &illust_ids,
                layout.context_dir(),
                &self.context.options,
            )
            .await,
        );
        let mut result = download_items(
            self.context.options.clone(),
            layout.context_dir().to_path_buf(),
            Arc::clone(&self.context.session),
            &items,
        )
        .await?;
        result.failed += failure_records.len();
        result.total += failure_records.len();
        result.failure_records.extend(failure_records);

        Ok(result)
    }
}
