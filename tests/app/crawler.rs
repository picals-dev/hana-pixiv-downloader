use std::{fs, path::PathBuf, sync::Arc};

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
    error::CrawlerError,
    net::PixivNetSession,
};
use tempfile::tempdir;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::support::fixtures::read_fixture;

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

async fn mount_illust_detail(server: &MockServer, illust_id: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/illust/{illust_id}")))
        .and(header(
            "referer",
            format!("{}/artworks/{illust_id}", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(read_fixture("illust_detail.json")))
        .mount(server)
        .await;
}

async fn mount_illust_details(server: &MockServer, illust_ids: &[&str]) {
    for illust_id in illust_ids {
        mount_illust_detail(server, illust_id).await;
    }
}

#[tokio::test]
async fn user_crawler_can_download_images_with_mock_server() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .and(header(
            "referer",
            format!("{}/users/12345678/illustrations", server.uri()),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(read_fixture("user_profile_all.json")),
        )
        .mount(&server)
        .await;
    mount_illust_details(&server, &["123456", "123457", "223456"]).await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "message": "",
            "body": [
                {
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p0.png", server.uri())
                    }
                },
                {
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p1.png", server.uri())
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123457/pages"))
        .and(header(
            "referer",
            format!("{}/artworks/123457", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "message": "",
            "body": []
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/223456/pages"))
        .and(header(
            "referer",
            format!("{}/artworks/223456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "message": "",
            "body": []
        })))
        .mount(&server)
        .await;

    let image_bytes = b"fake-png-data";
    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(image_bytes.to_vec()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p1.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(image_bytes.to_vec()))
        .mount(&server)
        .await;

    let crawler = UserCrawler::new_with_session(
        "12345678".to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.total, 2);
    assert_eq!(result.downloaded, 2);
    assert_eq!(result.skipped, 0);
    assert_eq!(result.failed, 0);
    assert!(temp.path().join("12345678/123456/123456_p0.png").exists());
    assert!(temp.path().join("12345678/123456/123456_p1.png").exists());
}

#[tokio::test]
async fn illust_crawler_can_download_single_work() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    mount_illust_detail(&server, "123456").await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .and(header("referer", format!("{}/artworks/123456", server.uri())))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": [
                {
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p0.png", server.uri())
                    }
                },
                {
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p1.png", server.uri())
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"p0".to_vec()))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p1.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"p1".to_vec()))
        .mount(&server)
        .await;

    let crawler = IllustCrawler::new_with_session(
        "123456".to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.total, 2);
    assert_eq!(result.downloaded, 2);
    assert_eq!(result.failed, 0);
    assert!(temp.path().join("123456/123456_p0.png").exists());
    assert!(temp.path().join("123456/123456_p1.png").exists());
}

#[tokio::test]
async fn user_crawler_exports_tags_json_when_enabled() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

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
                    "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p0.png", server.uri())
                }
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456"))
        .respond_with(ResponseTemplate::new(200).set_body_json(read_fixture("illust_detail.json")))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p0.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    let crawler = UserCrawler::new_with_session(
        "12345678".to_string(),
        options_with_tags(temp.path().to_path_buf()),
        session(
            options_with_tags(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 1);

    let tags_path = temp.path().join("12345678/tags.json");
    assert!(tags_path.exists());
    let tags: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(tags_path).unwrap()).unwrap();
    assert_eq!(
        tags["123456"],
        serde_json::json!(["Hatsune Miku", "オリジナル"])
    );
}

#[tokio::test]
async fn user_crawler_skips_existing_file() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    let output_dir = temp.path().join("12345678/123456");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(output_dir.join("123456_p0.png"), b"existing").unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .and(header(
            "referer",
            format!("{}/users/12345678/illustrations", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "message": "",
            "body": { "illusts": { "123456": {} }, "manga": {} }
        })))
        .mount(&server)
        .await;
    mount_illust_detail(&server, "123456").await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "message": "",
            "body": [{
                "urls": {
                    "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p0.png", server.uri())
                }
            }]
        })))
        .mount(&server)
        .await;

    let crawler = UserCrawler::new_with_session(
        "12345678".to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.total, 1);
    assert_eq!(result.downloaded, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.failed, 0);
}

