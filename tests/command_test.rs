use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use clap::{CommandFactory, Parser};
use picals_crawler::{
    auth::Credential,
    cli::{Cli, Command, download::DownloadSubcommand},
    commands,
    config::{Config, DownloadConfig, POPULAR_SORT_MIGRATION_MESSAGE, ProxyConfig, SortOrder},
    failure::{FailureManifest, FailureRecord, FailureStage, ReplayCommand, ReplayOptions},
    net::NetEvent,
    test_support::{EnvVarGuard, install_session_observer, lock_env, set_config_home},
};
use tempfile::tempdir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

fn observed_session_ids(events: &[NetEvent]) -> BTreeSet<u64> {
    events
        .iter()
        .map(|event| match event {
            NetEvent::Attempt { session_id, .. }
            | NetEvent::Retry { session_id, .. }
            | NetEvent::Failure { session_id, .. }
            | NetEvent::Cooldown { session_id, .. }
            | NetEvent::TransferCompleted { session_id, .. } => *session_id,
        })
        .collect()
}

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

#[test]
fn ranking_cli_accepts_positional_mode() {
    let cli = Cli::parse_from(["picals-crawler", "download", "ranking", "daily"]);

    match cli.command {
        picals_crawler::cli::Command::Download(download) => match download.target {
            Some(DownloadSubcommand::Ranking(args)) => {
                assert_eq!(
                    args.mode,
                    Some(picals_crawler::cli::download::RankingMode::Daily)
                );
            }
            _ => panic!("expected ranking command"),
        },
        _ => panic!("expected download command"),
    }
}

#[test]
fn ranking_cli_rejects_legacy_mode_flag() {
    let error = Cli::try_parse_from(["picals-crawler", "download", "ranking", "--mode", "daily"])
        .unwrap_err();

    assert!(error.to_string().contains("--mode"));
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
    let server = MockServer::start().await;
    let _config_home = set_config_home(temp.path());
    let _base_url = EnvVarGuard::set("PICALS_PIXIV_BASE_URL", server.uri());
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    Credential::new_with_user_id("cookie", Some("12345678"))
        .unwrap()
        .save()
        .unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/illusts/bookmarks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "total": 3,
                "works": [{ "id": "1" }]
            }
        })))
        .mount(&server)
        .await;

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
            Some(DownloadSubcommand::Keyword(args)) => {
                assert_eq!(args.query, "初音ミク");
                assert!(args.r18);
            }
            _ => panic!("expected keyword command"),
        },
        _ => panic!("expected download command"),
    }
}

#[test]
fn direct_download_cli_accepts_pixiv_url() {
    let cli = Cli::parse_from([
        "picals-crawler",
        "download",
        "https://www.pixiv.net/users/12345678",
        "--dry-run",
    ]);

    match cli.command {
        picals_crawler::cli::Command::Download(download) => {
            assert!(download.target.is_none());
            assert_eq!(
                download.direct.pixiv_url.as_deref(),
                Some("https://www.pixiv.net/users/12345678")
            );
            assert!(download.direct.common.dry_run);
        }
        _ => panic!("expected download command"),
    }
}

#[test]
fn direct_download_cli_accepts_encoded_tag_url() {
    let cli = Cli::parse_from([
        "picals-crawler",
        "download",
        "https://www.pixiv.net/tags/%E5%88%9D%E9%9F%B3%E3%83%9F%E3%82%AF/artworks",
    ]);

    match cli.command {
        picals_crawler::cli::Command::Download(download) => {
            assert!(download.target.is_none());
            assert_eq!(
                download.direct.pixiv_url.as_deref(),
                Some("https://www.pixiv.net/tags/%E5%88%9D%E9%9F%B3%E3%83%9F%E3%82%AF/artworks")
            );
        }
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
async fn auto_replay_reuses_same_session_instance_for_single_download_command() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let server = MockServer::start().await;
    let _config_home = set_config_home(temp.path());
    let _base_url = EnvVarGuard::set("PICALS_PIXIV_BASE_URL", server.uri());
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    Credential::new("cookie").unwrap().save().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": { "illusts": { "123456": {} }, "manga": {} }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": [{
                "urls": {
                    "original": format!("{}/img-original/123456_p0.png", server.uri())
                }
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/123456_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/123456_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    let events = Arc::new(Mutex::new(Vec::<NetEvent>::new()));
    let observer_events = Arc::clone(&events);
    let _observer = install_session_observer(Arc::new(move |event| {
        observer_events.lock().unwrap().push(event);
    }));

    let cli = Cli::parse_from([
        "picals-crawler",
        "download",
        "user",
        "12345678",
        "--count",
        "1",
        "--retry",
        "1",
        "--to",
        temp.path().join("user-root").to_string_lossy().as_ref(),
    ]);
    commands::dispatch(cli).await.unwrap();

    let session_ids = observed_session_ids(&events.lock().unwrap());
    assert_eq!(session_ids.len(), 1);
    assert!(
        temp.path()
            .join("user-root/12345678/123456/123456_p0.png")
            .exists()
    );
}

#[tokio::test]
async fn standalone_retry_uses_new_session_instance_but_same_net_stack() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let server = MockServer::start().await;
    let _config_home = set_config_home(temp.path());
    let _base_url = EnvVarGuard::set("PICALS_PIXIV_BASE_URL", server.uri());
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    Credential::new("cookie").unwrap().save().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/illust/111111/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": [{
                "urls": {
                    "original": format!("{}/img-original/111111_p0.png", server.uri())
                }
            }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/img-original/111111_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/111111", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/img-original/222222_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/222222", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    let command_events = Arc::new(Mutex::new(Vec::<NetEvent>::new()));
    let command_observer_events = Arc::clone(&command_events);
    let _command_observer = install_session_observer(Arc::new(move |event| {
        command_observer_events.lock().unwrap().push(event);
    }));

    let cli = Cli::parse_from([
        "picals-crawler",
        "download",
        "illust",
        "111111",
        "--to",
        temp.path().join("illust-root").to_string_lossy().as_ref(),
    ]);
    commands::dispatch(cli).await.unwrap();
    let command_session_ids = observed_session_ids(&command_events.lock().unwrap());
    assert_eq!(command_session_ids.len(), 1);
    drop(_command_observer);

    let manifest_path = temp.path().join("retry-manifest.json");
    let manifest = FailureManifest::new(
        ReplayCommand::Illust {
            illust_id: "222222".to_string(),
            options: ReplayOptions {
                directory: temp
                    .path()
                    .join("retry-root")
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
            illust_id: Some("222222".to_string()),
            image_url: Some(format!("{}/img-original/222222_p0.png", server.uri())),
            target_path: Some(
                temp.path()
                    .join("retry-root/222222/222222_p0.png")
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

    let retry_events = Arc::new(Mutex::new(Vec::<NetEvent>::new()));
    let retry_observer_events = Arc::clone(&retry_events);
    let _retry_observer = install_session_observer(Arc::new(move |event| {
        retry_observer_events.lock().unwrap().push(event);
    }));

    let cli = Cli::parse_from([
        "picals-crawler",
        "retry",
        manifest_path.to_string_lossy().as_ref(),
    ]);
    commands::dispatch(cli).await.unwrap();

    let retry_session_ids = observed_session_ids(&retry_events.lock().unwrap());
    assert_eq!(retry_session_ids.len(), 1);
    assert_ne!(
        command_session_ids.into_iter().next().unwrap(),
        retry_session_ids.into_iter().next().unwrap()
    );
    assert!(temp.path().join("retry-root/222222/222222_p0.png").exists());
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
