use std::sync::LazyLock;

use clap::{CommandFactory, Parser};
use picals_crawler::{
    auth::Credential,
    cli::{Cli, download::DownloadSubcommand},
    commands,
    config::{Config, DownloadConfig, POPULAR_SORT_MIGRATION_MESSAGE, ProxyConfig, SortOrder},
};
use tempfile::tempdir;
use tokio::sync::{Mutex, MutexGuard};

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

struct ConfigHomeGuard {
    _home: EnvVarGuard,
    _xdg: EnvVarGuard,
}

struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }

    fn unset(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

async fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().await
}

fn set_config_home(temp: &tempfile::TempDir) -> ConfigHomeGuard {
    let xdg = temp.path().join(".config");
    ConfigHomeGuard {
        _home: EnvVarGuard::set("HOME", temp.path()),
        _xdg: EnvVarGuard::set("XDG_CONFIG_HOME", &xdg),
    }
}

#[test]
fn ranking_help_does_not_expose_sort_r18_or_no_ai() {
    let mut command = Cli::command();
    let rendered = command.render_long_help().to_string();
    assert!(rendered.contains("download"));

    let mut ranking = Cli::command()
        .find_subcommand_mut("download")
        .unwrap()
        .find_subcommand_mut("ranking")
        .unwrap()
        .clone();
    let help = ranking.render_long_help().to_string();

    assert!(!help.contains("--sort"));
    assert!(!help.contains("--r18"));
    assert!(!help.contains("--no-ai"));
}

#[test]
fn ranking_cli_rejects_sort_flag_at_parse_time() {
    let error = Cli::try_parse_from([
        "picals-crawler",
        "download",
        "ranking",
        "--sort",
        "date_desc",
    ])
    .unwrap_err();

    assert!(error.to_string().contains("--sort"));
}

#[tokio::test]
async fn ranking_rejects_non_default_values_from_config() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(&temp);
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");

    Config {
        download: DownloadConfig {
            sort: SortOrder::DateAsc,
            ..DownloadConfig::default()
        },
        proxy: ProxyConfig::default(),
    }
    .save()
    .unwrap();

    let cli = Cli::parse_from(["picals-crawler", "download", "ranking", "--dry-run"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("download ranking 不支持自定义排序"));
}

#[tokio::test]
async fn ranking_rejects_ai_false_from_env() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(&temp);
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    let _ai = EnvVarGuard::set("PICALS_DOWNLOAD_AI", "false");

    let cli = Cli::parse_from(["picals-crawler", "download", "ranking", "--dry-run"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("download ranking 不支持 AI 过滤开关"));
}

#[tokio::test]
async fn bookmark_requires_user_id_in_credential() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(&temp);
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    Credential::new("cookie").unwrap().save().unwrap();

    let cli = Cli::parse_from(["picals-crawler", "download", "bookmark"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains("缺少 userId"));
}

#[tokio::test]
async fn bookmark_dry_run_accepts_credential_with_user_id() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(&temp);
    let _sort = EnvVarGuard::unset("PICALS_DOWNLOAD_SORT");
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    Credential::new_with_user_id("cookie", Some("12345678"))
        .unwrap()
        .save()
        .unwrap();

    let cli = Cli::parse_from(["picals-crawler", "download", "bookmark", "--dry-run"]);
    commands::dispatch(cli).await.unwrap();
}

#[tokio::test]
async fn env_popular_sort_returns_migration_error() {
    let _lock = lock_env().await;
    let temp = tempdir().unwrap();
    let _config_home = set_config_home(&temp);
    let _ai = EnvVarGuard::unset("PICALS_DOWNLOAD_AI");
    let _r18 = EnvVarGuard::unset("PICALS_DOWNLOAD_R18");
    let _sort = EnvVarGuard::set("PICALS_DOWNLOAD_SORT", "popular_desc");

    let cli = Cli::parse_from(["picals-crawler", "download", "illust", "123"]);
    let error = commands::dispatch(cli).await.unwrap_err();
    assert!(format!("{error:#}").contains(POPULAR_SORT_MIGRATION_MESSAGE));
}

#[test]
fn keyword_cli_still_uses_query_and_r18_only() {
    let cli = Cli::parse_from(["picals-crawler", "download", "keyword", "初音ミク", "--r18"]);

    match cli.command {
        picals_crawler::cli::Command::Download(download) => match download.target {
            DownloadSubcommand::Keyword(args) => {
                assert_eq!(args.query, "初音ミク");
                assert!(args.r18);
            }
            _ => panic!("expected keyword command"),
        },
        _ => panic!("expected download command"),
    }
}
