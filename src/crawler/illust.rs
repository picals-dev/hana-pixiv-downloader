//! IllustCrawler。

use crate::{
    auth::Credential,
    collector::{PixivCollector, resolve_base_url, selector::select_page_original_urls},
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{download_items, export_tags_json},
    downloader::DownloadResult,
    downloader::image::DownloadItem,
    error::AppResult,
    output::resolve_output_layout,
};
use url::Url;

#[derive(Debug, Clone)]
pub struct IllustCrawler {
    pub illust_id: String,
    pub context: CrawlContext,
    base_url: Url,
}

impl IllustCrawler {
    pub fn new(
        illust_id: String,
        credential: Credential,
        mut options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        options.mode = DownloadMode::Illust;
        Ok(Self {
            illust_id,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        illust_id: String,
        credential: Credential,
        mut options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        options.mode = DownloadMode::Illust;
        Self {
            illust_id,
            context: CrawlContext::new(credential, options),
            base_url,
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let collector = self.build_collector()?;
        let pages = collector.fetch_illust_pages(&self.illust_id).await?;

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
            &self.context.credential,
            self.context.options.clone(),
            target_dir.clone(),
            self.base_url.clone(),
            &items,
        )
        .await?;

        let failure_records = export_tags_json(
            &collector,
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

    fn build_collector(&self) -> AppResult<PixivCollector> {
        PixivCollector::new_with_base_url(
            &self.context.options,
            &self.context.credential,
            self.base_url.clone(),
        )
    }
}
