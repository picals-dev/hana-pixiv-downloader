//! 下载模块骨架。

pub mod image;

use crate::{
    config::ResolvedDownloadOptions,
    error::{AppResult, CrawlerError},
};

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
}

impl Downloader {
    pub fn new(options: ResolvedDownloadOptions) -> Self {
        Self { options }
    }

    pub async fn download(&self, _urls: &[String]) -> AppResult<DownloadResult> {
        Err(CrawlerError::not_implemented("Downloader 主流程").into())
    }
}
