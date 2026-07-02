use std::fs;

use wiremock::MockServer;

use crate::support::cli::CliTestContext;

#[tokio::test]
async fn organize_dry_run_has_no_side_effects() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;

    let config_dir = ctx.xdg_config_home().join("hana-pixiv-downloader");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!(
            r#"[download]
batch_layout = "flat"
count = 0
sort = "date_desc"
r18 = false
ai = true
concurrent = 8
timeout = 30
retry = 3
with_tags = false

[download.roots]
illust = "{0}"
user = "{1}"
bookmark = "{2}"
keyword = "{3}"
ranking = "{4}"

[proxy]
url = ""
"#,
            ctx.path("downloads/illust").display(),
            ctx.path("downloads/user").display(),
            ctx.path("downloads/bookmark").display(),
            ctx.path("downloads/keyword").display(),
            ctx.path("downloads/ranking").display(),
        ),
    )
    .unwrap();

    let context_dir = ctx.path("downloads/user/12345678");
    let illust_dir = context_dir.join("123456");
    fs::create_dir_all(&illust_dir).unwrap();
    fs::write(illust_dir.join("123456_p0.png"), b"ok").unwrap();

    ctx.command()
        .args(["organize", "--dry-run"])
        .assert()
        .success();

    assert!(illust_dir.join("123456_p0.png").exists());
    assert!(!context_dir.join("123456_p0.png").exists());
}
