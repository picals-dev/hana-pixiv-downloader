//! RankingCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{plan_tags_and_download, sort_illust_ids},
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
    pixiv::selector::select_ranking_illust_ids,
};

#[derive(Debug, Clone)]
pub struct RankingCrawler {
    pub mode: String,
    pub(crate) context: CrawlContext,
}

impl RankingCrawler {
    pub fn new(
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
        plan_tags_and_download(
            &self.context.session,
            illust_ids,
            &layout,
            &self.context.options,
        )
        .await
    }
}
