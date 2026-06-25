//! IllustCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::plan_tags_and_download,
    downloader::DownloadResult,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
};

#[derive(Debug, Clone)]
pub struct IllustCrawler {
    pub illust_id: String,
    pub(crate) context: CrawlContext,
}

impl IllustCrawler {
    pub fn new(
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
        plan_tags_and_download(
            &self.context.session,
            vec![self.illust_id.clone()],
            &layout,
            &self.context.options,
        )
        .await
    }
}
