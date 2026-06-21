use clap::{CommandFactory, Parser};
use picals_crawler::{
    auth::Credential,
    cli::{Cli, Command, download::DownloadSubcommand},
    commands,
    config::{Config, DownloadConfig, POPULAR_SORT_MIGRATION_MESSAGE, ProxyConfig, SortOrder},
    failure::{FailureManifest, FailureRecord, FailureStage, ReplayCommand, ReplayOptions},
    test_support::{EnvVarGuard, lock_env, set_config_home},
};
use tempfile::tempdir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

#[test]
fn ranking_help_does_not_expose_sort_r18_or_no_ai() {
    let mut command = Cli::command();
    let rendered = command.render_long_help().to_string();
    assert!(rendered.contains("download"));

    let mut ranking = Cli::command()
        .find_subcommand_mut("download")
        .unwrap()
        .find_subcommand_mut("ranking")
        .unwrap()
        .clone();
    let help = ranking.render_long_help().to_string();

    assert!(!help.contains("--sort"));
    assert!(!help.contains("--r18"));
    assert!(!help.contains("--no-ai"));
}

#[test]
fn ranking_cli_rejects_sort_flag_at_parse_time() {
    let error = Cli::try_parse_from([
        "picals-crawler",
        "download",
        "ranking",
        "--sort",
        "date_desc",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("--sort"));
}

#[tokio::test]
async fn ranking_rejects_non_default_values_from_config() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");

    Config {
        download: DownloadConfig {
            sort: SortOrder::DateAsc,
            ..DownloadConfig::default()
        },
        proxy: ProxyConfig::default(),
    }
    .save()
    .unwrap();

    let cli = Cli::parse_from(["picals-crawler", "download", "ranking", "--dry-run"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("download ranking 不支持自定义排序"));
}

#[tokio::test]
async fn ranking_rejects_ai_false_from_env() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    let _ai = EnvVarGuard::set("PICALS_DOWNLOAD_AI", "false");

    let cli = Cli::parse_from(["picals-crawler", "download", "ranking", "--dry-run"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("download ranking 不支持 AI 过滤开关"));
}

#[tokio::test]
async fn bookmark_requires_user_id_in_credential() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    Credential::new("cookie").unwrap().save().unwrap();

    let cli = Cli::parse_from(["picals-crawler", "download", "bookmark"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("缺少 userId"));
}

#[tokio::test]
async fn bookmark_dry_run_accepts_credential_with_user_id() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    Credential::new_with_user_id("cookie", Some("12345678"))
        .unwrap()
        .save()
        .unwrap();

    let cli = Cli::parse_from(["picals-crawler", "download", "bookmark", "--dry-run"]);
    commands::dispatch(cli).await.unwrap();
}

#[tokio::test]
async fn env_popular_sort_returns_migration_error() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    let _sort = EnvVarGuard::set("PICALS_DOWNLOAD_SORT", "popular_desc");

    let cli = Cli::parse_from(["picals-crawler", "download", "illust", "123"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains(POPULAR_SORT_MIGRATION_MESSAGE));
}

#[test]
fn keyword_cli_still_uses_query_and_r18_only() {
    let cli = Cli::parse_from(["picals-crawler", "download", "keyword", "初音ミク", "--r18"]);

    match cli.command {
        picals_crawler::cli::Command::Download(download) => match download.target {
            DownloadSubcommand::Keyword(args) => {
                assert_eq!(args.query, "初音ミク");
                assert!(args.r18);
            }
            _ => panic!("expected keyword command"),
        },
        _ => panic!("expected download command"),
    }
}

#[test]
fn retry_cli_accepts_manifest_path() {
    let cli = Cli::parse_from(["picals-crawler", "retry", "/tmp/failures/demo.json"]);

    match cli.command {
        Command::Retry(args) => {
            assert_eq!(
                args.manifest_path,
                std::path::PathBuf::from("/tmp/failures/demo.json")
            );
        }
        _ => panic!("expected retry command"),
    }
}

#[tokio::test]
async fn retry_command_can_read_manifest_file() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let manifest_path = temp.path().join("demo.json");
    Credential::new("cookie").unwrap().save().unwrap();

    let manifest = FailureManifest::new(
        ReplayCommand::Illust {
            illust_id: "123456".to_string(),
            options: ReplayOptions {
                directory: "/tmp/picals/illust".to_string(),
                count: 1,
                sort: SortOrder::DateDesc,
                r18: false,
                ai: true,
                concurrent: 2,
                timeout: 30,
                retry: 2,
                with_tags: false,
                proxy_url: None,
                dry_run: false,
            },
        },
        vec![FailureRecord {
            mode: picals_crawler::config::DownloadMode::Illust,
            stage: FailureStage::Download,
            illust_id: Some("123456".to_string()),
            image_url: Some("https://example.com/123456_p0.png".to_string()),
            target_path: Some("/tmp/picals/illust/123456/123456_p0.png".to_string()),
            error_kind: "timeout".to_string(),
            error_message: "timeout".to_string(),
            retryable: true,
        }],
    )
    .unwrap();
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let cli = Cli::parse_from([
        "picals-crawler",
        "retry",
        manifest_path.to_string_lossy().as_ref(),
    ]);
    commands::dispatch(cli).await.unwrap();
}

#[tokio::test]
async fn retry_command_can_recover_retryable_download_record() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let manifest_path = temp.path().join("retryable.json");
    let server = MockServer::start().await;
    Credential::new("cookie").unwrap().save().unwrap();

    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    let manifest = FailureManifest::new(
        ReplayCommand::Illust {
            illust_id: "123456".to_string(),
            options: ReplayOptions {
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
            },
        },
        vec![FailureRecord {
            mode: picals_crawler::config::DownloadMode::Illust,
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
    )
    .unwrap();
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let _base_url = EnvVarGuard::set("PICALS_PIXIV_BASE_URL", server.uri());
    let cli = Cli::parse_from([
        "picals-crawler",
        "retry",
        manifest_path.to_string_lossy().as_ref(),
    ]);
    commands::dispatch(cli).await.unwrap();

    assert!(
        temp.path()
            .join("illust-root/123456/123456_p0.png")
            .exists()
    );
}

#[tokio::test]
async fn retry_command_skips_non_retryable_record() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let manifest_path = temp.path().join("non-retryable.json");
    Credential::new("cookie").unwrap().save().unwrap();

    let manifest = FailureManifest::new(
        ReplayCommand::Illust {
            illust_id: "123456".to_string(),
            options: ReplayOptions {
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
            },
        },
        vec![FailureRecord {
            mode: picals_crawler::config::DownloadMode::Illust,
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
    )
    .unwrap();
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let cli = Cli::parse_from([
        "picals-crawler",
        "retry",
        manifest_path.to_string_lossy().as_ref(),
    ]);
    commands::dispatch(cli).await.unwrap();

    assert!(
        !temp
            .path()
            .join("illust-root/123456/123456_p0.png")
            .exists()
    );
}
