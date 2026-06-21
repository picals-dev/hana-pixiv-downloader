//! 下载模块骨架。

pub mod image;

use std::{
    fs,
    path::{Path, PathBuf},
};

use futures::{StreamExt, stream};
use log::warn;
use url::Url;

use crate::{
    auth::Credential,
    config::ResolvedDownloadOptions,
    error::{AppResult, CrawlerError},
    failure::{FailureRecord, FailureStage},
    net::{PixivRequestRuntime, RequestClass},
    utils::progress::DownloadProgress,
};

use self::image::{DownloadItem, illust_id_from_image_url};

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
    referer_base_url: Url,
    runtime: PixivRequestRuntime,
}

impl Downloader {
    pub fn new(
        options: ResolvedDownloadOptions,
        directory: PathBuf,
        referer_base_url: Url,
        credential: &Credential,
    ) -> AppResult<Self> {
        Ok(Self {
            directory,
            runtime: PixivRequestRuntime::new(&options, credential)?,
            options,
            referer_base_url,
        })
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

        let referer = match build_referer(&self.referer_base_url, &item.image_url) {
            Ok(referer) => referer,
            Err(error) => {
                warn!("构造 Referer 失败 {}: {error}", item.image_url);
                let report = eyre::Report::new(error);
                return DownloadOutcome::Failed(FailureRecord::from_report(
                    self.options.mode,
                    FailureStage::Download,
                    Some(item.illust_id.clone()),
                    Some(item.image_url.clone()),
                    Some(target_path.to_string_lossy().into_owned()),
                    &report,
                ));
            }
        };

        let temp_path = temporary_download_path(&target_path);
        let _ = fs::remove_file(&temp_path);
        let url = item.image_url;
        let download_result = async {
            let response = self
                .runtime
                .get_bytes(
                    Url::parse(&url)?,
                    Some(referer),
                    RequestClass::ImageDownload,
                )
                .await?;

            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }

            fs::write(&temp_path, &response.bytes)?;
            fs::rename(&temp_path, &target_path)?;

            Ok::<u64, eyre::Report>(response.bytes.len() as u64)
        }
        .await;

        match download_result {
            Ok(bytes) => DownloadOutcome::Downloaded(bytes),
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
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

fn build_referer(referer_base_url: &Url, url: &str) -> Result<String, CrawlerError> {
    let illust_id = illust_id_from_image_url(url)?;
    Ok(referer_base_url
        .join(&format!("/artworks/{illust_id}"))?
        .to_string())
}

fn file_exists_and_nonempty(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn temporary_download_path(path: &Path) -> PathBuf {
    let mut extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    if extension.is_empty() {
        extension = "part".to_string();
    } else {
        extension.push_str(".part");
    }

    path.with_extension(extension)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::temporary_download_path;

    #[test]
    fn temporary_path_uses_part_extension() {
        let path = PathBuf::from("/tmp/a.png");
        assert_eq!(
            temporary_download_path(&path),
            PathBuf::from("/tmp/a.png.part")
        );
    }
}
