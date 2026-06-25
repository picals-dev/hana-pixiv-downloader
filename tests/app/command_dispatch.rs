use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use clap::Parser;
use picals_crawler::{
    auth::Credential,
    cli::Cli,
    commands,
    config::{Config, SortOrder},
    failure::{FailureManifest, FailureRecord, FailureStage, ReplayCommand, ReplayOptions},
    net::NetEvent,
};
use tempfile::tempdir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::support::env::{
    EnvVarGuard, install_observer, lock_env, set_base_url, set_config_home, unset_download_env,
};

async fn mount_illust_detail(server: &MockServer, illust_id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/illust/{illust_id}")))
        .and(header(
            "referer",
            format!("{}/artworks/{illust_id}", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illustType": 0,
                "tags": {
                    "tags": [{ "tag": "static" }]
                }
            }
        })))
        .mount(server)
        .await;
}

fn observed_session_ids(events: &[NetEvent]) -> BTreeSet<u64> {
    events
        .iter()
        .map(|event| match event {
            NetEvent::Attempt { session_id, .. }
            | NetEvent::Retry { session_id, .. }
            | NetEvent::Failure { session_id, .. }
            | NetEvent::Cooldown { session_id, .. }
            | NetEvent::TransferProgress { session_id, .. }
            | NetEvent::TransferCompleted { session_id, .. } => *session_id,
        })
        .collect()
}

#[tokio::test]
async fn ranking_rejects_non_default_values_from_config() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _download_env = unset_download_env();

    let mut config = Config::default();
    config.download.sort = SortOrder::DateAsc;
    config.save().unwrap();

    let cli = Cli::parse_from(["picals-crawler", "download", "ranking", "--dry-run"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("download ranking 不支持自定义排序"));
}

#[tokio::test]
async fn ranking_rejects_ai_false_from_env() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _download_env = unset_download_env();
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
    let _download_env = unset_download_env();
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
    let _base_url = set_base_url(&server.uri());
    let _download_env = unset_download_env();
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
async fn env_invalid_sort_returns_error() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(temp.path());
    let _download_env = unset_download_env();
    let _sort = EnvVarGuard::set("PICALS_DOWNLOAD_SORT", "popular_desc");

    let cli = Cli::parse_from(["picals-crawler", "download", "illust", "123"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("无效的排序值"));
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
            source_url: Some("https://example.com/123456_p0.png".to_string()),
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
            source_url: Some(format!(
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

    let _base_url = set_base_url(&server.uri());
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
    let _base_url = set_base_url(&server.uri());
    let _download_env = unset_download_env();
    Credential::new("cookie").unwrap().save().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": { "illusts": { "123456": {} }, "manga": {} }
        })))
        .mount(&server)
        .await;
    mount_illust_detail(&server, "123456").await;

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
    let _observer = install_observer(Arc::new(move |event| {
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
    let _base_url = set_base_url(&server.uri());
    let _download_env = unset_download_env();
    Credential::new("cookie").unwrap().save().unwrap();
    mount_illust_detail(&server, "111111").await;

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
    let _command_observer = install_observer(Arc::new(move |event| {
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
            source_url: Some(format!("{}/img-original/222222_p0.png", server.uri())),
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
    let _retry_observer = install_observer(Arc::new(move |event| {
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
            source_url: Some("https://example.invalid/123456_p0.png".to_string()),
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
