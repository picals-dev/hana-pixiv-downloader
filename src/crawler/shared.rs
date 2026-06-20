//! 多个下载入口共享的抓取辅助。

use std::{collections::BTreeMap, fs, path::Path, time::Duration};

use futures::{StreamExt, stream};
use log::warn;
use url::Url;

use crate::{
    collector::{
        PixivCollector,
        selector::{select_illust_tags, select_page_original_urls},
    },
    config::{ResolvedDownloadOptions, SortOrder},
    downloader::{DownloadResult, Downloader},
    error::{AppResult, CrawlerError},
    utils::retry::retry_async,
};

pub async fn collect_image_urls_for_illust_ids(
    collector: &PixivCollector,
    illust_ids: Vec<String>,
    options: &ResolvedDownloadOptions,
) -> (Vec<String>, usize) {
    let page_results = stream::iter(illust_ids.into_iter().map(|illust_id| {
        let collector = collector.clone();
        let retry_count = options.retry;

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
    .buffer_unordered(options.concurrent.max(1))
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

    (urls, failed_units)
}

pub async fn export_tags_json(
    collector: &PixivCollector,
    illust_ids: &[String],
    output_directory: &Path,
    options: &ResolvedDownloadOptions,
) -> usize {
    if !options.with_tags || illust_ids.is_empty() {
        return 0;
    }

    let tag_results = stream::iter(illust_ids.iter().cloned().map(|illust_id| {
        let collector = collector.clone();
        let retry_count = options.retry;

        async move {
            let response = retry_async(retry_count, Duration::from_millis(200), |_| {
                let collector = collector.clone();
                let illust_id = illust_id.clone();

                async move { collector.fetch_illust_detail(&illust_id).await }
            })
            .await;

            (illust_id, response)
        }
    }))
    .buffer_unordered(options.concurrent.max(1))
    .collect::<Vec<_>>()
    .await;

    let mut tag_map = BTreeMap::<String, Vec<String>>::new();
    let mut failed = 0usize;

    for (illust_id, response) in tag_results {
        match response {
            Ok(value) => match select_illust_tags(&value) {
                Ok(tags) => {
                    tag_map.insert(illust_id, tags);
                }
                Err(error) => {
                    failed += 1;
                    warn!("解析作品 {illust_id} 的标签失败: {error}");
                }
            },
            Err(error) => {
                failed += 1;
                warn!("获取作品 {illust_id} 的标签失败: {error}");
            }
        }
    }

    if let Err(error) = write_tags_json(output_directory, &tag_map) {
        failed += 1;
        warn!("写入 tags.json 失败: {error}");
    }

    failed
}

pub fn write_tags_json(
    output_directory: &Path,
    tag_map: &BTreeMap<String, Vec<String>>,
) -> AppResult<()> {
    fs::create_dir_all(output_directory)?;
    let path = output_directory.join("tags.json");
    fs::write(path, serde_json::to_vec_pretty(tag_map)?)?;
    Ok(())
}

pub fn sort_illust_ids(illust_ids: &mut [String], sort: SortOrder) -> AppResult<()> {
    let mut keyed = illust_ids
        .iter()
        .map(|id| {
            id.parse::<u64>()
                .map(|value| (value, id.clone()))
                .map_err(|_| CrawlerError::Parse(format!("作品 ID 不是数字: {id}")))
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

pub async fn download_urls(
    options: ResolvedDownloadOptions,
    output_directory: std::path::PathBuf,
    referer_base_url: Url,
    urls: &[String],
) -> AppResult<DownloadResult> {
    let downloader = Downloader::new(options, output_directory, referer_base_url)?;
    downloader.download(urls).await
}