#[tokio::test]
async fn downloader_recovers_from_stale_part_file() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    let output_dir = temp.path().join("123456");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(output_dir.join("123456_p0.png.part"), b"stale").unwrap();
    mount_illust_detail(&server, "123456").await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": [{
                "urls": {
                    "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p0.png", server.uri())
                }
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p0.png"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fresh".to_vec()))
        .mount(&server)
        .await;

    let crawler = IllustCrawler::new_with_session(
        "123456".to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.downloaded, 1);
    assert_eq!(
        fs::read(output_dir.join("123456_p0.png")).unwrap(),
        b"fresh"
    );
    assert!(!output_dir.join("123456_p0.png.part").exists());
}

#[tokio::test]
async fn keyword_crawler_can_download_search_results() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path(
            "/ajax/search/artworks/%E5%88%9D%E9%9F%B3%E3%83%9F%E3%82%AF",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(read_fixture("keyword_search.json")))
        .mount(&server)
        .await;
    mount_illust_details(&server, &["146185119", "146185709"]).await;

    for illust_id in ["146185119", "146185709"] {
        Mock::given(method("GET"))
            .and(path(format!("/ajax/illust/{illust_id}/pages")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": false,
                "body": [{
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/{}_p0.png", server.uri(), illust_id)
                    }
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/img-original/img/2024/01/02/03/04/05/{}_p0.png",
                illust_id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;
    }

    let mut keyword_options = options(temp.path().to_path_buf());
    keyword_options.count = 2;
    let crawler = KeywordCrawler::new_with_session(
        "初音ミク".to_string(),
        KeywordMode::Safe,
        keyword_options.clone(),
        session(
            keyword_options,
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 2);
}

#[tokio::test]
async fn ranking_crawler_can_download_ranked_results() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path("/ranking.php"))
        .respond_with(ResponseTemplate::new(200).set_body_json(read_fixture("ranking.json")))
        .mount(&server)
        .await;
    mount_illust_details(&server, &["146109718", "146135045"]).await;

    for illust_id in ["146109718", "146135045"] {
        Mock::given(method("GET"))
            .and(path(format!("/ajax/illust/{illust_id}/pages")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": false,
                "body": [{
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/{}_p0.png", server.uri(), illust_id)
                    }
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/img-original/img/2024/01/02/03/04/05/{}_p0.png",
                illust_id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;
    }

    let mut ranking_options = options(temp.path().to_path_buf());
    ranking_options.count = 2;
    let crawler = RankingCrawler::new_with_session(
        "daily".to_string(),
        ranking_options.clone(),
        session(
            ranking_options,
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 2);
}

#[tokio::test]
async fn bookmark_crawler_can_download_bookmarks() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/illusts/bookmarks"))
        .and(header(
            "referer",
            format!("{}/bookmark.php?type=user", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(read_fixture("bookmark.json")))
        .mount(&server)
        .await;
    mount_illust_details(&server, &["146185119", "146185709"]).await;

    for illust_id in ["146185119", "146185709"] {
        Mock::given(method("GET"))
            .and(path(format!("/ajax/illust/{illust_id}/pages")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": false,
                "body": [{
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/{}_p0.png", server.uri(), illust_id)
                    }
                }]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/img-original/img/2024/01/02/03/04/05/{}_p0.png",
                illust_id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;
    }

    let crawler = BookmarkCrawler::new_with_session(
        "12345678".to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new_with_user_id("cookie", Some("12345678")).unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 2);
    assert!(
        temp.path()
            .join("12345678/146185119/146185119_p0.png")
            .exists()
    );
    assert!(
        temp.path()
            .join("12345678/146185709/146185709_p0.png")
            .exists()
    );
}

#[tokio::test]
async fn bookmark_crawler_truncates_to_requested_count() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/illusts/bookmarks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(read_fixture("bookmark.json")))
        .mount(&server)
        .await;
    mount_illust_detail(&server, "146185709").await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/146185709/pages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": [{
                "urls": {
                    "original": format!("{}/img-original/img/2024/01/02/03/04/05/146185709_p0.png", server.uri())
                }
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/img-original/img/2024/01/02/03/04/05/146185709_p0.png",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    let mut bookmark_options = options(temp.path().to_path_buf());
    bookmark_options.count = 1;
    let crawler = BookmarkCrawler::new_with_session(
        "12345678".to_string(),
        bookmark_options.clone(),
        session(
            bookmark_options,
            Credential::new_with_user_id("cookie", Some("12345678")).unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.downloaded, 1);
    assert!(
        temp.path()
            .join("12345678/146185709/146185709_p0.png")
            .exists()
    );
    assert!(
        !temp
            .path()
            .join("12345678/146185119/146185119_p0.png")
            .exists()
    );
}

#[tokio::test]
async fn bookmark_crawler_exports_tags_json_when_enabled() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/illusts/bookmarks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(read_fixture("bookmark.json")))
        .mount(&server)
        .await;

    for illust_id in ["146185119", "146185709"] {
        Mock::given(method("GET"))
            .and(path(format!("/ajax/illust/{illust_id}/pages")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": false,
                "body": [{
                    "urls": {
                        "original": format!("{}/img-original/img/2024/01/02/03/04/05/{}_p0.png", server.uri(), illust_id)
                    }
                }]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(format!("/ajax/illust/{illust_id}")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(read_fixture("illust_detail.json")),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(format!(
                "/img-original/img/2024/01/02/03/04/05/{}_p0.png",
                illust_id
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;
    }

    let crawler = BookmarkCrawler::new_with_session(
        "12345678".to_string(),
        options_with_tags(temp.path().to_path_buf()),
        session(
            options_with_tags(temp.path().to_path_buf()),
            Credential::new_with_user_id("cookie", Some("12345678")).unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();
    assert_eq!(result.downloaded, 2);

    let tags_path = temp.path().join("12345678/tags.json");
    assert!(tags_path.exists());
    let tags: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(tags_path).unwrap()).unwrap();
    assert_eq!(
        tags["146185119"],
        serde_json::json!(["Hatsune Miku", "オリジナル"])
    );
}

#[tokio::test]
async fn user_crawler_counts_partial_failures_without_failing_command() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .and(header(
            "referer",
            format!("{}/users/12345678/illustrations", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "message": "",
            "body": { "illusts": { "123456": {}, "123457": {} }, "manga": {} }
        })))
        .mount(&server)
        .await;
    mount_illust_details(&server, &["123456", "123457"]).await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "message": "",
            "body": [{
                "urls": {
                    "original": format!("{}/img-original/img/2024/01/02/03/04/05/123456_p0.png", server.uri())
                }
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123457/pages"))
        .and(header(
            "referer",
            format!("{}/artworks/123457", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/img/2024/01/02/03/04/05/123456_p0.png"))
        .and(header(
            "referer",
            format!("{}/artworks/123456", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    let crawler = UserCrawler::new_with_session(
        "12345678".to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.downloaded, 1);
    assert_eq!(result.failed, 1);
    assert_eq!(result.skipped, 0);
}

#[tokio::test]
async fn user_crawler_fails_on_profile_request_error() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .and(header(
            "referer",
            format!("{}/users/12345678/illustrations", server.uri()),
        ))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let crawler = UserCrawler::new_with_session(
        "12345678".to_string(),
        options(temp.path().to_path_buf()),
        session(
            options(temp.path().to_path_buf()),
            Credential::new("cookie").unwrap(),
            base_url,
        ),
    );

    let error = crawler.run().await.unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("401"), "{message}");
}

#[test]
fn missing_credential_error_keeps_chinese_message() {
    let error = CrawlerError::MissingCredential;
    assert!(error.to_string().contains("请先运行 picals-crawler setup"));
}

#[test]
fn missing_user_id_error_keeps_chinese_message() {
    let error = CrawlerError::MissingUserId;
    assert!(error.to_string().contains("缺少 userId"));
}
