use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use gif::ColorOutput;
use picals_crawler::{
    auth::Credential,
    config::{DownloadConfig, DownloadMode, ResolvedDownloadOptions, SortOrder},
    crawler::{
        bookmark::BookmarkCrawler,
        illust::IllustCrawler,
        keyword::{KeywordCrawler, KeywordMode},
        ranking::RankingCrawler,
        user::UserCrawler,
    },
    failure::{FailureRecord, FailureStage, ReplayCommand, ReplayOptions},
    net::{NetEvent, PixivNetSession, RequestKind},
    replay::replay_failures_with_session,
};
use tempfile::tempdir;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::support::{
    fixtures::read_binary_fixture,
    mock_pixiv::{artwork_referer, user_illustrations_referer},
};

const USER_ID: &str = "12345678";
const STATIC_ID: &str = "123456";
const UGOIRA_ID: &str = "654321";
const KEYWORD: &str = "miku";
const RANKING_MODE: &str = "daily";

fn options(directory: PathBuf) -> ResolvedDownloadOptions {
    let defaults = DownloadConfig::default();
    ResolvedDownloadOptions {
        mode: DownloadMode::Illust,
        directory,
        count: defaults.count,
        sort: SortOrder::DateDesc,
        r18: defaults.r18,
        ai: defaults.ai,
        concurrent: 4,
        timeout: 5,
        retry: 2,
        with_tags: false,
        proxy_url: None,
        dry_run: false,
    }
}

fn options_with_tags(directory: PathBuf) -> ResolvedDownloadOptions {
    let mut options = options(directory);
    options.with_tags = true;
    options
}

fn session(
    options: ResolvedDownloadOptions,
    credential: Credential,
    base_url: Url,
) -> Arc<PixivNetSession> {
    Arc::new(PixivNetSession::new_with_base_url(options, credential, base_url).unwrap())
}

fn image_url(server: &MockServer, illust_id: &str) -> String {
    format!("{}/img-original/{illust_id}_p0.png", server.uri())
}

fn ugoira_zip_url(server: &MockServer) -> String {
    format!("{}/img-zip-ugoira/{UGOIRA_ID}.zip", server.uri())
}

async fn mount_static_artwork(server: &MockServer, illust_id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/illust/{illust_id}")))
        .and(header("referer", artwork_referer(&server.uri(), illust_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illustType": 0,
                "tags": {
                    "tags": [
                        { "tag": "静态图", "translation": { "en": "static" } }
                    ]
                }
            }
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/ajax/illust/{illust_id}/pages")))
        .and(header("referer", artwork_referer(&server.uri(), illust_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": [{
                "urls": {
                    "original": image_url(server, illust_id)
                }
            }]
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/img-original/{illust_id}_p0.png")))
        .and(header("referer", artwork_referer(&server.uri(), illust_id)))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"png-ok".to_vec()))
        .mount(server)
        .await;
}

async fn mount_ugoira_artwork(server: &MockServer, zip_bytes: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/illust/{UGOIRA_ID}")))
        .and(header("referer", artwork_referer(&server.uri(), UGOIRA_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illustType": 2,
                "tags": {
                    "tags": [
                        { "tag": "うごイラ", "translation": { "en": "ugoira" } }
                    ]
                }
            }
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/ajax/illust/{UGOIRA_ID}/ugoira_meta")))
        .and(header("referer", artwork_referer(&server.uri(), UGOIRA_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "originalSrc": ugoira_zip_url(server),
                "mime_type": "image/png",
                "frames": [
                    { "file": "000000.png", "delay": 60 },
                    { "file": "000001.png", "delay": 120 }
                ]
            }
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/img-zip-ugoira/{UGOIRA_ID}.zip")))
        .and(header("referer", artwork_referer(&server.uri(), UGOIRA_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(zip_bytes))
        .mount(server)
        .await;
}

async fn mount_user_profile(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/user/{USER_ID}/profile/all")))
        .and(header(
            "referer",
            user_illustrations_referer(&server.uri(), USER_ID),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illusts": {
                    (STATIC_ID): {},
                    (UGOIRA_ID): {}
                },
                "manga": {}
            }
        })))
        .mount(server)
        .await;
}

async fn mount_keyword_page(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/search/artworks/{KEYWORD}")))
        .and(header(
            "referer",
            format!("{}/tags/{KEYWORD}/artworks", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illustManga": {
                    "data": [{ "id": STATIC_ID }, { "id": UGOIRA_ID }]
                }
            }
        })))
        .mount(server)
        .await;
}

