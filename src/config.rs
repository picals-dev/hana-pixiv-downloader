//! 全局配置加载与合并。

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use eyre::{Context, eyre};
use serde::{Deserialize, Serialize};

use crate::error::{AppResult, CrawlerError};

const CONFIG_DIR_NAME: &str = "picals-crawler";
const CONFIG_FILE_NAME: &str = "config.toml";
const CREDENTIAL_FILE_NAME: &str = "credentials";
const DEFAULT_DOWNLOAD_DIRECTORY: &str = "~/Pictures/Pixiv";
pub const POPULAR_SORT_MIGRATION_MESSAGE: &str =
    "popular_desc 已不再支持，请改用 date_desc 或 date_asc";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    #[default]
    DateDesc,
    DateAsc,
    PopularDesc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub directory: String,
    pub count: usize,
    pub sort: SortOrder,
    pub r18: bool,
    pub ai: bool,
    pub concurrent: usize,
    pub timeout: u64,
    pub retry: usize,
    pub with_tags: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            directory: DEFAULT_DOWNLOAD_DIRECTORY.to_string(),
            count: 0,
            sort: SortOrder::DateDesc,
            r18: false,
            ai: true,
            concurrent: 8,
            timeout: 30,
            retry: 3,
            with_tags: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Config {
    pub download: DownloadConfig,
    pub proxy: ProxyConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DownloadOverrides {
    pub directory: Option<PathBuf>,
    pub count: Option<usize>,
    pub sort: Option<SortOrder>,
    pub r18: Option<bool>,
    pub ai: Option<bool>,
    pub concurrent: Option<usize>,
    pub timeout: Option<u64>,
    pub retry: Option<usize>,
    pub with_tags: Option<bool>,
    pub proxy_url: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvOverrides {
    pub directory: Option<PathBuf>,
    pub count: Option<usize>,
    pub sort: Option<SortOrder>,
    pub r18: Option<bool>,
    pub ai: Option<bool>,
    pub concurrent: Option<usize>,
    pub timeout: Option<u64>,
    pub retry: Option<usize>,
    pub with_tags: Option<bool>,
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDownloadOptions {
    pub directory: PathBuf,
    pub count: usize,
    pub sort: SortOrder,
    pub r18: bool,
    pub ai: bool,
    pub concurrent: usize,
    pub timeout: u64,
    pub retry: usize,
    pub with_tags: bool,
    pub proxy_url: Option<String>,
    pub dry_run: bool,
}

impl Config {
    pub fn load() -> AppResult<Self> {
        Self::load_from(&config_file_path()?)
    }

    pub fn load_from(path: &Path) -> AppResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {}", path.display()))?;

        let config = toml::from_str::<Self>(&content)?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self) -> AppResult<()> {
        let dir = ensure_config_dir()?;
        self.save_to(&dir.join(CONFIG_FILE_NAME))
    }

    pub fn save_to(&self, path: &Path) -> AppResult<()> {
        self.validate()?;
        let content = toml::to_string_pretty(self)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建配置目录失败: {}", parent.display()))?;
        }

        fs::write(path, content)
            .with_context(|| format!("写入配置文件失败: {}", path.display()))?;

        Ok(())
    }

    fn validate(&self) -> AppResult<()> {
        validate_sort_order(self.download.sort)
    }

    pub fn resolve_download_options(
        &self,
        env: &EnvOverrides,
        cli: &DownloadOverrides,
    ) -> AppResult<ResolvedDownloadOptions> {
        let directory = cli
            .directory
            .clone()
            .or_else(|| env.directory.clone())
            .unwrap_or_else(|| PathBuf::from(self.download.directory.as_str()));

        let proxy_url = cli
            .proxy_url
            .clone()
            .or_else(|| env.proxy_url.clone())
            .or_else(|| {
                if self.proxy.url.trim().is_empty() {
                    None
                } else {
                    Some(self.proxy.url.clone())
                }
            });

        let resolved = ResolvedDownloadOptions {
            directory: expand_home_dir(&directory)?,
            count: cli.count.or(env.count).unwrap_or(self.download.count),
            sort: cli.sort.or(env.sort).unwrap_or(self.download.sort),
            r18: cli.r18.or(env.r18).unwrap_or(self.download.r18),
            ai: cli.ai.or(env.ai).unwrap_or(self.download.ai),
            concurrent: cli
                .concurrent
                .or(env.concurrent)
                .unwrap_or(self.download.concurrent),
            timeout: cli.timeout.or(env.timeout).unwrap_or(self.download.timeout),
            retry: cli.retry.or(env.retry).unwrap_or(self.download.retry),
            with_tags: cli
                .with_tags
                .or(env.with_tags)
                .unwrap_or(self.download.with_tags),
            proxy_url,
            dry_run: cli.dry_run,
        };

        validate_sort_order(resolved.sort)?;
        Ok(resolved)
    }
}

impl EnvOverrides {
    pub fn from_process_env() -> AppResult<Self> {
        Ok(Self {
            directory: env::var_os("PICALS_DOWNLOAD_DIRECTORY").map(PathBuf::from),
            count: parse_env_value("PICALS_DOWNLOAD_COUNT"),
            sort: env::var("PICALS_DOWNLOAD_SORT")
                .ok()
                .map(|value| parse_sort_value(&value))
                .transpose()?,
            r18: parse_env_bool("PICALS_DOWNLOAD_R18"),
            ai: parse_env_bool("PICALS_DOWNLOAD_AI"),
            concurrent: parse_env_value("PICALS_DOWNLOAD_CONCURRENT"),
            timeout: parse_env_value("PICALS_DOWNLOAD_TIMEOUT"),
            retry: parse_env_value("PICALS_DOWNLOAD_RETRY"),
            with_tags: parse_env_bool("PICALS_DOWNLOAD_WITH_TAGS"),
            proxy_url: env::var("PICALS_PROXY_URL")
                .ok()
                .or_else(|| env::var("HTTPS_PROXY").ok())
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

pub fn config_dir() -> AppResult<PathBuf> {
    let base = dirs_next::config_dir()
        .ok_or_else(|| eyre!(CrawlerError::Config("无法定位用户配置目录".to_string())))?;

    Ok(base.join(CONFIG_DIR_NAME))
}

pub fn ensure_config_dir() -> AppResult<PathBuf> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("创建配置目录失败: {}", dir.display()))?;
    Ok(dir)
}

pub fn config_file_path() -> AppResult<PathBuf> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

pub fn credential_file_path() -> AppResult<PathBuf> {
    Ok(config_dir()?.join(CREDENTIAL_FILE_NAME))
}

pub fn expand_home_dir(path: &Path) -> AppResult<PathBuf> {
    let raw = path.to_string_lossy();

    if raw == "~" {
        return dirs_next::home_dir().ok_or_else(|| {
            eyre!(CrawlerError::Config(
                "无法展开 ~，因为未找到 home 目录".to_string()
            ))
        });
    }

    if let Some(suffix) = raw.strip_prefix("~/") {
        let home = dirs_next::home_dir().ok_or_else(|| {
            eyre!(CrawlerError::Config(
                "无法展开 ~/，因为未找到 home 目录".to_string()
            ))
        })?;
        return Ok(home.join(suffix));
    }

    Ok(path.to_path_buf())
}

fn parse_env_value<T>(key: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    env::var(key).ok().and_then(|value| value.parse::<T>().ok())
}

fn parse_env_bool(key: &str) -> Option<bool> {
    env::var(key)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

pub(crate) fn parse_sort_value(value: &str) -> AppResult<SortOrder> {
    match value.trim().to_ascii_lowercase().as_str() {
        "date_desc" => Ok(SortOrder::DateDesc),
        "date_asc" => Ok(SortOrder::DateAsc),
        "popular_desc" => Err(popular_sort_migration_error().into()),
        _ => Err(invalid_sort_value_error(value).into()),
    }
}

pub(crate) fn invalid_sort_value_error(value: &str) -> CrawlerError {
    CrawlerError::InvalidInput(format!(
        "无效的排序值: {}，可选值为 date_desc/date_asc",
        value
    ))
}

pub(crate) fn popular_sort_migration_error() -> CrawlerError {
    CrawlerError::InvalidInput(POPULAR_SORT_MIGRATION_MESSAGE.to_string())
}

fn validate_sort_order(sort: SortOrder) -> AppResult<()> {
    if sort == SortOrder::PopularDesc {
        return Err(popular_sort_migration_error().into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{
        Config, DownloadOverrides, EnvOverrides, POPULAR_SORT_MIGRATION_MESSAGE, SortOrder,
        parse_sort_value,
    };

    #[test]
    fn cli_env_config_default_priority_is_correct() {
        let config = Config::default();
        let env = EnvOverrides {
            directory: Some(PathBuf::from("/env")),
            count: Some(12),
            sort: Some(SortOrder::DateAsc),
            r18: Some(true),
            ai: Some(false),
            concurrent: Some(16),
            timeout: Some(45),
            retry: Some(6),
            with_tags: Some(false),
            proxy_url: Some("socks5://127.0.0.1:1080".to_string()),
        };
        let cli = DownloadOverrides {
            directory: Some(PathBuf::from("/cli")),
            count: Some(24),
            sort: Some(SortOrder::DateAsc),
            r18: Some(false),
            ai: Some(true),
            concurrent: Some(4),
            timeout: Some(10),
            retry: Some(2),
            with_tags: Some(true),
            proxy_url: Some("http://127.0.0.1:7890".to_string()),
            dry_run: true,
        };

        let resolved = config.resolve_download_options(&env, &cli).unwrap();

        assert_eq!(resolved.directory, PathBuf::from("/cli"));
        assert_eq!(resolved.count, 24);
        assert_eq!(resolved.sort, SortOrder::DateAsc);
        assert!(!resolved.r18);
        assert!(resolved.ai);
        assert_eq!(resolved.concurrent, 4);
        assert_eq!(resolved.timeout, 10);
        assert_eq!(resolved.retry, 2);
        assert!(resolved.with_tags);
        assert_eq!(resolved.proxy_url.as_deref(), Some("http://127.0.0.1:7890"));
        assert!(resolved.dry_run);
    }

    #[test]
    fn config_value_is_used_when_no_override_exists() {
        let mut config = Config::default();
        config.download.count = 99;
        config.proxy.url = "socks5://127.0.0.1:1080".to_string();

        let resolved = config
            .resolve_download_options(&EnvOverrides::default(), &DownloadOverrides::default())
            .unwrap();

        assert_eq!(resolved.count, 99);
        assert_eq!(resolved.sort, SortOrder::DateDesc);
        assert_eq!(
            resolved.proxy_url.as_deref(),
            Some("socks5://127.0.0.1:1080")
        );
    }

    #[test]
    fn parse_sort_value_rejects_popular_sort() {
        let error = parse_sort_value("popular_desc").unwrap_err();
        assert!(format!("{error:#}").contains(POPULAR_SORT_MIGRATION_MESSAGE));
    }

    #[test]
    fn config_load_rejects_popular_sort_in_history_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[download]
directory = "~/Pictures/Pixiv"
count = 0
sort = "popular_desc"
r18 = false
ai = true
concurrent = 8
timeout = 30
retry = 3
with_tags = false

[proxy]
url = ""
"#,
        )
        .unwrap();

        let error = Config::load_from(&path).unwrap_err();
        assert!(format!("{error:#}").contains(POPULAR_SORT_MIGRATION_MESSAGE));
    }

    #[test]
    fn resolve_download_options_rejects_popular_sort_from_existing_config() {
        let mut config = Config::default();
        config.download.sort = SortOrder::PopularDesc;

        let error = config
            .resolve_download_options(&EnvOverrides::default(), &DownloadOverrides::default())
            .unwrap_err();

        assert!(format!("{error:#}").contains(POPULAR_SORT_MIGRATION_MESSAGE));
    }
}
