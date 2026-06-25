use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::support::{
    cli::CliTestContext,
    fixtures::read_binary_fixture,
    mock_pixiv::{artwork_referer, user_illustrations_referer},
};

const USER_ID: &str = "12345678";
const STATIC_ID: &str = "123456";
const UGOIRA_ID: &str = "654321";

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
                    "tags": [{ "tag": "static" }]
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

async fn mount_ugoira_artwork(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/ajax/illust/{UGOIRA_ID}")))
        .and(header("referer", artwork_referer(&server.uri(), UGOIRA_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": false,
            "body": {
                "illustType": 2,
                "tags": {
                    "tags": [{ "tag": "ugoira" }]
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
        .respond_with(ResponseTemplate::new(200).set_body_bytes(read_binary_fixture("ugoira.zip")))
        .mount(server)
        .await;
}

#[tokio::test]
async fn illust_ugoira_dry_run_does_not_leak_source_details() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("downloads");

    let assert = ctx
        .command()
        .args([
            "download",
            "illust",
            UGOIRA_ID,
            "--dry-run",
            "--to",
            output_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(!stdout.contains("originalSrc"));
    assert!(!stdout.contains(".zip"));
    assert!(!stdout.contains("ugoira_source"));
    assert!(
        !output_root
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
}

#[tokio::test]
async fn illust_ugoira_download_writes_final_gif() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("downloads");
    ctx.write_cookie_only_credential();
    mount_ugoira_artwork(&server).await;

    ctx.command()
        .args([
            "download",
            "illust",
            UGOIRA_ID,
            "--to",
            output_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(
        output_root
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
    assert!(
        !output_root
            .join(UGOIRA_ID)
            .join(".hpd-workspace")
            .exists()
    );
}

#[tokio::test]
async fn user_download_handles_mixed_static_and_ugoira_outputs() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("downloads");
    ctx.write_credential_with_user_id(USER_ID);

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
        .mount(&server)
        .await;

    mount_static_artwork(&server, STATIC_ID).await;
    mount_ugoira_artwork(&server).await;

    ctx.command()
        .args([
            "download",
            "user",
            USER_ID,
            "--count",
            "2",
            "--to",
            output_root.to_string_lossy().as_ref(),
        ])
        .assert()
        .success();

    assert!(
        output_root
            .join(USER_ID)
            .join(STATIC_ID)
            .join(format!("{STATIC_ID}_p0.png"))
            .exists()
    );
    assert!(
        output_root
            .join(USER_ID)
            .join(UGOIRA_ID)
            .join(format!("{UGOIRA_ID}.gif"))
            .exists()
    );
}
