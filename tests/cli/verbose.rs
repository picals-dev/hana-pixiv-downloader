use predicates::prelude::*;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::support::{cli::CliTestContext, mock_pixiv::artwork_referer};

async fn mount_single_illust(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456"))
        .and(header("referer", artwork_referer(&server.uri(), "123456")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illustType": 0,
                "tags": { "tags": [{ "tag": "static" }] }
            }
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/ajax/illust/123456/pages"))
        .and(header("referer", artwork_referer(&server.uri(), "123456")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": [{
                "urls": { "original": format!("{}/img-original/123456_p0.png", server.uri()) }
            }]
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/img-original/123456_p0.png"))
        .and(header("referer", artwork_referer(&server.uri(), "123456")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn verbose_flag_emits_debug_request_logs() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("downloads");
    ctx.write_credential_with_user_id("12345678");
    mount_single_illust(&server).await;

    ctx.command()
        .env_remove("RUST_LOG")
        .args([
            "--verbose",
            "download",
            "illust",
            "123456",
            "--to",
            output_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("request.attempt"));
}

#[tokio::test]
async fn without_verbose_debug_request_logs_are_suppressed() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("downloads");
    ctx.write_credential_with_user_id("12345678");
    mount_single_illust(&server).await;

    ctx.command()
        .env_remove("RUST_LOG")
        .args([
            "download",
            "illust",
            "123456",
            "--to",
            output_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("request.attempt").not());
}
