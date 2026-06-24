//! IllustCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{download_artworks, export_tags_json, plan_artworks_for_illust_ids},
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
};

#[derive(Debug, Clone)]
pub struct IllustCrawler {
    pub illust_id: String,
    pub context: CrawlContext,
}

impl IllustCrawler {
    pub fn new(
        illust_id: String,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> AppResult<Self> {
        options.mode = DownloadMode::Illust;
        Ok(Self {
            illust_id,
            context: CrawlContext::new(options, session),
        })
    }

    pub fn new_with_session(
        illust_id: String,
        mut options: ResolvedDownloadOptions,
        session: Arc<PixivNetSession>,
    ) -> Self {
        options.mode = DownloadMode::Illust;
        Self {
            illust_id,
            context: CrawlContext::new(options, session),
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.illust_id,
        )?;
        let target_dir = layout.context_dir().to_path_buf();
        let planned = plan_artworks_for_illust_ids(
            &self.context.session,
            vec![self.illust_id.clone()],
            &layout,
            &self.context.options,
        )
        .await;
        let mut failure_records = planned.failures;
        failure_records.extend(export_tags_json(
            &planned.detail_cache,
            &target_dir,
            &self.context.options,
        ));

        let mut result = download_artworks(
            self.context.options.clone(),
            target_dir.clone(),
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
