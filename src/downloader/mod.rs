//! 下载模块骨架。

pub mod image;

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
    utils::progress::DownloadProgress,
};

use self::image::DownloadItem;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DownloadResult {
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub total_bytes: u64,
    pub failure_records: Vec<FailureRecord>,
}

#[derive(Debug, Clone)]
pub struct Downloader {
    pub options: ResolvedDownloadOptions,
    pub directory: PathBuf,
    session: Arc<PixivNetSession>,
}

impl Downloader {
    pub fn new(
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

    pub async fn download(&self, items: &[DownloadItem]) -> AppResult<DownloadResult> {
        if items.is_empty() {
            return Ok(DownloadResult::default());
        }

        fs::create_dir_all(&self.directory)?;

        let progress =
            DownloadProgress::new(items.len() as u64, build_illust_progress_totals(items));

        let outcomes = stream::iter(items.iter().cloned().map(|item| {
            let downloader = self.clone();
            let progress = progress.clone();

            async move {
                let illust_id = item.illust_id.clone();
                let outcome = downloader.download_one(item, progress.clone()).await;
                let failed = match &outcome {
                    DownloadOutcome::Downloaded(_) => false,
                    DownloadOutcome::Skipped => false,
                    DownloadOutcome::Failed(_) => true,
                };
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DownloadOutcome {
    Downloaded(u64),
    Skipped,
    Failed(FailureRecord),
}

fn build_illust_progress_totals(items: &[DownloadItem]) -> Vec<(String, u64)> {
    let mut totals = BTreeMap::<String, u64>::new();
    for item in items {
        *totals.entry(item.illust_id.clone()).or_default() += 1;
    }
    totals.into_iter().collect()
}

fn file_exists_and_nonempty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::downloader::{build_illust_progress_totals, image::DownloadItem};

    #[test]
    fn illust_progress_totals_are_grouped_by_illust() {
        let items = vec![
            DownloadItem {
                illust_id: "200".to_string(),
                image_url: "https://i.pximg.net/200_p0.png".to_string(),
                target_dir: PathBuf::from("/tmp/200"),
            },
            DownloadItem {
                illust_id: "100".to_string(),
                image_url: "https://i.pximg.net/100_p0.png".to_string(),
                target_dir: PathBuf::from("/tmp/100"),
            },
            DownloadItem {
                illust_id: "200".to_string(),
                image_url: "https://i.pximg.net/200_p1.png".to_string(),
                target_dir: PathBuf::from("/tmp/200"),
            },
        ];

        assert_eq!(
            build_illust_progress_totals(&items),
            vec![("100".to_string(), 1), ("200".to_string(), 2)]
        );
    }
}
