//! UserCrawler 骨架。

use std::time::Duration;

use futures::{StreamExt, stream};
use log::warn;
use url::Url;

use crate::{
    auth::Credential,
    collector::{
        PixivCollector, resolve_base_url,
        selector::{select_page_original_urls, select_user_illust_ids},
    },
    config::{ResolvedDownloadOptions, SortOrder},
    crawler::CrawlContext,
    downloader::{DownloadResult, Downloader},
    error::AppResult,
    utils::retry::retry_async,
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
        options: ResolvedDownloadOptions,
    ) -> AppResult<Self> {
        Ok(Self {
            artist_id,
            context: CrawlContext::new(credential, options),
            base_url: resolve_base_url(None)?,
        })
    }

    pub fn new_with_base_url(
        artist_id: String,
        credential: Credential,
        options: ResolvedDownloadOptions,
        base_url: Url,
    ) -> Self {
        Self {
            artist_id,
            context: CrawlContext::new(credential, options),
            base_url,
        }
    }

    pub async fn run(&self) -> AppResult<DownloadResult> {
        let collector = self.build_collector()?;
        let profile = retry_async(
            self.context.options.retry,
            Duration::from_millis(200),
            |_| {
                let collector = collector.clone();
                let artist_id = self.artist_id.clone();

                async move { collector.fetch_user_profile_all(&artist_id).await }
            },
        )
        .await?;

        let mut illust_ids = select_user_illust_ids(&profile)?;
        sort_illust_ids(&mut illust_ids, self.context.options.sort)?;
        if self.context.options.count > 0 && illust_ids.len() > self.context.options.count {
            illust_ids.truncate(self.context.options.count);
        }

        let page_results = stream::iter(illust_ids.into_iter().map(|illust_id| {
            let collector = collector.clone();
            let retry_count = self.context.options.retry;

            async move {
                let response = retry_async(retry_count, Duration::from_millis(200), |_| {
                    let collector = collector.clone();
                    let illust_id = illust_id.clone();

                    async move { collector.fetch_illust_pages(&illust_id).await }
                })
                .await;

                (illust_id, response)
            }
        }))
        .buffer_unordered(self.context.options.concurrent.max(1))
        .collect::<Vec<_>>()
        .await;

        let mut urls = Vec::new();
        let mut failed_units = 0usize;

        for (illust_id, response) in page_results {
            match response {
                Ok(value) => match select_page_original_urls(&value) {
                    Ok(mut image_urls) => urls.append(&mut image_urls),
                    Err(error) => {
                        failed_units += 1;
                        warn!("解析作品 {illust_id} 的图片 URL 失败: {error}");
                    }
                },
                Err(error) => {
                    failed_units += 1;
                    warn!("获取作品 {illust_id} 的图片 URL 失败: {error}");
                }
            }
        }

        let output_directory = self.context.options.directory.join(&self.artist_id);
        let downloader = Downloader::new(
            self.context.options.clone(),
            output_directory,
            self.base_url.clone(),
        )?;
        let mut result = downloader.download(&urls).await?;

        result.failed += failed_units;
        result.total += failed_units;

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

fn sort_illust_ids(illust_ids: &mut [String], sort: SortOrder) -> AppResult<()> {
    let mut keyed = illust_ids
        .iter()
        .map(|id| {
            id.parse::<u64>()
                .map(|value| (value, id.clone()))
                .map_err(|_| crate::error::CrawlerError::Parse(format!("作品 ID 不是数字: {id}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    keyed.sort_by_key(|(value, _)| *value);

    if sort == SortOrder::DateDesc {
        keyed.reverse();
    }

    for (slot, (_, id)) in illust_ids.iter_mut().zip(keyed) {
        *slot = id;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::SortOrder;

    use super::sort_illust_ids;

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
