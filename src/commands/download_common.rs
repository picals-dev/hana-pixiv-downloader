//! 下载命令共享辅助。

use std::{fmt, path::Path, sync::Arc};

use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use inquire::{InquireError, Select, Text};

use crate::{
    auth::Credential,
    cli::download::RankingMode,
    config::{
        Config, DownloadMode, DownloadOverrides, EnvOverrides, ResolvedDownloadOptions, SortOrder,
    },
    downloader::DownloadResult,
    error::{AppResult, CrawlerError},
    failure::{FailureManifest, ReplayCommand, ReplayOptions},
    net::{PixivNetSession, resolve_base_url, test_hook::attach_session_observer},
    output::{OutputLayout, resolve_output_layout},
    pixiv::selector::{
        count_user_illust_ids, select_bookmark_total, select_keyword_total, select_ranking_total,
    },
    replay::{ReplayExecutionReport, replay_failures_with_session},
};

pub const RANKING_SORT_ERROR: &str = "download ranking 不支持自定义排序；仅允许默认值 date_desc";
pub const RANKING_R18_ERROR: &str =
    "download ranking 不支持通用 R-18 开关；请改用 ranking daily_r18 或 ranking weekly_r18";
pub const RANKING_AI_ERROR: &str = "download ranking 不支持 AI 过滤开关；当前仅允许默认值 ai=true";
const BULK_WARNING_THRESHOLD: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchProbeSummary {
    pub candidate_count: usize,
    pub count_source: &'static str,
    pub subject_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadPresentation {
    pub mode_label: String,
    pub subject_label: String,
    pub candidate_count: Option<usize>,
    pub planned_count: Option<usize>,
    pub order_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RankingModeChoice {
    Daily,
    Weekly,
    Monthly,
    Male,
    Female,
    DailyR18,
    WeeklyR18,
}

impl RankingModeChoice {
    fn options() -> Vec<Self> {
        vec![
            Self::Daily,
            Self::Weekly,
            Self::Monthly,
            Self::Male,
            Self::Female,
            Self::DailyR18,
            Self::WeeklyR18,
        ]
    }

    fn as_api_mode(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Male => "male",
            Self::Female => "female",
            Self::DailyR18 => "daily_r18",
            Self::WeeklyR18 => "weekly_r18",
        }
    }
}

impl fmt::Display for RankingModeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_api_mode())
    }
}

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

pub fn create_shared_session(
    options: &ResolvedDownloadOptions,
    credential: &Credential,
) -> AppResult<Arc<PixivNetSession>> {
    let base_url = resolve_base_url(None)?;
    let builder = attach_session_observer(PixivNetSession::builder(
        options.clone(),
        credential.clone(),
        base_url,
    ));
    Ok(Arc::new(builder.build()?))
}