async fn mount_ranking_page(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/ranking.php"))
        .and(header("referer", format!("{}/ranking.php", server.uri())))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "contents": [
                { "illust_id": STATIC_ID },
                { "illust_id": UGOIRA_ID }
            ],
            "rank_total": 2
        })))
        .mount(server)
        .await;
}

async fn mount_bookmark_page(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/user/{USER_ID}/illusts/bookmarks")))
        .and(header(
            "referer",
            format!("{}/bookmark.php?type=user", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "total": 2,
                "works": [
                    { "id": STATIC_ID },
                    { "id": UGOIRA_ID }
                ]
            }
        })))
        .mount(server)
        .await;
}

async fn mount_mixed_artwork_assets(server: &MockServer) {
    mount_static_artwork(server, STATIC_ID).await;
    mount_ugoira_artwork(server, read_binary_fixture("ugoira.zip")).await;
}

fn assert_gif_file(path: &Path) {
    let file = std::fs::File::open(path).unwrap();
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(ColorOutput::RGBA);
    let mut reader = options.read_info(file).unwrap();
    let mut frames = 0usize;
    while reader.read_next_frame().unwrap().is_some() {
        frames += 1;
    }

    assert_eq!(frames, 2);
}

#[tokio::test]
async fn illust_crawler_downloads_ugoira_gif_and_cleans_workspace() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_ugoira_artwork(&server, read_binary_fixture("ugoira.zip")).await;

    let crawler = IllustCrawler::new_with_session(
        UGOIRA_ID.to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    let gif_path = temp.path().join(UGOIRA_ID).join(format!("{UGOIRA_ID}.gif"));

    assert_eq!(result.total, 1);
    assert_eq!(result.downloaded, 1);
    assert_eq!(result.failed, 0);
    assert!(gif_path.exists());
    assert_gif_file(&gif_path);
    assert!(
        !temp
            .path()
            .join(UGOIRA_ID)
            .join(".picals-workspace")
            .exists()
    );
}

#[tokio::test]
async fn illust_crawler_records_convert_failure_and_cleans_workspace() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_ugoira_artwork(&server, b"not-a-zip".to_vec()).await;

    let crawler = IllustCrawler::new_with_session(
        UGOIRA_ID.to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.failed, 1);
    assert_eq!(result.failure_records[0].stage, FailureStage::Convert);
    assert_eq!(
        result.failure_records[0].source_url.as_deref(),
        Some(ugoira_zip_url(&server).as_str())
    );
    assert!(
        !temp
            .path()
            .join(UGOIRA_ID)
            .join(".picals-workspace")
            .exists()
    );
    assert!(
        !temp
            .path()
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
}

