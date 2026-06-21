//! 多个下载入口共享的抓取辅助。

use std::{collections::BTreeMap, fs, path::Path};

use futures::{StreamExt, stream};
use log::warn;
use url::Url;

use crate::{
    auth::Credential,
    collector::{
        PixivCollector,
        selector::{select_illust_tags, select_page_original_urls},
    },
    config::{ResolvedDownloadOptions, SortOrder},
    downloader::{DownloadResult, Downloader, image::DownloadItem},
    error::{AppResult, CrawlerError},
    failure::{FailureRecord, FailureStage},
    output::OutputLayout,
};

pub async fn collect_download_items_for_illust_ids(
    collector: &PixivCollector,
    illust_ids: Vec<String>,
    layout: &OutputLayout,
    options: &ResolvedDownloadOptions,
) -> (Vec<DownloadItem>, Vec<FailureRecord>) {
    let page_results = stream::iter(illust_ids.into_iter().map(|illust_id| {
        let collector = collector.clone();
        let layout = layout.clone();

        async move {
            let response = collector.fetch_illust_pages(&illust_id).await;

            (illust_id, layout, response)
        }
    }))
    .buffer_unordered(options.concurrent.max(1))
    .collect::<Vec<_>>()
    .await;

    let mut items = Vec::new();
    let mut failures = Vec::new();

    for (illust_id, layout, response) in page_results {
        match response {
            Ok(value) => match select_page_original_urls(&value) {
                Ok(image_urls) => match layout.illust_dir(&illust_id) {
                    Ok(target_dir) => {
                        items.extend(image_urls.into_iter().map(|image_url| DownloadItem {
                            illust_id: illust_id.clone(),
                            image_url,
                            target_dir: target_dir.clone(),
                        }));
                    }
                    Err(error) => {
                        warn!("解析作品 {illust_id} 的下载目录失败: {error}");
                        failures.push(FailureRecord::from_report(
                            options.mode,
                            FailureStage::Collect,
                            Some(illust_id.clone()),
                            None,
                            None,
                            &error,
                        ));
                    }
                },
                Err(error) => {
                    warn!("解析作品 {illust_id} 的图片 URL 失败: {error}");
                    failures.push(FailureRecord::from_crawler_error(
                        options.mode,
                        FailureStage::Collect,
                        Some(illust_id.clone()),
                        None,
                        None,
                        &error,
                    ));
                }
            },
            Err(error) => {
                warn!("获取作品 {illust_id} 的图片 URL 失败: {error}");
                failures.push(FailureRecord::from_report(
                    options.mode,
                    FailureStage::Collect,
                    Some(illust_id.clone()),
                    None,
                    None,
                    &error,
                ));
            }
        }
    }

    (items, failures)
}

pub async fn export_tags_json(
    collector: &PixivCollector,
    illust_ids: &[String],
    output_directory: &Path,
    options: &ResolvedDownloadOptions,
) -> Vec<FailureRecord> {
    if !options.with_tags || illust_ids.is_empty() {
        return Vec::new();
    }

    let tag_results = stream::iter(illust_ids.iter().cloned().map(|illust_id| {
        let collector = collector.clone();

        async move {
            let response = collector.fetch_illust_detail(&illust_id).await;

            (illust_id, response)
        }
    }))
    .buffer_unordered(options.concurrent.max(1))
    .collect::<Vec<_>>()
    .await;

    let mut tag_map = BTreeMap::<String, Vec<String>>::new();
    let mut failures = Vec::new();

    for (illust_id, response) in tag_results {
        match response {
            Ok(value) => match select_illust_tags(&value) {
                Ok(tags) => {
                    tag_map.insert(illust_id, tags);
                }
                Err(error) => {
                    warn!("解析作品 {illust_id} 的标签失败: {error}");
                    failures.push(FailureRecord::from_crawler_error(
                        options.mode,
                        FailureStage::Tags,
                        Some(illust_id.clone()),
                        None,
                        None,
                        &error,
                    ));
                }
            },
            Err(error) => {
                warn!("获取作品 {illust_id} 的标签失败: {error}");
                failures.push(FailureRecord::from_report(
                    options.mode,
                    FailureStage::Tags,
                    Some(illust_id.clone()),
                    None,
                    None,
                    &error,
                ));
            }
        }
    }

    if let Err(error) = write_tags_json(output_directory, &tag_map) {
        warn!("写入 tags.json 失败: {error}");
        failures.push(FailureRecord::from_report(
            options.mode,
            FailureStage::Tags,
            None,
            None,
            Some(
                output_directory
                    .join("tags.json")
                    .to_string_lossy()
                    .into_owned(),
            ),
            &error,
        ));
    }

    failures
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

pub async fn download_items(
    credential: &Credential,
    options: ResolvedDownloadOptions,
    output_directory: std::path::PathBuf,
    referer_base_url: Url,
    items: &[DownloadItem],
) -> AppResult<DownloadResult> {
    let downloader = Downloader::new(options, output_directory, referer_base_url, credential)?;
    downloader.download(items).await
}
