use picals_crawler::{
    config::{DownloadMode, SortOrder},
    failure::{FailureManifest, FailureRecord, FailureStage, ReplayCommand, ReplayOptions},
};

#[test]
fn failure_manifest_roundtrip_preserves_public_contract() {
    let manifest = FailureManifest::new(
        ReplayCommand::Illust {
            illust_id: "123456".to_string(),
            options: ReplayOptions {
                directory: "/tmp/picals".to_string(),
                count: 2,
                sort: SortOrder::DateDesc,
                r18: false,
                ai: true,
                concurrent: 4,
                timeout: 30,
                retry: 2,
                with_tags: true,
                proxy_url: Some("socks5://127.0.0.1:1080".to_string()),
                dry_run: false,
            },
        },
        vec![FailureRecord {
            mode: DownloadMode::Illust,
            stage: FailureStage::Download,
            illust_id: Some("123456".to_string()),
            source_url: Some("https://example.com/123456_p0.png".to_string()),
            target_path: Some("/tmp/picals/123456/123456_p0.png".to_string()),
            error_kind: "timeout".to_string(),
            error_message: "timeout".to_string(),
            retryable: true,
        }],
    )
    .unwrap();

    let encoded = serde_json::to_vec_pretty(&manifest).unwrap();
    let decoded: FailureManifest = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded.command, manifest.command);
    assert_eq!(decoded.records, manifest.records);
}

#[test]
fn legacy_image_url_field_is_still_accepted() {
    let legacy_manifest = serde_json::json!({
        "created_at": "20260624T000000Z",
        "command": {
            "kind": "illust",
            "illust_id": "123456",
            "options": {
                "directory": "/tmp/picals",
                "count": 1,
                "sort": "date_desc",
                "r18": false,
                "ai": true,
                "concurrent": 1,
                "timeout": 30,
                "retry": 2,
                "with_tags": false,
                "proxy_url": null,
                "dry_run": false
            }
        },
        "records": [{
            "mode": "illust",
            "stage": "download",
            "illust_id": "123456",
            "image_url": "https://example.com/123456_p0.png",
            "target_path": "/tmp/picals/123456/123456_p0.png",
            "error_kind": "timeout",
            "error_message": "timeout",
            "retryable": true
        }]
    });

    let manifest: FailureManifest = serde_json::from_value(legacy_manifest).unwrap();
    assert_eq!(
        manifest.records[0].source_url.as_deref(),
        Some("https://example.com/123456_p0.png")
    );
}