pub fn print_download_summary(target_directory: &Path, result: &DownloadResult) {
    println!("下载目录: {}", target_directory.display());
    println!(
        "下载完成：图片总数 {}，成功 {}，跳过 {}，失败 {}",
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

pub fn resolve_ranking_mode(mode: Option<RankingMode>) -> AppResult<String> {
    if let Some(mode) = mode {
        return Ok(mode.as_api_mode().to_string());
    }

    let selected = Select::new("请选择要下载的排行榜模式", RankingModeChoice::options())
        .with_starting_cursor(0)
        .prompt()
        .map_err(map_inquire_error)?;
    Ok(selected.as_api_mode().to_string())
}

pub async fn probe_user_count(
    session: &Arc<PixivNetSession>,
    user_id: &str,
) -> AppResult<BatchProbeSummary> {
    let profile = session.fetch_user_profile_all(user_id).await?;
    let count = count_user_illust_ids(&profile)?;
    Ok(BatchProbeSummary {
        candidate_count: count,
        count_source: "profile/all 的 illusts + manga 作品 ID 数量",
        subject_label: format!("画师 {user_id}"),
    })
}

pub async fn probe_keyword_count(
    session: &Arc<PixivNetSession>,
    query: &str,
    order: &str,
    mode: &str,
    include_ai: bool,
) -> AppResult<BatchProbeSummary> {
    let value = session
        .fetch_keyword_page(query, order, mode, 1, include_ai)
        .await?;
    let count = select_keyword_total(&value)?;
    Ok(BatchProbeSummary {
        candidate_count: count,
        count_source: "搜索接口 body.illustManga.total",
        subject_label: format!("关键词 {query}"),
    })
}

pub async fn probe_bookmark_count(
    session: &Arc<PixivNetSession>,
    user_id: &str,
) -> AppResult<BatchProbeSummary> {
    let value = session.fetch_bookmark_page(user_id, 0, 1).await?;
    let count = select_bookmark_total(&value)?;
    Ok(BatchProbeSummary {
        candidate_count: count,
        count_source: "收藏接口 body.total",
        subject_label: format!("账号 {user_id} 的收藏"),
    })
}

pub async fn probe_ranking_count(
    session: &Arc<PixivNetSession>,
    mode: &str,
) -> AppResult<BatchProbeSummary> {
    let value = session.fetch_ranking_page(mode, 1).await?;
    let count = select_ranking_total(&value)?;
    Ok(BatchProbeSummary {
        candidate_count: count,
        count_source: "排行榜接口 rank_total",
        subject_label: format!("排行榜 {mode}"),
    })
}

pub fn print_bulk_probe_summary(summary: &BatchProbeSummary) {
    println!(
        "已探测到候选作品 {} 个（{}）",
        summary.candidate_count, summary.subject_label
    );
    println!("计数来源: {}", summary.count_source);
    if summary.candidate_count > BULK_WARNING_THRESHOLD {
        println!(
            "⚠️ 警告：本次候选作品数已超过 {}，请谨慎确认下载规模，避免一次性下载过多作品。",
            BULK_WARNING_THRESHOLD
        );
    }
}

pub fn resolve_planned_count(
    options: &ResolvedDownloadOptions,
    candidate_count: usize,
) -> AppResult<usize> {
    if candidate_count == 0 {
        return Ok(0);
    }

    if options.count > 0 {
        return Ok(options.count.min(candidate_count));
    }

    if options.dry_run {
        return Ok(candidate_count);
    }

    let value = Text::new("本次下载作品数（直接回车表示全部）")
        .with_default(&candidate_count.to_string())
        .with_help_message("请输入 1 到候选作品总数之间的整数")
        .prompt()
        .map_err(map_inquire_error)?;
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Ok(candidate_count);
    }

    let count = trimmed
        .parse::<usize>()
        .map_err(|_| CrawlerError::InvalidInput("下载作品数需要是无符号整数".to_string()))?;
    if count == 0 {
        return Err(CrawlerError::InvalidInput("下载作品数必须大于 0".to_string()).into());
    }
    if count > candidate_count {
        return Err(CrawlerError::InvalidInput(format!(
            "下载作品数不能超过候选作品总数 {candidate_count}"
        ))
        .into());
    }

    Ok(count)
}

pub fn render_order_label(mode: DownloadMode, sort: SortOrder) -> String {
    match mode {
        DownloadMode::Ranking => "按 Pixiv 榜单顺序".to_string(),
        _ => match sort {
            SortOrder::DateDesc => "按发布时间从新到旧".to_string(),
            SortOrder::DateAsc => "按发布时间从旧到新".to_string(),
            SortOrder::PopularDesc => "按热度从高到低".to_string(),
        },
    }
}

pub fn print_download_config_table(
    presentation: &DownloadPresentation,
    options: &ResolvedDownloadOptions,
    target_directory: &Path,
) {
    let candidate = presentation
        .candidate_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "单作品".to_string());
    let planned = presentation
        .planned_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "单作品".to_string());
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["项目", "当前值"]);

    table.add_row(vec!["模式", presentation.mode_label.as_str()]);
    table.add_row(vec!["目标", presentation.subject_label.as_str()]);
    table.add_row(vec!["下载目录", &target_directory.display().to_string()]);
    table.add_row(vec!["候选作品数", candidate.as_str()]);
    table.add_row(vec!["本次下载作品数", planned.as_str()]);
    table.add_row(vec!["顺序", presentation.order_label.as_str()]);
    table.add_row(vec!["并发下载数", &options.concurrent.to_string()]);
    table.add_row(vec!["单次请求超时", &format!("{} 秒", options.timeout)]);
    table.add_row(vec!["网络重试次数", &options.retry.to_string()]);
    table.add_row(vec![
        "导出 tags.json",
        if options.with_tags {
            "开启"
        } else {
            "关闭"
        },
    ]);
    table.add_row(vec![
        "代理",
        options.proxy_url.as_deref().unwrap_or("<未设置>"),
    ]);

    println!("{table}");
}

fn map_inquire_error(error: InquireError) -> eyre::Report {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            eyre::eyre!("操作已取消")
        }
        other => eyre::Report::new(other).wrap_err("交互式输入失败"),
    }
}

pub async fn finalize_download_result(
    session: Arc<PixivNetSession>,
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
        let replay_report = replay_failures_with_session(
            Arc::clone(&session),
            &replay_command,
            result.failure_records.clone(),
        )
        .await?;
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
    use std::{
        collections::BTreeSet,
        sync::{Arc, Mutex},
    };

    use tempfile::tempdir;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use crate::{
        auth::Credential,
        config::SortOrder,
        failure::{FailureManifest, FailureRecord, FailureStage},
        net::{NetEvent, PixivNetSession},
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
        let events = Arc::new(Mutex::new(Vec::<NetEvent>::new()));
        let observer_events = Arc::clone(&events);
        let session = Arc::new(
            PixivNetSession::builder(
                options.to_resolved(crate::config::DownloadMode::Illust),
                credential.clone(),
                server.uri().parse().unwrap(),
            )
            .with_observer(Arc::new(move |event| {
                observer_events.lock().unwrap().push(event);
            }))
            .build()
            .unwrap(),
        );
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

        let finalized = finalize_download_result(Arc::clone(&session), command, result)
            .await
            .unwrap();

        assert_eq!(finalized.failed, 0);
        assert_eq!(finalized.downloaded, 1);
        assert!(finalized.failure_records.is_empty());
        let session_ids = events
            .lock()
            .unwrap()
            .iter()
            .map(|event| match event {
                NetEvent::Attempt { session_id, .. }
                | NetEvent::Retry { session_id, .. }
                | NetEvent::Failure { session_id, .. }
                | NetEvent::Cooldown { session_id, .. }
                | NetEvent::TransferProgress { session_id, .. }
                | NetEvent::TransferCompleted { session_id, .. } => *session_id,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(session_ids.len(), 1);
        assert_eq!(
            session_ids.into_iter().next().unwrap(),
            session.session_id()
        );
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
        let session = Arc::new(
            PixivNetSession::new_with_base_url(
                options.to_resolved(crate::config::DownloadMode::Illust),
                credential.clone(),
                "https://www.pixiv.net".parse().unwrap(),
            )
            .unwrap(),
        );
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

        let finalized = finalize_download_result(session, command, result)
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
