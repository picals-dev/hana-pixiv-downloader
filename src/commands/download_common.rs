//! 下载命令共享辅助。

use std::path::Path;

use crate::{
    auth::Credential,
    config::{Config, DownloadOverrides, EnvOverrides, ResolvedDownloadOptions, SortOrder},
    downloader::DownloadResult,
    error::{AppResult, CrawlerError},
};

pub const RANKING_SORT_ERROR: &str = "download ranking 不支持自定义排序；仅允许默认值 date_desc";
pub const RANKING_R18_ERROR: &str =
    "download ranking 不支持通用 R-18 开关；请改用 --mode daily_r18 或 weekly_r18";
pub const RANKING_AI_ERROR: &str = "download ranking 不支持 AI 过滤开关；当前仅允许默认值 ai=true";

pub fn resolve_options(overrides: &DownloadOverrides) -> AppResult<ResolvedDownloadOptions> {
    let config = Config::load()?;
    let env = EnvOverrides::from_process_env()?;
    config.resolve_download_options(&env, overrides)
}

pub fn load_required_credential() -> AppResult<Credential> {
    Credential::load()?.ok_or(CrawlerError::MissingCredential.into())
}

pub fn print_download_summary(target_directory: &Path, result: &DownloadResult) {
    println!("下载目录: {}", target_directory.display());
    println!(
        "下载完成：总数 {}，成功 {}，跳过 {}，失败 {}",
        result.total, result.downloaded, result.skipped, result.failed
    );
}

pub fn ensure_ranking_defaults(options: &ResolvedDownloadOptions) -> AppResult<()> {
    if options.sort != SortOrder::DateDesc {
        return Err(CrawlerError::InvalidInput(RANKING_SORT_ERROR.to_string()).into());
    }

    if options.r18 {
        return Err(CrawlerError::InvalidInput(RANKING_R18_ERROR.to_string()).into());
    }

    if !options.ai {
        return Err(CrawlerError::InvalidInput(RANKING_AI_ERROR.to_string()).into());
    }

    Ok(())
}
