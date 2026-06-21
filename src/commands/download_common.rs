//! 下载命令共享辅助。

use std::path::Path;

use crate::{
    auth::Credential,
    config::{
        Config, DownloadMode, DownloadOverrides, EnvOverrides, ResolvedDownloadOptions, SortOrder,
    },
    downloader::DownloadResult,
    error::{AppResult, CrawlerError},
    failure::{FailureManifest, ReplayCommand, ReplayOptions},
    output::{OutputLayout, resolve_output_layout},
    replay::{ReplayExecutionReport, replay_failures},
};

pub const RANKING_SORT_ERROR: &str = "download ranking 不支持自定义排序；仅允许默认值 date_desc";
pub const RANKING_R18_ERROR: &str =
    "download ranking 不支持通用 R-18 开关；请改用 --mode daily_r18 或 weekly_r18";
pub const RANKING_AI_ERROR: &str = "download ranking 不支持 AI 过滤开关；当前仅允许默认值 ai=true";

pub fn resolve_options(
    mode: DownloadMode,
    overrides: &DownloadOverrides,
) -> AppResult<ResolvedDownloadOptions> {
    let config = Config::load()?;
    let env = EnvOverrides::from_process_env()?;
    config.resolve_download_options(mode, &env, overrides)
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

pub fn resolve_layout(options: &ResolvedDownloadOptions, subject: &str) -> AppResult<OutputLayout> {
    resolve_output_layout(options.mode, &options.directory, subject)
}

pub async fn finalize_download_result(
    credential: &Credential,
    command: ReplayCommand,
    mut result: DownloadResult,
) -> AppResult<DownloadResult> {
    if result.failure_records.is_empty() {
        return Ok(result);
    }

    let retryable_records = result
        .failure_records
        .iter()
        .filter(|record| record.retryable)
        .count();
    if retryable_records > 0 {
        println!(
            "检测到 {} 个可重试失败项，开始自动补拉一次。",
            retryable_records
        );
        let replay_command = command.with_retry_profile();
        let replay_report =
            replay_failures(credential, &replay_command, result.failure_records.clone()).await?;
        result = apply_replay_report(result, replay_report);
    }

    if !result.failure_records.is_empty() {
        let manifest = FailureManifest::new(command, result.failure_records.clone())?;
        let manifest_path = manifest.save()?;
        println!("仍有 {} 个失败项未恢复。", result.failure_records.len());
        println!("失败清单已保存到: {}", manifest_path.display());
        println!("可使用以下命令继续补拉：");
        println!("  picals-crawler retry {}", manifest_path.display());
    }

    Ok(result)
}

fn apply_replay_report(
    mut result: DownloadResult,
    replay_report: ReplayExecutionReport,
) -> DownloadResult {
    if replay_report.recovered > 0 {
        result.downloaded += replay_report.recovered;
        result.failed = result.failed.saturating_sub(replay_report.recovered);
    }
    result.failure_records = replay_report.remaining_records;
    result
}

pub fn build_replay_command(
    mode: DownloadMode,
    options: &ResolvedDownloadOptions,
    subject: &str,
    extra: Option<&str>,
) -> ReplayCommand {
    let replay_options = ReplayOptions::from(options);

    match mode {
        DownloadMode::User => ReplayCommand::User {
            user_id: subject.to_string(),
            options: replay_options,
        },
        DownloadMode::Illust => ReplayCommand::Illust {
            illust_id: subject.to_string(),
            options: replay_options,
        },
        DownloadMode::Bookmark => ReplayCommand::Bookmark {
            user_id: subject.to_string(),
            options: replay_options,
        },
        DownloadMode::Keyword => ReplayCommand::Keyword {
            query: subject.to_string(),
            r18: extra == Some("r18"),
            options: replay_options,
        },
        DownloadMode::Ranking => ReplayCommand::Ranking {
            mode: subject.to_string(),
            options: replay_options,
        },
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use crate::{
        auth::Credential,
        config::SortOrder,
        failure::{FailureManifest, FailureRecord, FailureStage},
        test_support::{EnvVarGuard, lock_env},
    };

    use super::{build_replay_command, finalize_download_result};

    #[tokio::test]
    async fn finalize_download_result_replays_retryable_failure() {
        let _lock = lock_env().await;
        let temp = tempdir().unwrap();
        let _xdg = EnvVarGuard::set("XDG_CONFIG_HOME", temp.path());
        let server = MockServer::start().await;
        let _base_url = EnvVarGuard::set("PICALS_PIXIV_BASE_URL", server.uri());

        Mock::given(method("GET"))
            .and(path("/img-original/img/2024/01/02/03/04/05/123456_p0.png"))
            .and(header(
                "referer",
                format!("{}/artworks/123456", server.uri()),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;

        let options = crate::failure::ReplayOptions {
            directory: temp
                .path()
                .join("illust-root")
                .to_string_lossy()
                .into_owned(),
            count: 1,
            sort: SortOrder::DateDesc,
            r18: false,
            ai: true,
            concurrent: 1,
            timeout: 30,
            retry: 2,
            with_tags: false,
            proxy_url: None,
            dry_run: false,
        };
        let command = build_replay_command(
            crate::config::DownloadMode::Illust,
            &options.to_resolved(crate::config::DownloadMode::Illust),
            "123456",
            None,
        );
        let credential = Credential::new("cookie").unwrap();
        let result = crate::downloader::DownloadResult {
            total: 1,
            downloaded: 0,
            skipped: 0,
            failed: 1,
            total_bytes: 0,
            failure_records: vec![FailureRecord {
                mode: crate::config::DownloadMode::Illust,
                stage: FailureStage::Download,
                illust_id: Some("123456".to_string()),
                image_url: Some(format!(
                    "{}/img-original/img/2024/01/02/03/04/05/123456_p0.png",
                    server.uri()
                )),
                target_path: Some(
                    temp.path()
                        .join("illust-root/123456/123456_p0.png")
                        .to_string_lossy()
                        .into_owned(),
                ),
                error_kind: "timeout".to_string(),
                error_message: "timeout".to_string(),
                retryable: true,
            }],
        };

        let finalized = finalize_download_result(&credential, command, result)
            .await
            .unwrap();

        assert_eq!(finalized.failed, 0);
        assert_eq!(finalized.downloaded, 1);
        assert!(finalized.failure_records.is_empty());
        assert!(
            temp.path()
                .join("illust-root/123456/123456_p0.png")
                .exists()
        );
    }

    #[tokio::test]
    async fn finalize_download_result_writes_manifest_for_remaining_failures() {
        let _lock = lock_env().await;
        let temp = tempdir().unwrap();
        let _xdg = EnvVarGuard::set("XDG_CONFIG_HOME", temp.path());

        let options = crate::failure::ReplayOptions {
            directory: temp
                .path()
                .join("illust-root")
                .to_string_lossy()
                .into_owned(),
            count: 1,
            sort: SortOrder::DateDesc,
            r18: false,
            ai: true,
            concurrent: 1,
            timeout: 30,
            retry: 2,
            with_tags: false,
            proxy_url: None,
            dry_run: false,
        };
        let command = build_replay_command(
            crate::config::DownloadMode::Illust,
            &options.to_resolved(crate::config::DownloadMode::Illust),
            "123456",
            None,
        );
        let credential = Credential::new("cookie").unwrap();
        let result = crate::downloader::DownloadResult {
            total: 1,
            downloaded: 0,
            skipped: 0,
            failed: 1,
            total_bytes: 0,
            failure_records: vec![FailureRecord {
                mode: crate::config::DownloadMode::Illust,
                stage: FailureStage::Download,
                illust_id: Some("123456".to_string()),
                image_url: Some("https://example.invalid/123456_p0.png".to_string()),
                target_path: Some(
                    temp.path()
                        .join("illust-root/123456/123456_p0.png")
                        .to_string_lossy()
                        .into_owned(),
                ),
                error_kind: "auth".to_string(),
                error_message: "auth".to_string(),
                retryable: false,
            }],
        };

        let finalized = finalize_download_result(&credential, command, result)
            .await
            .unwrap();

        assert_eq!(finalized.failed, 1);
        assert_eq!(finalized.failure_records.len(), 1);

        let failures_dir = crate::config::ensure_config_dir().unwrap().join("failures");
        let manifest_paths = std::fs::read_dir(&failures_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(manifest_paths.len(), 1);
        let manifest = FailureManifest::load_from(&manifest_paths[0]).unwrap();
        assert_eq!(manifest.records.len(), 1);
        assert_eq!(manifest.records[0].error_kind, "auth");
    }
}
