#[path = "support/mod.rs"]
mod common;

use std::{fs, path::PathBuf};

use picals_crawler::{
    auth::Credential,
    config::{DownloadConfig, ResolvedDownloadOptions, SortOrder},
    crawler::user::UserCrawler,
    error::CrawlerError,
};
use tempfile::tempdir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

fn options(directory: PathBuf) -> ResolvedDownloadOptions {
    let defaults = DownloadConfig::default();
    ResolvedDownloadOptions {
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
            ResponseTemplate::new(200).set_body_json(common::read_fixture("user_profile_all.json")),
        )
        .mount(&server)
        .await;

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

    let crawler = UserCrawler::new_with_base_url(
        "12345678".to_string(),
        Credential::new("cookie").unwrap(),
        options(temp.path().to_path_buf()),
        base_url,
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.total, 2);
    assert_eq!(result.downloaded, 2);
    assert_eq!(result.skipped, 0);
    assert_eq!(result.failed, 0);
    assert!(temp.path().join("12345678/123456_p0.png").exists());
    assert!(temp.path().join("12345678/123456_p1.png").exists());
}

#[tokio::test]
async fn user_crawler_skips_existing_file() {
    let server = MockServer::start().await;
    let temp = tempdir().unwrap();
    let base_url = server.uri().parse().unwrap();
    let output_dir = temp.path().join("12345678");
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

    let crawler = UserCrawler::new_with_base_url(
        "12345678".to_string(),
        Credential::new("cookie").unwrap(),
        options(temp.path().to_path_buf()),
        base_url,
    );

    let result = crawler.run().await.unwrap();

    assert_eq!(result.total, 1);
    assert_eq!(result.downloaded, 0);
    assert_eq!(result.skipped, 1);
    assert_eq!(result.failed, 0);
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

    let crawler = UserCrawler::new_with_base_url(
        "12345678".to_string(),
        Credential::new("cookie").unwrap(),
        options(temp.path().to_path_buf()),
        base_url,
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

    let crawler = UserCrawler::new_with_base_url(
        "12345678".to_string(),
        Credential::new("cookie").unwrap(),
        options(temp.path().to_path_buf()),
        base_url,
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
