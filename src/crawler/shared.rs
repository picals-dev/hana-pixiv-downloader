//! 多个下载入口共享的抓取辅助。

use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

use futures::{StreamExt, stream};
use log::warn;
use serde_json::Value;

use crate::{
    config::{ResolvedDownloadOptions, SortOrder},
    downloader::{
        ArtworkDownloadPlan, DownloadResult, Downloader, ImageArtworkPlan, UgoiraDownloadPlan,
        ugoira::target_path_for_ugoira,
    },
    error::{AppResult, CrawlerError},
    failure::{FailureRecord, FailureStage},
    net::PixivNetSession,
    output::OutputLayout,
    pixiv::selector::{
        IllustType, select_illust_tags, select_illust_type, select_page_original_urls,
        select_ugoira_metadata,
    },
};

#[derive(Debug, Clone, Default)]
pub(crate) struct PlannedArtworkBatch {
    pub plans: Vec<ArtworkDownloadPlan>,
    pub detail_cache: BTreeMap<String, Value>,
    pub failures: Vec<FailureRecord>,
}

pub(crate) async fn plan_artworks_for_illust_ids(
    session: &Arc<PixivNetSession>,
    illust_ids: Vec<String>,
    layout: &OutputLayout,
    options: &ResolvedDownloadOptions,
) -> PlannedArtworkBatch {
    let (detail_cache, mut failures) =
        fetch_illust_details(session, &illust_ids, options, FailureStage::Collect).await;

    let planned = stream::iter(illust_ids.into_iter().filter_map(|illust_id| {
        detail_cache.get(&illust_id).cloned().map(|detail| {
            let session = Arc::clone(session);
            let layout = layout.clone();
            let options = options.clone();

            async move {
                let outcome = plan_single_artwork(&session, &illust_id, &detail, &layout).await;
                (illust_id, outcome, options.mode)
            }
        })
    }))
    .buffer_unordered(options.concurrent.max(1))
    .collect::<Vec<_>>()
    .await;

    let mut plans = Vec::new();
    for (illust_id, outcome, mode) in planned {
        match outcome {
            Ok(plan) => plans.push(plan),
            Err(error) => {
                warn!("规划作品 {illust_id} 失败: {error}");
                failures.push(FailureRecord::from_report(
                    mode,
                    FailureStage::Collect,
                    Some(illust_id),
                    None,
                    None,
                    &error,
                ));
            }
        }
    }

    PlannedArtworkBatch {
        plans,
        detail_cache,
        failures,
    }
}

