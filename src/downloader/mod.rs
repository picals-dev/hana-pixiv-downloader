//! 下载模块骨架。

pub mod image;

use std::{
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

        let progress = DownloadProgress::new(items.len() as u64);
        progress.set_message(format!("下载目录 {}", self.directory.display()));

        let outcomes = stream::iter(items.iter().cloned().map(|item| {
            let downloader = self.clone();
            let progress = progress.clone();

            async move {
                let outcome = downloader.download_one(item).await;
                progress.inc(1);
                if let DownloadOutcome::Downloaded(bytes) = outcome {
                    progress.record_downloaded_bytes(bytes);
                }
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

    async fn download_one(&self, item: DownloadItem) -> DownloadOutcome {
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
            let bytes = self
                .session
                .download_original_image(&url, &item.illust_id, &target_path)
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

fn file_exists_and_nonempty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}
