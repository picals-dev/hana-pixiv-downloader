use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn binary_help_shows_download_subcommand() {
    Command::cargo_bin("picals-crawler")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("download"));
}

#[test]
fn ranking_help_does_not_expose_sort_r18_or_no_ai() {
    let output = Command::cargo_bin("picals-crawler")
        .unwrap()
        .args(["download", "ranking", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8_lossy(&output);

    assert!(!help.contains("--sort"));
    assert!(!help.contains("--r18"));
    assert!(!help.contains("--no-ai"));
}

#[test]
fn ranking_cli_rejects_sort_flag_at_parse_time() {
    Command::cargo_bin("picals-crawler")
        .unwrap()
        .args(["download", "ranking", "--sort", "date_desc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--sort"));
}

#[test]
fn ranking_cli_rejects_legacy_mode_flag() {
    Command::cargo_bin("picals-crawler")
        .unwrap()
        .args(["download", "ranking", "--mode", "daily"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--mode"));
}
