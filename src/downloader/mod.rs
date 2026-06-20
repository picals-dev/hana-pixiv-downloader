//! 下载模块骨架。

pub mod image;

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use eyre::eyre;
use futures::{StreamExt, stream};
use log::warn;
use reqwest::{
    Client, Proxy,
    header::{REFERER, USER_AGENT},
};
use url::Url;

use crate::{
    collector::DEFAULT_USER_AGENT,
    config::ResolvedDownloadOptions,
    error::{AppResult, CrawlerError},
    utils::{progress::DownloadProgress, retry::retry_async},
};

use self::image::{illust_id_from_image_url, target_path_for_image};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DownloadResult {
    pub total: usize,
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct Downloader {
    pub options: ResolvedDownloadOptions,
    pub directory: PathBuf,
    referer_base_url: Url,
    client: Client,
}

impl Downloader {
    pub fn new(
        options: ResolvedDownloadOptions,
        directory: PathBuf,
        referer_base_url: Url,
    ) -> AppResult<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(options.timeout))
            .user_agent(DEFAULT_USER_AGENT);

        if let Some(proxy_url) = options.proxy_url.as_deref() {
            builder = builder.proxy(
                Proxy::all(proxy_url)
                    .map_err(|error| CrawlerError::Config(format!("代理配置无效: {error}")))?,
            );
        }

        Ok(Self {
            directory,
            client: builder.build()?,
            options,
            referer_base_url,
        })
    }

    pub async fn download(&self, urls: &[String]) -> AppResult<DownloadResult> {
        if urls.is_empty() {
            return Ok(DownloadResult::default());
        }

        fs::create_dir_all(&self.directory)?;

        let progress = DownloadProgress::new(urls.len() as u64);
        progress.set_message(format!("下载目录 {}", self.directory.display()));

        let outcomes = stream::iter(urls.iter().cloned().map(|url| {
            let downloader = self.clone();
            let progress = progress.clone();

            async move {
                let outcome = downloader.download_one(&url).await;
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
                DownloadOutcome::Failed => result.failed += 1,
            }
        }

        Ok(result)
    }

    async fn download_one(&self, url: &str) -> DownloadOutcome {
        let target_path = match target_path_for_image(&self.directory, url) {
            Ok(path) => path,
            Err(error) => {
                warn!("解析下载路径失败 {url}: {error}");
                return DownloadOutcome::Failed;
            }
        };

        if file_exists_and_nonempty(&target_path) {
            return DownloadOutcome::Skipped;
        }

        let referer = match build_referer(&self.referer_base_url, url) {
            Ok(referer) => referer,
            Err(error) => {
                warn!("构造 Referer 失败 {url}: {error}");
                return DownloadOutcome::Failed;
            }
        };

        let temp_path = temporary_download_path(&target_path);
        let _ = fs::remove_file(&temp_path);
        let url = url.to_string();
        let client = self.client.clone();
        let retry_count = self.options.retry;

        let download_result = retry_async(retry_count, Duration::from_millis(200), |_| {
            let client = client.clone();
            let referer = referer.clone();
            let temp_path = temp_path.clone();
            let target_path = target_path.clone();
            let url = url.clone();

            async move {
                let response = client
                    .get(&url)
                    .header(REFERER, referer)
                    .header(USER_AGENT, DEFAULT_USER_AGENT)
                    .send()
                    .await?
                    .error_for_status()?;

                let expected_length = response.content_length();
                let bytes = response.bytes().await?;

                if bytes.is_empty() {
                    return Err(eyre!(CrawlerError::DownloadInterrupted(format!(
                        "下载到空文件: {url}"
                    ))));
                }

                if let Some(expected_length) = expected_length
                    && bytes.len() as u64 != expected_length
                {
                    return Err(eyre!(CrawlerError::DownloadInterrupted(format!(
                        "下载内容长度不匹配: {url}"
                    ))));
                }

                if let Some(parent) = target_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::write(&temp_path, &bytes)?;
                fs::rename(&temp_path, &target_path)?;

                Ok::<u64, eyre::Report>(bytes.len() as u64)
            }
        })
        .await;

        match download_result {
            Ok(bytes) => DownloadOutcome::Downloaded(bytes),
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                warn!("下载失败 {url}: {error}");
                DownloadOutcome::Failed
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadOutcome {
    Downloaded(u64),
    Skipped,
    Failed,
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
