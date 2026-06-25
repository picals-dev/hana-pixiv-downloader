use hana_pixiv_downloader::{
    config::DownloadMode,
    downloader::ugoira::quantize_delay_centiseconds,
    failure::{FailureManifest, FailureRecord, FailureStage, ReplayCommand, ReplayOptions},
    pixiv::selector::{IllustType, select_illust_type, select_ugoira_metadata},
};
use zip::ZipArchive;

use crate::support::fixtures::{read_binary_fixture, read_fixture};

#[test]
fn ugoira_detail_fixture_selects_ugoira_type() {
    let value = read_fixture("ugoira_detail.json");
    assert_eq!(select_illust_type(&value).unwrap(), IllustType::Ugoira);
}

#[test]
fn ugoira_meta_fixture_selects_zip_and_frames() {
    let value = read_fixture("ugoira_meta.json");
    let metadata = select_ugoira_metadata(&value).unwrap();

    assert_eq!(
        metadata.original_src,
        "https://i.pximg.net/img-zip-ugoira/img/2024/01/02/03/04/05/654321_ugoira1920x1080.zip"
    );
    assert_eq!(metadata.frames.len(), 2);
    assert_eq!(metadata.frames[0].file, "000000.png");
    assert_eq!(metadata.frames[1].delay_ms, 120);
}

#[test]
fn ugoira_zip_fixture_contains_expected_frames() {
    let bytes = read_binary_fixture("ugoira.zip");
    let mut archive = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();

    assert!(archive.by_name("000000.png").is_ok());
    assert!(archive.by_name("000001.png").is_ok());
}

#[test]
fn ugoira_manifest_roundtrip_preserves_convert_and_source_url() {
    let manifest = FailureManifest::new(
        ReplayCommand::Illust {
            illust_id: "654321".to_string(),
            options: ReplayOptions {
                directory: "/tmp/hpd".to_string(),
                count: 1,
                sort: hana_pixiv_downloader::config::SortOrder::DateDesc,
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
            stage: FailureStage::Convert,
            illust_id: Some("654321".to_string()),
            source_url: Some("https://i.pximg.net/example.zip".to_string()),
            target_path: Some("/tmp/hpd/654321/654321.gif".to_string()),
            error_kind: "convert".to_string(),
            error_message: "broken".to_string(),
            retryable: true,
        }],
    )
    .unwrap();

    let encoded = serde_json::to_vec_pretty(&manifest).unwrap();
    let decoded: FailureManifest = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.records[0].stage, FailureStage::Convert);
    assert_eq!(
        decoded.records[0].source_url.as_deref(),
        Some("https://i.pximg.net/example.zip")
    );
}

#[test]
fn quantized_delay_uses_centisecond_steps() {
    assert_eq!(quantize_delay_centiseconds(60), 6);
    assert_eq!(quantize_delay_centiseconds(120), 12);
}
