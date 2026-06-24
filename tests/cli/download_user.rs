use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::support::{
    cli::CliTestContext,
    mock_pixiv::{artwork_referer, user_illustrations_referer},
};

#[tokio::test]
async fn user_download_dry_run_succeeds_in_black_box_mode() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("downloads");
    ctx.write_credential_with_user_id("12345678");

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .and(header(
            "referer",
            user_illustrations_referer(&server.uri(), "12345678"),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": { "illusts": { "123456": {} }, "manga": {} }
        })))
        .mount(&server)
        .await;

    ctx.command()
        .args([
            "download",
            "user",
            "12345678",
            "--dry-run",
            "--to",
            output_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(!output_root.join("12345678/123456/123456_p0.png").exists());
}

#[tokio::test]
async fn user_download_writes_files_in_black_box_mode() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("downloads");
    ctx.write_credential_with_user_id("12345678");

    Mock::given(method("GET"))
        .and(path("/ajax/user/12345678/profile/all"))
        .and(header(
            "referer",
            user_illustrations_referer(&server.uri(), "12345678"),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": { "illusts": { "123456": {} }, "manga": {} }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456"))
        .and(header("referer", artwork_referer(&server.uri(), "123456")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illustType": 0,
                "tags": {
                    "tags": [{ "tag": "static" }]
                }
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .and(header("referer", artwork_referer(&server.uri(), "123456")))
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
        .and(header("referer", artwork_referer(&server.uri(), "123456")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    ctx.command()
        .args([
            "download",
            "user",
            "12345678",
            "--count",
            "1",
            "--to",
            output_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(output_root.join("12345678/123456/123456_p0.png").exists());
}