#[tokio::test]
async fn user_crawler_mixed_batch_reuses_detail_for_tags_export() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_user_profile(&server).await;
    mount_mixed_artwork_assets(&server).await;

    let events = Arc::new(Mutex::new(Vec::<NetEvent>::new()));
    let observer_events = Arc::clone(&events);
    let session = Arc::new(
        PixivNetSession::builder(
            options_with_tags(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        )
        .with_observer(Arc::new(move |event| {
            observer_events.lock().unwrap().push(event);
        }))
        .build()
        .unwrap(),
    );

    let crawler = UserCrawler::new_with_session(
        USER_ID.to_string(),
        options_with_tags(temp.path().to_path_buf()),
        session,
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.downloaded, 2);
    assert!(
        temp.path()
            .join(USER_ID)
            .join(STATIC_ID)
            .join(format!("{STATIC_ID}_p0.png"))
            .exists()
    );
    assert!(
        temp.path()
            .join(USER_ID)
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
    assert!(temp.path().join(USER_ID).join("tags.json").exists());

    let detail_attempts = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            NetEvent::Attempt { kind, url, .. } if *kind == RequestKind::IllustDetail => {
                Some(url.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        detail_attempts
            .iter()
            .filter(|url| url.contains(&format!("/ajax/illust/{STATIC_ID}")))
            .count(),
        1
    );
    assert_eq!(
        detail_attempts
            .iter()
            .filter(|url| url.contains(&format!("/ajax/illust/{UGOIRA_ID}")))
            .count(),
        1
    );
}

#[tokio::test]
async fn keyword_crawler_supports_mixed_batch_with_ugoira() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_keyword_page(&server).await;
    mount_mixed_artwork_assets(&server).await;

    let crawler = KeywordCrawler::new_with_session(
        KEYWORD.to_string(),
        KeywordMode::Safe,
        ResolvedDownloadOptions {
            count: 2,
            ..options(temp.path().to_path_buf())
        },
        session(
            ResolvedDownloadOptions {
                count: 2,
                ..options(temp.path().to_path_buf())
            },
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 2);
    assert!(
        temp.path()
            .join(KEYWORD)
            .join(STATIC_ID)
            .join(format!("{STATIC_ID}_p0.png"))
            .exists()
    );
    assert!(
        temp.path()
            .join(KEYWORD)
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
}

#[tokio::test]
async fn ranking_crawler_supports_mixed_batch_with_ugoira() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_ranking_page(&server).await;
    mount_mixed_artwork_assets(&server).await;

    let crawler = RankingCrawler::new_with_session(
        RANKING_MODE.to_string(),
        ResolvedDownloadOptions {
            count: 2,
            ..options(temp.path().to_path_buf())
        },
        session(
            ResolvedDownloadOptions {
                count: 2,
                ..options(temp.path().to_path_buf())
            },
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 2);
    assert!(
        temp.path()
            .join(RANKING_MODE)
            .join(STATIC_ID)
            .join(format!("{STATIC_ID}_p0.png"))
            .exists()
    );
    assert!(
        temp.path()
            .join(RANKING_MODE)
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
}

#[tokio::test]
async fn bookmark_crawler_supports_mixed_batch_with_ugoira() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_bookmark_page(&server).await;
    mount_mixed_artwork_assets(&server).await;

    let crawler = BookmarkCrawler::new_with_session(
        USER_ID.to_string(),
        ResolvedDownloadOptions {
            count: 2,
            ..options(temp.path().to_path_buf())
        },
        session(
            ResolvedDownloadOptions {
                count: 2,
                ..options(temp.path().to_path_buf())
            },
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 2);
    assert!(
        temp.path()
            .join(USER_ID)
            .join(STATIC_ID)
            .join(format!("{STATIC_ID}_p0.png"))
            .exists()
    );
    assert!(
        temp.path()
            .join(USER_ID)
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
}

#[tokio::test]
async fn replay_can_recover_convert_failure_record() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_ugoira_artwork(&server, read_binary_fixture("ugoira.zip")).await;

    let options = options(temp.path().to_path_buf());
    let replay_command = ReplayCommand::Illust {
        illust_id: UGOIRA_ID.to_string(),
        options: ReplayOptions::from(&options),
    };
    let report = replay_failures_with_session(
        session(
            options.clone(),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
        &replay_command,
        vec![FailureRecord {
            mode: DownloadMode::Illust,
            stage: FailureStage::Convert,
            illust_id: Some(UGOIRA_ID.to_string()),
            source_url: Some(ugoira_zip_url(&server)),
            target_path: Some(
                temp.path()
                    .join(UGOIRA_ID)
                    .join(format!("{UGOIRA_ID}.gif"))
                    .to_string_lossy()
                    .into_owned(),
            ),
            error_kind: "convert".to_string(),
            error_message: "broken".to_string(),
            retryable: true,
        }],
    )
    .await
    .unwrap();

    assert_eq!(report.recovered, 1);
    assert_eq!(report.remaining_records.len(), 0);
    assert_gif_file(&temp.path().join(UGOIRA_ID).join(format!("{UGOIRA_ID}.gif")));
}
