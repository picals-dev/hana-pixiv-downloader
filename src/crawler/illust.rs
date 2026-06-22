//! IllustCrawler。

use std::sync::Arc;

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{download_items, export_tags_json},
    downloader::DownloadResult,
    downloader::image::DownloadItem,
    error::AppResult,
    net::PixivNetSession,
    output::resolve_output_layout,
    pixiv::selector::select_page_original_urls,
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
        let pages = self
            .context
            .session
            .fetch_illust_pages(&self.illust_id)
            .await?;

        let urls = select_page_original_urls(&pages)?;
        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.illust_id,
        )?;
        let target_dir = layout.context_dir().to_path_buf();
        let items = urls
            .into_iter()
            .map(|image_url| DownloadItem {
                illust_id: self.illust_id.clone(),
                image_url,
                target_dir: target_dir.clone(),
            })
            .collect::<Vec<_>>();
        let mut result = download_items(
            self.context.options.clone(),
            target_dir.clone(),
            Arc::clone(&self.context.session),
            &items,
        )
        .await?;

        let failure_records = export_tags_json(
            &self.context.session,
            std::slice::from_ref(&self.illust_id),
            &target_dir,
            &self.context.options,
        )
        .await;
        result.failed += failure_records.len();
        result.total += failure_records.len();
        result.failure_records.extend(failure_records);

        Ok(result)
    }
}
