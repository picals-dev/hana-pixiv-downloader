use picals_crawler::{
    config::{DownloadMode, SortOrder},
    failure::{FailureManifest, FailureRecord, FailureStage, ReplayCommand, ReplayOptions},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::support::{cli::CliTestContext, mock_pixiv::artwork_referer};

#[tokio::test]
async fn retry_command_replays_manifest_in_black_box_mode() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let output_root = ctx.path("retry-root");
    let manifest_path = ctx.path("retry.json");
    ctx.write_cookie_only_credential();

    Mock::given(method("GET"))
        .and(path("/img-original/123456_p0.png"))
        .and(header("referer", artwork_referer(&server.uri(), "123456")))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .mount(&server)
        .await;

    let manifest = FailureManifest::new(
        ReplayCommand::Illust {
            illust_id: "123456".to_string(),
            options: ReplayOptions {
                directory: output_root.to_string_lossy().into_owned(),
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
            mode: DownloadMode::Illust,
            stage: FailureStage::Download,
            illust_id: Some("123456".to_string()),
            source_url: Some(format!("{}/img-original/123456_p0.png", server.uri())),
            target_path: Some(
                output_root
                    .join("123456/123456_p0.png")
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

    ctx.command()
        .args(["retry", manifest_path.to_string_lossy().as_ref()])
        .assert()
        .success();

    assert!(output_root.join("123456/123456_p0.png").exists());
}
