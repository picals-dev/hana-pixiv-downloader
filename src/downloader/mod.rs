//! 下载模块骨架。

pub(crate) mod image;
pub mod ugoira;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::{StreamExt, stream};
use log::warn;

use crate::{
    config::ResolvedDownloadOptions,
    error::AppResult,
    failure::{FailureRecord, FailureStage},
    net::PixivNetSession,
    pixiv::selector::UgoiraMetadata,
    utils::progress::DownloadProgress,
};

use self::{
    image::DownloadItem,
    ugoira::{UgoiraDownloadError, download_ugoira_with_progress},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DownloadResult {
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub total_bytes: u64,
    pub failure_records: Vec<FailureRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImageArtworkPlan {
    pub illust_id: String,
    pub image_urls: Vec<String>,
    pub target_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UgoiraDownloadPlan {
    pub illust_id: String,
    pub source_url: String,
    pub target_path: PathBuf,
    pub metadata: UgoiraMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtworkDownloadPlan {
    Images(ImageArtworkPlan),
    Ugoira(UgoiraDownloadPlan),
}

impl ArtworkDownloadPlan {
    pub(crate) fn illust_id(&self) -> &str {
        match self {
            Self::Images(plan) => &plan.illust_id,
            Self::Ugoira(plan) => &plan.illust_id,
        }
    }

    pub(crate) fn output_count(&self) -> u64 {
        match self {
            Self::Images(plan) => plan.image_urls.len() as u64,
            Self::Ugoira(_) => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Downloader {
    pub options: ResolvedDownloadOptions,
    pub directory: PathBuf,
    session: Arc<PixivNetSession>,
}

impl Downloader {
    pub(crate) fn new(
        options: ResolvedDownloadOptions,
        directory: PathBuf,
        session: Arc<PixivNetSession>,
    ) -> Self {
        Self {
            directory,
            options,
            session,
        }
    }

    pub(crate) async fn download(
        &self,
        plans: &[ArtworkDownloadPlan],
    ) -> AppResult<DownloadResult> {
        if plans.is_empty() {
            return Ok(DownloadResult::default());
        }

        fs::create_dir_all(&self.directory)?;

        let progress =
            DownloadProgress::new(total_outputs(plans), build_illust_progress_totals(plans));
        let tasks = build_download_tasks(plans);

        let outcomes = stream::iter(tasks.into_iter().map(|task| {
            let downloader = self.clone();
            let progress = progress.clone();

            async move {
                let illust_id = task.illust_id().to_string();
                let outcome = downloader.download_one(task, progress.clone()).await;
                let failed = matches!(outcome, DownloadOutcome::Failed(_));
                progress.record_unit_completion(&illust_id, failed);
                outcome
            }
        }))
        .buffer_unordered(self.options.concurrent.max(1))
        .collect::<Vec<_>>()
        .await;

        progress.finish_with_message("下载阶段完成");

        let mut result = DownloadResult::default();
        for outcome in outcomes {
            result.total += 1;

            match outcome {
                DownloadOutcome::Downloaded(bytes) => {
                    result.downloaded += 1;
                    result.total_bytes += bytes;
                }
                DownloadOutcome::Skipped => result.skipped += 1,
                DownloadOutcome::Failed(record) => {
                    result.failed += 1;
                    result.failure_records.push(record);
                }
            }
        }

        Ok(result)
    }

    async fn download_one(
        &self,
        task: DownloadTask,
        progress: DownloadProgress,
    ) -> DownloadOutcome {
        match task {
            DownloadTask::Image(item) => self.download_image(item, progress).await,
            DownloadTask::Ugoira(plan) => self.download_ugoira(plan, progress).await,
        }
    }

    async fn download_image(
        &self,
        item: DownloadItem,
        progress: DownloadProgress,
    ) -> DownloadOutcome {
        let target_path = match item.target_path() {
            Ok(path) => path,
            Err(error) => {
                warn!("解析下载路径失败 {}: {error}", item.image_url);
                let report = eyre::Report::new(error);
                return DownloadOutcome::Failed(FailureRecord::from_report(
                    self.options.mode,
                    FailureStage::Download,
                    Some(item.illust_id.clone()),
                    Some(item.image_url.clone()),
                    None,
                    &report,
                ));
            }
        };

        if file_exists_and_nonempty(&target_path) {
            return DownloadOutcome::Skipped;
        }

        let url = item.image_url;
        let download_result = async {
            let progress_observer = Arc::new(move |bytes| {
                progress.record_downloaded_bytes(bytes);
            });
            let bytes = self
                .session
                .download_original_image_with_progress(
                    &url,
                    &item.illust_id,
                    &target_path,
                    Some(progress_observer),
                )
                .await?;
            Ok::<u64, eyre::Report>(bytes)
        }
        .await;

        match download_result {
            Ok(bytes) => DownloadOutcome::Downloaded(bytes),
            Err(error) => {
                warn!("下载失败 {url}: {error}");
                DownloadOutcome::Failed(FailureRecord::from_report(
                    self.options.mode,
                    FailureStage::Download,
                    Some(item.illust_id),
                    Some(url),
                    Some(target_path.to_string_lossy().into_owned()),
                    &error,
                ))
            }
        }
    }

    async fn download_ugoira(
        &self,
        plan: UgoiraDownloadPlan,
        progress: DownloadProgress,
    ) -> DownloadOutcome {
        if file_exists_and_nonempty(&plan.target_path) {
            return DownloadOutcome::Skipped;
        }

        let source_url = plan.source_url.clone();
        let target_path = plan.target_path.clone();
        let illust_id = plan.illust_id.clone();
        let download_result = async {
            let progress_observer = Arc::new(move |bytes| {
                progress.record_downloaded_bytes(bytes);
            });
            download_ugoira_with_progress(self.session.as_ref(), &plan, Some(progress_observer))
                .await
        }
        .await;

        match download_result {
            Ok(bytes) => DownloadOutcome::Downloaded(bytes),
            Err(UgoiraDownloadError { stage, error }) => {
                warn!("下载动图失败 {source_url}: {error}");
                DownloadOutcome::Failed(FailureRecord::from_report(
                    self.options.mode,
                    stage,
                    Some(illust_id),
                    Some(source_url),
                    Some(target_path.to_string_lossy().into_owned()),
                    &error,
                ))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DownloadTask {
    Image(DownloadItem),
    Ugoira(UgoiraDownloadPlan),
}

impl DownloadTask {
    fn illust_id(&self) -> &str {
        match self {
            Self::Image(item) => &item.illust_id,
            Self::Ugoira(plan) => &plan.illust_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DownloadOutcome {
    Downloaded(u64),
    Skipped,
    Failed(FailureRecord),
}

fn build_download_tasks(plans: &[ArtworkDownloadPlan]) -> Vec<DownloadTask> {
    let mut tasks = Vec::new();
    for plan in plans {
        match plan {
            ArtworkDownloadPlan::Images(plan) => {
                tasks.extend(plan.image_urls.iter().cloned().map(|image_url| {
                    DownloadTask::Image(DownloadItem {
                        illust_id: plan.illust_id.clone(),
                        image_url,
                        target_dir: plan.target_dir.clone(),
                    })
                }));
            }
            ArtworkDownloadPlan::Ugoira(plan) => {
                tasks.push(DownloadTask::Ugoira(plan.clone()));
            }
        }
    }
    tasks
}

fn build_illust_progress_totals(plans: &[ArtworkDownloadPlan]) -> Vec<(String, u64)> {
    let mut totals = BTreeMap::<String, u64>::new();
    for plan in plans {
        *totals.entry(plan.illust_id().to_string()).or_default() += plan.output_count();
    }
    totals.into_iter().collect()
}

fn total_outputs(plans: &[ArtworkDownloadPlan]) -> u64 {
    plans.iter().map(ArtworkDownloadPlan::output_count).sum()
}

fn file_exists_and_nonempty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        downloader::{
            ArtworkDownloadPlan, ImageArtworkPlan, UgoiraDownloadPlan, build_illust_progress_totals,
        },
        pixiv::selector::{UgoiraFrame, UgoiraMetadata},
    };

    #[test]
    fn illust_progress_totals_are_grouped_by_illust() {
        let plans = vec![
            ArtworkDownloadPlan::Images(ImageArtworkPlan {
                illust_id: "200".to_string(),
                image_urls: vec![
                    "https://i.pximg.net/200_p0.png".to_string(),
                    "https://i.pximg.net/200_p1.png".to_string(),
                ],
                target_dir: PathBuf::from("/tmp/200"),
            }),
            ArtworkDownloadPlan::Images(ImageArtworkPlan {
                illust_id: "100".to_string(),
                image_urls: vec!["https://i.pximg.net/100_p0.png".to_string()],
                target_dir: PathBuf::from("/tmp/100"),
            }),
            ArtworkDownloadPlan::Ugoira(UgoiraDownloadPlan {
                illust_id: "300".to_string(),
                source_url: "https://i.pximg.net/300.zip".to_string(),
                target_path: PathBuf::from("/tmp/300/300.gif"),
                metadata: UgoiraMetadata {
                    original_src: "https://i.pximg.net/300.zip".to_string(),
                    mime_type: Some("image/png".to_string()),
                    frames: vec![UgoiraFrame {
                        file: "000000.png".to_string(),
                        delay_ms: 60,
                    }],
                },
            }),
        ];

        assert_eq!(
            build_illust_progress_totals(&plans),
            vec![
                ("100".to_string(), 1),
                ("200".to_string(), 2),
                ("300".to_string(), 1)
            ]
        );
    }
}
