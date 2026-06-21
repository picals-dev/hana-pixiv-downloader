//! UserCrawler。

use url::Url;

use crate::{
    auth::Credential,
    collector::{PixivCollector, resolve_base_url, selector::select_user_illust_ids},
    config::{DownloadMode, ResolvedDownloadOptions},
    crawler::CrawlContext,
    crawler::shared::{
        collect_download_items_for_illust_ids, download_items, export_tags_json, sort_illust_ids,
    },
    downloader::DownloadResult,
    error::AppResult,
    output::resolve_output_layout,
};

#[derive(Debug, Clone)]
pub struct UserCrawler {
    pub artist_id: String,
    pub context: CrawlContext,
    base_url: Url,
}

impl UserCrawler {
    pub fn new(
        artist_id: String,
        credential: Credential,
        mut options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        options.mode = DownloadMode::User;
        Ok(Self {
            artist_id,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        artist_id: String,
        credential: Credential,
        mut options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        options.mode = DownloadMode::User;
        Self {
            artist_id,
            context: CrawlContext::new(credential, options),
            base_url,
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let collector = self.build_collector()?;
        let profile = collector.fetch_user_profile_all(&self.artist_id).await?;

        let mut illust_ids = select_user_illust_ids(&profile)?;
        sort_illust_ids(&mut illust_ids, self.context.options.sort)?;
        if self.context.options.count > 0 && illust_ids.len() > self.context.options.count {
            illust_ids.truncate(self.context.options.count);
        }
        let layout = resolve_output_layout(
            self.context.options.mode,
            &self.context.options.directory,
            &self.artist_id,
        )?;
        let (items, mut failure_records) = collect_download_items_for_illust_ids(
            &collector,
            illust_ids.clone(),
            &layout,
            &self.context.options,
        )
        .await;
        failure_records.extend(
            export_tags_json(
                &collector,
                &illust_ids,
                layout.context_dir(),
                &self.context.options,
            )
            .await,
        );
        let mut result = download_items(
            &self.context.credential,
            self.context.options.clone(),
            layout.context_dir().to_path_buf(),
            self.base_url.clone(),
            &items,
        )
        .await?;

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
#[cfg(test)]
mod tests {
    use crate::config::SortOrder;

    use crate::crawler::shared::sort_illust_ids;

    #[test]
    fn illust_ids_can_be_sorted_descending_by_default() {
        let mut ids = vec!["123".to_string(), "456".to_string(), "234".to_string()];
        sort_illust_ids(&mut ids, SortOrder::DateDesc).unwrap();
        assert_eq!(ids, vec!["456", "234", "123"]);
    }

    #[test]
    fn illust_ids_can_be_sorted_ascending() {
        let mut ids = vec!["123".to_string(), "456".to_string(), "234".to_string()];
        sort_illust_ids(&mut ids, SortOrder::DateAsc).unwrap();
        assert_eq!(ids, vec!["123", "234", "456"]);
    }

    #[test]
    fn illust_ids_reject_non_numeric_values() {
        let mut ids = vec!["123".to_string(), "oops".to_string()];
        let error = sort_illust_ids(&mut ids, SortOrder::DateDesc).unwrap_err();
        assert!(format!("{error:#}").contains("作品 ID 不是数字"));
    }
}
