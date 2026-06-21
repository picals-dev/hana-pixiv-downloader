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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadMode {
    Illust,
    User,
    Bookmark,
    Keyword,
    Ranking,
}

impl DownloadMode {
    pub fn as_config_key(self) -> &'static str {
        match self {
            Self::Illust => "illust",
            Self::User => "user",
            Self::Bookmark => "bookmark",
            Self::Keyword => "keyword",
            Self::Ranking => "ranking",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadRootsConfig {
    pub illust: String,
    pub user: String,
    pub bookmark: String,
    pub keyword: String,
    pub ranking: String,
}

impl Default for DownloadRootsConfig {
    fn default() -> Self {
        Self::from_seed(DEFAULT_DOWNLOAD_DIRECTORY)
    }
}

impl DownloadRootsConfig {
    pub fn from_seed(seed: &str) -> Self {
        Self {
            illust: join_root_seed(seed, "illust"),
            user: join_root_seed(seed, "user"),
            bookmark: join_root_seed(seed, "bookmark"),
            keyword: join_root_seed(seed, "keyword"),
            ranking: join_root_seed(seed, "ranking"),
        }
    }

    pub fn get(&self, mode: DownloadMode) -> &str {
        match mode {
            DownloadMode::Illust => &self.illust,
            DownloadMode::User => &self.user,
            DownloadMode::Bookmark => &self.bookmark,
            DownloadMode::Keyword => &self.keyword,
            DownloadMode::Ranking => &self.ranking,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DownloadConfig {
    pub roots: DownloadRootsConfig,
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
            roots: DownloadRootsConfig::default(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
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
    pub mode: DownloadMode,
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

#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    download: RawDownloadConfig,
    #[serde(default)]
    proxy: RawProxyConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawDownloadConfig {
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    roots: Option<RawDownloadRootsConfig>,
    #[serde(default)]
    count: Option<usize>,
    #[serde(default)]
    sort: Option<SortOrder>,
    #[serde(default)]
    r18: Option<bool>,
    #[serde(default)]
    ai: Option<bool>,
    #[serde(default)]
    concurrent: Option<usize>,
    #[serde(default)]
    timeout: Option<u64>,
    #[serde(default)]
    retry: Option<usize>,
    #[serde(default)]
    with_tags: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawDownloadRootsConfig {
    #[serde(default)]
    illust: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    bookmark: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    ranking: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawProxyConfig {
    #[serde(default)]
    url: Option<String>,
}

impl RawDownloadRootsConfig {
    fn resolve(self, defaults: &DownloadRootsConfig) -> DownloadRootsConfig {
        DownloadRootsConfig {
            illust: self.illust.unwrap_or_else(|| defaults.illust.clone()),
            user: self.user.unwrap_or_else(|| defaults.user.clone()),
            bookmark: self.bookmark.unwrap_or_else(|| defaults.bookmark.clone()),
            keyword: self.keyword.unwrap_or_else(|| defaults.keyword.clone()),
            ranking: self.ranking.unwrap_or_else(|| defaults.ranking.clone()),
        }
    }
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

        let raw = toml::from_str::<RawConfig>(&content)?;
        let config = Self::from_raw(raw)?;
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

    pub fn resolve_download_options(
        &self,
        mode: DownloadMode,
        env: &EnvOverrides,
        cli: &DownloadOverrides,
    ) -> AppResult<ResolvedDownloadOptions> {
        let directory = cli
            .directory
            .clone()
            .or_else(|| env.directory.clone())
            .unwrap_or_else(|| PathBuf::from(self.download.roots.get(mode)));

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
            mode,
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

    fn from_raw(raw: RawConfig) -> AppResult<Self> {
        let defaults = DownloadConfig::default();
        let seed = raw
            .download
            .directory
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_DOWNLOAD_DIRECTORY);
        let root_defaults = DownloadRootsConfig::from_seed(seed);
        let roots = raw
            .download
            .roots
            .map(|value| value.resolve(&root_defaults))
            .unwrap_or(root_defaults);

        Ok(Self {
            download: DownloadConfig {
                roots,
                count: raw.download.count.unwrap_or(defaults.count),
                sort: raw.download.sort.unwrap_or(defaults.sort),
                r18: raw.download.r18.unwrap_or(defaults.r18),
                ai: raw.download.ai.unwrap_or(defaults.ai),
                concurrent: raw.download.concurrent.unwrap_or(defaults.concurrent),
                timeout: raw.download.timeout.unwrap_or(defaults.timeout),
                retry: raw.download.retry.unwrap_or(defaults.retry),
                with_tags: raw.download.with_tags.unwrap_or(defaults.with_tags),
            },
            proxy: ProxyConfig {
                url: raw.proxy.url.unwrap_or_default(),
            },
        })
    }

    fn validate(&self) -> AppResult<()> {
        validate_sort_order(self.download.sort)?;
        validate_non_empty_path("download.roots.illust", &self.download.roots.illust)?;
        validate_non_empty_path("download.roots.user", &self.download.roots.user)?;
        validate_non_empty_path("download.roots.bookmark", &self.download.roots.bookmark)?;
        validate_non_empty_path("download.roots.keyword", &self.download.roots.keyword)?;
        validate_non_empty_path("download.roots.ranking", &self.download.roots.ranking)?;
        Ok(())
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
    let base = if let Some(xdg_config_home) = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
    {
        xdg_config_home
    } else {
        dirs_next::config_dir()
            .ok_or_else(|| eyre!(CrawlerError::Config("无法定位用户配置目录".to_string())))?
    };

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

fn validate_non_empty_path(key: &str, value: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(CrawlerError::InvalidInput(format!("{key} 不能为空")).into());
    }

    Ok(())
}

fn join_root_seed(seed: &str, suffix: &str) -> String {
    let seed = seed.trim();
    if seed.is_empty() {
        return suffix.to_string();
    }

    PathBuf::from(seed)
        .join(suffix)
        .to_string_lossy()
        .into_owned()
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{
        Config, DownloadMode, DownloadOverrides, DownloadRootsConfig, EnvOverrides,
        POPULAR_SORT_MIGRATION_MESSAGE, SortOrder, parse_sort_value,
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

        let resolved = config
            .resolve_download_options(DownloadMode::User, &env, &cli)
            .unwrap();

        assert_eq!(resolved.mode, DownloadMode::User);
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
            .resolve_download_options(
                DownloadMode::Keyword,
                &EnvOverrides::default(),
                &DownloadOverrides::default(),
            )
            .unwrap();

        assert_eq!(resolved.count, 99);
        assert_eq!(resolved.sort, SortOrder::DateDesc);
        assert_eq!(
            resolved.directory,
            super::expand_home_dir(Path::new("~/Pictures/Pixiv/keyword")).unwrap()
        );
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
            .resolve_download_options(
                DownloadMode::Illust,
                &EnvOverrides::default(),
                &DownloadOverrides::default(),
            )
            .unwrap_err();

        assert!(format!("{error:#}").contains(POPULAR_SORT_MIGRATION_MESSAGE));
    }

    #[test]
    fn legacy_download_directory_seeds_all_mode_roots() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[download]
directory = "~/Downloads/LegacyPixiv"
count = 5

[proxy]
url = ""
"#,
        )
        .unwrap();

        let config = Config::load_from(&path).unwrap();

        assert_eq!(
            config.download.roots,
            DownloadRootsConfig {
                illust: "~/Downloads/LegacyPixiv/illust".to_string(),
                user: "~/Downloads/LegacyPixiv/user".to_string(),
                bookmark: "~/Downloads/LegacyPixiv/bookmark".to_string(),
                keyword: "~/Downloads/LegacyPixiv/keyword".to_string(),
                ranking: "~/Downloads/LegacyPixiv/ranking".to_string(),
            }
        );
        assert_eq!(config.download.count, 5);
    }

    #[test]
    fn partial_roots_fall_back_to_legacy_seed() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"[download]
directory = "~/Downloads/LegacyPixiv"

[download.roots]
illust = "~/CustomIllust"
keyword = "~/CustomKeyword"
"#,
        )
        .unwrap();

        let config = Config::load_from(&path).unwrap();

        assert_eq!(config.download.roots.illust, "~/CustomIllust");
        assert_eq!(config.download.roots.keyword, "~/CustomKeyword");
        assert_eq!(config.download.roots.user, "~/Downloads/LegacyPixiv/user");
        assert_eq!(
            config.download.roots.bookmark,
            "~/Downloads/LegacyPixiv/bookmark"
        );
    }

    #[test]
    fn save_does_not_write_legacy_directory_key() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        let config = Config::default();

        config.save_to(&path).unwrap();
        let saved = std::fs::read_to_string(path).unwrap();

        assert!(saved.contains("[download.roots]"));
        assert!(!saved.contains("directory = "));
    }
}
