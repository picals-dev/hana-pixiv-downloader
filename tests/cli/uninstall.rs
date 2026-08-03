use std::fs;

use assert_cmd::{Command, cargo::cargo_bin};
use predicates::prelude::*;
use wiremock::MockServer;

use crate::support::cli::CliTestContext;

#[tokio::test]
async fn uninstall_requires_explicit_confirmation() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let config_dir = ctx.xdg_config_home().join("hana-pixiv-downloader");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "[download]\n").unwrap();

    ctx.command()
        .arg("uninstall")
        .assert()
        .failure()
        .stderr(predicate::str::contains("hpd uninstall --yes"));

    assert!(config_dir.exists());
}

#[tokio::test]
async fn uninstall_purges_config_downloads_and_its_executable() {
    let server = MockServer::start().await;
    let ctx = CliTestContext::new(&server).await;
    let executable = ctx.path("hpd");
    fs::copy(cargo_bin("hpd"), &executable).unwrap();

    let downloads = ctx.path("downloads");
    let roots = [
        downloads.join("illust"),
        downloads.join("user"),
        downloads.join("bookmark"),
        downloads.join("keyword"),
        downloads.join("ranking"),
    ];
    for root in &roots {
        fs::create_dir_all(root).unwrap();
        fs::write(root.join("sample.jpg"), "image").unwrap();
    }

    let config_dir = ctx.xdg_config_home().join("hana-pixiv-downloader");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        format!(
            "[download.roots]\nillust = {:?}\nuser = {:?}\nbookmark = {:?}\nkeyword = {:?}\nranking = {:?}\n",
            roots[0], roots[1], roots[2], roots[3], roots[4]
        ),
    )
    .unwrap();
    fs::write(config_dir.join("credentials"), "phpsessid = \"cookie\"\n").unwrap();

    Command::new(&executable)
        .env("HOME", ctx.home_dir())
        .env("XDG_CONFIG_HOME", ctx.xdg_config_home())
        .args(["uninstall", "--yes", "--purge-downloads"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hpd 已卸载"));

    assert!(!config_dir.exists());
    assert!(roots.iter().all(|root| !root.exists()));
    assert!(!executable.exists());
}