pub(crate) fn export_tags_json(
    detail_cache: &BTreeMap<String, Value>,
    output_directory: &Path,
    options: &ResolvedDownloadOptions,
) -> Vec<FailureRecord> {
    if !options.with_tags || detail_cache.is_empty() {
        return Vec::new();
    }

    let mut tag_map = BTreeMap::<String, Vec<String>>::new();
    let mut failures = Vec::new();

    for (illust_id, detail) in detail_cache {
        match select_illust_tags(detail) {
            Ok(tags) => {
                tag_map.insert(illust_id.clone(), tags);
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

pub(crate) async fn export_tags_json_for_illust_ids(
    session: &Arc<PixivNetSession>,
    illust_ids: &[String],
    output_directory: &Path,
    options: &ResolvedDownloadOptions,
) -> Vec<FailureRecord> {
    if !options.with_tags || illust_ids.is_empty() {
        return Vec::new();
    }

    let (detail_cache, mut failures) =
        fetch_illust_details(session, illust_ids, options, FailureStage::Tags).await;
    failures.extend(export_tags_json(&detail_cache, output_directory, options));
    failures
}

pub(crate) fn write_tags_json(
    output_directory: &Path,
    tag_map: &BTreeMap<String, Vec<String>>,
) -> AppResult<()> {
    fs::create_dir_all(output_directory)?;
    let path = output_directory.join("tags.json");
    fs::write(path, serde_json::to_vec_pretty(tag_map)?)?;
    Ok(())
}

pub(crate) fn sort_illust_ids(illust_ids: &mut [String], sort: SortOrder) -> AppResult<()> {
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

pub(crate) async fn download_artworks(
    options: ResolvedDownloadOptions,
    output_directory: std::path::PathBuf,
    session: Arc<PixivNetSession>,
    plans: &[ArtworkDownloadPlan],
) -> AppResult<DownloadResult> {
    let downloader = Downloader::new(options, output_directory, session);
    downloader.download(plans).await
}

/// 作品 ID 就绪后的统一收尾：规划作品计划、导出 tags、下载，并合并采集/标签阶段的失败计数。
///
/// 五类下载模式只在“如何收集 illust_ids”上有差异，收尾逻辑完全一致，统一收口到此处，
/// 避免各 crawler 手抄失败计数导致的静默偏差。
pub(crate) async fn plan_tags_and_download(
    session: &Arc<PixivNetSession>,
    illust_ids: Vec<String>,
    layout: &OutputLayout,
    options: &ResolvedDownloadOptions,
) -> AppResult<DownloadResult> {
    let planned = plan_artworks_for_illust_ids(session, illust_ids, layout, options).await;
    let mut failure_records = planned.failures;
    failure_records.extend(export_tags_json(
        &planned.detail_cache,
        layout.context_dir(),
        options,
    ));

    let mut result = download_artworks(
        options.clone(),
        layout.context_dir().to_path_buf(),
        Arc::clone(session),
        &planned.plans,
    )
    .await?;
    result.failed += failure_records.len();
    result.total += failure_records.len();
    result.failure_records.extend(failure_records);
    Ok(result)
}

async fn fetch_illust_details(
    session: &Arc<PixivNetSession>,
    illust_ids: &[String],
    options: &ResolvedDownloadOptions,
    stage: FailureStage,
) -> (BTreeMap<String, Value>, Vec<FailureRecord>) {
    let detail_results = stream::iter(illust_ids.iter().cloned().map(|illust_id| {
        let session = Arc::clone(session);

        async move {
            let response = session.fetch_illust_detail(&illust_id).await;
            (illust_id, response)
        }
    }))
    .buffer_unordered(options.concurrent.max(1))
    .collect::<Vec<_>>()
    .await;

    let mut detail_cache = BTreeMap::new();
    let mut failures = Vec::new();

    for (illust_id, response) in detail_results {
        match response {
            Ok(detail) => {
                detail_cache.insert(illust_id, detail);
            }
            Err(error) => {
                warn!("获取作品详情 {illust_id} 失败: {error}");
                failures.push(FailureRecord::from_report(
                    options.mode,
                    stage,
                    Some(illust_id),
                    None,
                    None,
                    &error,
                ));
            }
        }
    }

    (detail_cache, failures)
}

async fn plan_single_artwork(
    session: &Arc<PixivNetSession>,
    illust_id: &str,
    detail: &Value,
    layout: &OutputLayout,
) -> AppResult<ArtworkDownloadPlan> {
    match select_illust_type(detail)? {
        IllustType::Image => {
            let pages = session.fetch_illust_pages(illust_id).await?;
            let image_urls = select_page_original_urls(&pages)?;
            let target_dir = layout.illust_dir(illust_id)?;
            Ok(ArtworkDownloadPlan::Images(ImageArtworkPlan {
                illust_id: illust_id.to_string(),
                image_urls,
                target_dir,
            }))
        }
        IllustType::Ugoira => {
            let meta = session.fetch_ugoira_meta(illust_id).await?;
            let metadata = select_ugoira_metadata(&meta)?;
            let target_dir = layout.illust_dir(illust_id)?;
            Ok(ArtworkDownloadPlan::Ugoira(UgoiraDownloadPlan {
                illust_id: illust_id.to_string(),
                source_url: metadata.original_src.clone(),
                target_path: target_path_for_ugoira(&target_dir, illust_id),
                metadata,
            }))
        }
    }
}
