//! `hpd config` 命令。

use std::path::Path;

use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};

use super::prompt_support::{
    prompt_batch_layout, prompt_bool, prompt_optional_text, prompt_sort_order, prompt_text,
    prompt_u64, prompt_usize,
};
use crate::{
    auth::Credential,
    cli::config::SetConfigArgs,
    config::{
        BatchLayoutStrategy, Config, SortOrder, config_dir, parse_batch_layout_value,
        parse_sort_value,
    },
    error::{AppResult, CrawlerError},
};

const CONFIG_SET_USAGE: &str = "hpd config set <KEY> <VALUE>";
const UNSET_VALUE: &str = "<未设置>";
const CONFIG_FIELDS: [ConfigFieldKey; 17] = [
    ConfigFieldKey::AuthPhpsessid,
    ConfigFieldKey::AuthUserId,
    ConfigFieldKey::DownloadBatchLayout,
    ConfigFieldKey::DownloadCount,
    ConfigFieldKey::DownloadSort,
    ConfigFieldKey::DownloadR18,
    ConfigFieldKey::DownloadAi,
    ConfigFieldKey::DownloadConcurrent,
    ConfigFieldKey::DownloadTimeout,
    ConfigFieldKey::DownloadRetry,
    ConfigFieldKey::DownloadWithTags,
    ConfigFieldKey::DownloadRootsIllust,
    ConfigFieldKey::DownloadRootsUser,
    ConfigFieldKey::DownloadRootsBookmark,
    ConfigFieldKey::DownloadRootsKeyword,
    ConfigFieldKey::DownloadRootsRanking,
    ConfigFieldKey::ProxyUrl,
];

#[derive(Debug, Clone)]
struct ConfigSnapshot {
    config: Config,
    credential: Option<Credential>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigTableRow {
    key: &'static str,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedSetConfigArgs {
    key: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SetInvocation {
    Help,
    MissingArgs,
    Prompt(String),
    Update(ResolvedSetConfigArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigUpdateNote {
    None,
    BatchLayoutChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigFieldKey {
    AuthPhpsessid,
    AuthUserId,
    DownloadBatchLayout,
    DownloadCount,
    DownloadSort,
    DownloadR18,
    DownloadAi,
    DownloadConcurrent,
    DownloadTimeout,
    DownloadRetry,
    DownloadWithTags,
    DownloadRootsIllust,
    DownloadRootsUser,
    DownloadRootsBookmark,
    DownloadRootsKeyword,
    DownloadRootsRanking,
    ProxyUrl,
}

trait ConfigPrompter {
    fn prompt_text(&self, message: &str, help: &str, default: &str) -> AppResult<String>;
    fn prompt_optional_text(&self, message: &str, help: &str, default: &str) -> AppResult<String>;
    fn prompt_usize(&self, message: &str, help: &str, default: usize) -> AppResult<usize>;
    fn prompt_u64(&self, message: &str, help: &str, default: u64) -> AppResult<u64>;
    fn prompt_bool(&self, message: &str, help: &str, default: bool) -> AppResult<bool>;
    fn prompt_sort_order(
        &self,
        message: &str,
        help: &str,
        default: SortOrder,
    ) -> AppResult<SortOrder>;
    fn prompt_batch_layout(
        &self,
        message: &str,
        help: &str,
        default: BatchLayoutStrategy,
    ) -> AppResult<BatchLayoutStrategy>;
}

struct InteractiveConfigPrompter;

impl ConfigPrompter for InteractiveConfigPrompter {
    fn prompt_text(&self, message: &str, help: &str, default: &str) -> AppResult<String> {
        prompt_text(message, help, default)
    }

    fn prompt_optional_text(&self, message: &str, help: &str, default: &str) -> AppResult<String> {
        prompt_optional_text(message, help, default)
    }

    fn prompt_usize(&self, message: &str, help: &str, default: usize) -> AppResult<usize> {
        prompt_usize(message, help, default)
    }

    fn prompt_u64(&self, message: &str, help: &str, default: u64) -> AppResult<u64> {
        prompt_u64(message, help, default)
    }

    fn prompt_bool(&self, message: &str, help: &str, default: bool) -> AppResult<bool> {
        prompt_bool(message, help, default)
    }

    fn prompt_sort_order(
        &self,
        message: &str,
        help: &str,
        default: SortOrder,
    ) -> AppResult<SortOrder> {
        prompt_sort_order(message, help, default)
    }

    fn prompt_batch_layout(
        &self,
        message: &str,
        help: &str,
        default: BatchLayoutStrategy,
    ) -> AppResult<BatchLayoutStrategy> {
        prompt_batch_layout(message, help, default)
    }
}

pub(crate) async fn show() -> AppResult<()> {
    let snapshot = load_config_snapshot()?;
    let rendered = render_show_output(&snapshot, &config_dir()?);
    println!("{rendered}");

    Ok(())
}

pub(crate) async fn set(args: SetConfigArgs) -> AppResult<()> {
    match resolve_set_invocation(args) {
        SetInvocation::Help => {
            let snapshot = load_config_snapshot()?;
            println!("{}", render_set_help_output(&snapshot, &config_dir()?));
            Ok(())
        }
        SetInvocation::MissingArgs => {
            let snapshot = load_config_snapshot()?;
            eprintln!("{}", render_set_help_output(&snapshot, &config_dir()?));
            Err(
                CrawlerError::InvalidInput(format!("请提供配置键和值。用法: {CONFIG_SET_USAGE}"))
                    .into(),
            )
        }
        SetInvocation::Prompt(key) => prompt_and_apply_config_update(key),
        SetInvocation::Update(args) => apply_config_update(args),
    }
}

fn prompt_and_apply_config_update(key: String) -> AppResult<()> {
    let snapshot = load_config_snapshot()?;
    let field = match parse_config_field_key(&key) {
        Ok(field) => field,
        Err(error) => {
            eprintln!("{}", render_set_help_output(&snapshot, &config_dir()?));
            return Err(error);
        }
    };

    let args = resolve_prompted_update_for_field(field, &snapshot, &InteractiveConfigPrompter)?;
    apply_config_update(args)
}

fn apply_config_update(args: ResolvedSetConfigArgs) -> AppResult<()> {
    let updated_key = args.key.clone();
    let field = parse_config_field_key(&args.key)?;
    let note = field.apply_value(&args.value)?;

    if note == ConfigUpdateNote::BatchLayoutChanged {
        println!("提示：该设置只会影响后续批量下载。");
        println!("如需按新布局整理之前设定下的已有目录，请运行：");
        println!("  hpd organize --dry-run");
        println!("  hpd organize --yes");
    }

    println!("✅ 已更新配置：{updated_key}");
    Ok(())
}

fn set_auth_phpsessid(value: &str) -> AppResult<()> {
    let mut credential = Credential::load()?.unwrap_or_else(|| Credential {
        phpsessid: String::new(),
        user_id: None,
    });
    credential.set_phpsessid(value.to_string())?;
    credential.save()?;
    Ok(())
}

fn set_auth_user_id(value: &str) -> AppResult<()> {
    let mut credential = Credential::load()?.ok_or_else(|| {
        CrawlerError::InvalidInput(
            "尚未配置 PHPSESSID，请先运行 setup 或先设置 auth.phpsessid".to_string(),
        )
    })?;
    credential.set_user_id(Some(value.to_string()))?;
    credential.save()?;
    Ok(())
}

fn resolve_set_invocation(args: SetConfigArgs) -> SetInvocation {
    if args.help {
        return SetInvocation::Help;
    }

    match (args.key, args.value) {
        (Some(key), Some(value)) => SetInvocation::Update(ResolvedSetConfigArgs { key, value }),
        (Some(key), None) => SetInvocation::Prompt(key),
        _ => SetInvocation::MissingArgs,
    }
}

fn load_config_snapshot() -> AppResult<ConfigSnapshot> {
    Ok(ConfigSnapshot {
        config: Config::load()?,
        credential: Credential::load()?,
    })
}

fn render_show_output(snapshot: &ConfigSnapshot, config_dir: &Path) -> String {
    format!(
        "配置目录: {}\n\n{}",
        config_dir.display(),
        render_config_table(snapshot)
    )
}

fn render_set_help_output(snapshot: &ConfigSnapshot, config_dir: &Path) -> String {
    format!(
        "用法:\n  {CONFIG_SET_USAGE}\n\n配置目录: {}\n\n可设置的配置字段与当前值：\n{}",
        config_dir.display(),
        render_config_table(snapshot)
    )
}

fn render_config_table(snapshot: &ConfigSnapshot) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["配置字段", "当前值"]);

    for row in collect_config_rows(snapshot) {
        table.add_row(vec![row.key, row.value.as_str()]);
    }

    table.to_string()
}

fn collect_config_rows(snapshot: &ConfigSnapshot) -> Vec<ConfigTableRow> {
    CONFIG_FIELDS
        .into_iter()
        .map(|field| ConfigTableRow {
            key: field.key(),
            value: field.current_value(snapshot),
        })
        .collect()
}

fn parse_config_field_key(key: &str) -> AppResult<ConfigFieldKey> {
    CONFIG_FIELDS
        .into_iter()
        .find(|field| field.key() == key)
        .ok_or_else(|| {
            CrawlerError::InvalidInput(format!(
                "不支持的配置键: {key}。可运行 `hpd config set --help` 查看完整字段表"
            ))
            .into()
        })
}

fn resolve_prompted_update_for_field(
    field: ConfigFieldKey,
    snapshot: &ConfigSnapshot,
    prompter: &impl ConfigPrompter,
) -> AppResult<ResolvedSetConfigArgs> {
    Ok(ResolvedSetConfigArgs {
        key: field.key().to_string(),
        value: field.prompt_value(snapshot, prompter)?,
    })
}

impl ConfigFieldKey {
    fn key(self) -> &'static str {
        match self {
            Self::AuthPhpsessid => "auth.phpsessid",
            Self::AuthUserId => "auth.user_id",
            Self::DownloadBatchLayout => "download.batch_layout",
            Self::DownloadCount => "download.count",
            Self::DownloadSort => "download.sort",
            Self::DownloadR18 => "download.r18",
            Self::DownloadAi => "download.ai",
            Self::DownloadConcurrent => "download.concurrent",
            Self::DownloadTimeout => "download.timeout",
            Self::DownloadRetry => "download.retry",
            Self::DownloadWithTags => "download.with_tags",
            Self::DownloadRootsIllust => "download.roots.illust",
            Self::DownloadRootsUser => "download.roots.user",
            Self::DownloadRootsBookmark => "download.roots.bookmark",
            Self::DownloadRootsKeyword => "download.roots.keyword",
            Self::DownloadRootsRanking => "download.roots.ranking",
            Self::ProxyUrl => "proxy.url",
        }
    }

    fn current_value(self, snapshot: &ConfigSnapshot) -> String {
        match self {
            Self::AuthPhpsessid => render_optional_value(
                snapshot
                    .credential
                    .as_ref()
                    .map(|value| value.phpsessid.as_str()),
            ),
            Self::AuthUserId => {
                render_optional_value(snapshot.credential.as_ref().and_then(Credential::user_id))
            }
            Self::DownloadBatchLayout => snapshot
                .config
                .download
                .batch_layout
                .display_name()
                .to_string(),
            Self::DownloadCount => snapshot.config.download.count.to_string(),
            Self::DownloadSort => render_sort_order(snapshot.config.download.sort).to_string(),
            Self::DownloadR18 => render_bool(snapshot.config.download.r18).to_string(),
            Self::DownloadAi => render_bool(snapshot.config.download.ai).to_string(),
            Self::DownloadConcurrent => snapshot.config.download.concurrent.to_string(),
            Self::DownloadTimeout => snapshot.config.download.timeout.to_string(),
            Self::DownloadRetry => snapshot.config.download.retry.to_string(),
            Self::DownloadWithTags => render_bool(snapshot.config.download.with_tags).to_string(),
            Self::DownloadRootsIllust => render_text_value(&snapshot.config.download.roots.illust),
            Self::DownloadRootsUser => render_text_value(&snapshot.config.download.roots.user),
            Self::DownloadRootsBookmark => {
                render_text_value(&snapshot.config.download.roots.bookmark)
            }
            Self::DownloadRootsKeyword => {
                render_text_value(&snapshot.config.download.roots.keyword)
            }
            Self::DownloadRootsRanking => {
                render_text_value(&snapshot.config.download.roots.ranking)
            }
            Self::ProxyUrl => render_text_value(&snapshot.config.proxy.url),
        }
    }

    fn prompt_value(
        self,
        snapshot: &ConfigSnapshot,
        prompter: &impl ConfigPrompter,
    ) -> AppResult<String> {
        match self {
            Self::AuthPhpsessid => {
                let current = snapshot
                    .credential
                    .as_ref()
                    .map(|value| value.phpsessid.as_str())
                    .unwrap_or_default();
                let value = prompter.prompt_text(
                    "PHPSESSID（auth.phpsessid）",
                    "请输入浏览器 Cookies 中的 PHPSESSID",
                    current,
                )?;
                Ok(Credential::new(value)?.phpsessid)
            }
            Self::AuthUserId => {
                let current = snapshot
                    .credential
                    .as_ref()
                    .and_then(Credential::user_id)
                    .unwrap_or_default();
                let value = prompter.prompt_text(
                    "当前账号 userId（auth.user_id）",
                    "请输入纯数字 userId；可从 Pixiv 个人主页 URL 中提取",
                    current,
                )?;
                Credential::parse_user_id(&value)
            }
            Self::DownloadBatchLayout => Ok(prompter
                .prompt_batch_layout(
                    "批量下载目录布局（download.batch_layout）",
                    "只影响多作品下载；mixed 会在单输出作品时直接平铺",
                    snapshot.config.download.batch_layout,
                )?
                .display_name()
                .to_string()),
            Self::DownloadCount => Ok(prompter
                .prompt_usize(
                    "默认下载数量（download.count）",
                    "0 表示下载当前模式可获取的全部内容",
                    snapshot.config.download.count,
                )?
                .to_string()),
            Self::DownloadSort => Ok(render_sort_order(prompter.prompt_sort_order(
                "默认排序方式（download.sort）",
                "支持按发布时间排序（新→旧 / 旧→新）",
                snapshot.config.download.sort,
            )?)
            .to_string()),
            Self::DownloadR18 => Ok(render_bool(prompter.prompt_bool(
                "默认开启 R-18 过滤（download.r18）",
                "仅影响支持该开关的命令",
                snapshot.config.download.r18,
            )?)
            .to_string()),
            Self::DownloadAi => Ok(render_bool(prompter.prompt_bool(
                "默认包含 AI 作品（download.ai）",
                "关闭后会尽量排除 AI 作品",
                snapshot.config.download.ai,
            )?)
            .to_string()),
            Self::DownloadConcurrent => Ok(prompter
                .prompt_usize(
                    "默认并发下载数（download.concurrent）",
                    "建议从 4 到 8 开始，避免过高并发触发限流",
                    snapshot.config.download.concurrent,
                )?
                .to_string()),
            Self::DownloadTimeout => Ok(prompter
                .prompt_u64(
                    "默认单次请求超时（秒）（download.timeout）",
                    "用于单次 HTTP 请求的超时上限",
                    snapshot.config.download.timeout,
                )?
                .to_string()),
            Self::DownloadRetry => Ok(prompter
                .prompt_usize(
                    "默认网络重试次数（download.retry）",
                    "当前版本的统一请求层会在此基础上执行收敛策略",
                    snapshot.config.download.retry,
                )?
                .to_string()),
            Self::DownloadWithTags => Ok(render_bool(prompter.prompt_bool(
                "默认导出 tags.json（download.with_tags）",
                "开启后会在下载目录中写入当前批次 tags.json",
                snapshot.config.download.with_tags,
            )?)
            .to_string()),
            Self::DownloadRootsIllust => prompter.prompt_text(
                "作品下载根目录（download.roots.illust）",
                "用于 download illust 的根目录；最终会继续追加作品目录",
                &snapshot.config.download.roots.illust,
            ),
            Self::DownloadRootsUser => prompter.prompt_text(
                "画师下载根目录（download.roots.user）",
                "用于 download user 的根目录；最终会追加 userId，再按 batch_layout 决定作品是否建目录",
                &snapshot.config.download.roots.user,
            ),
            Self::DownloadRootsBookmark => prompter.prompt_text(
                "收藏下载根目录（download.roots.bookmark）",
                "用于 download bookmark 的根目录；最终会追加 userId，再按 batch_layout 决定作品是否建目录",
                &snapshot.config.download.roots.bookmark,
            ),
            Self::DownloadRootsKeyword => prompter.prompt_text(
                "关键词下载根目录（download.roots.keyword）",
                "用于 download keyword 的根目录；最终会追加关键词目录，再按 batch_layout 决定作品是否建目录",
                &snapshot.config.download.roots.keyword,
            ),
            Self::DownloadRootsRanking => prompter.prompt_text(
                "排行榜下载根目录（download.roots.ranking）",
                "用于 download ranking 的根目录；最终会追加 mode，再按 batch_layout 决定作品是否建目录",
                &snapshot.config.download.roots.ranking,
            ),
            Self::ProxyUrl => prompter.prompt_optional_text(
                "默认代理地址（proxy.url）",
                "留空表示不使用代理，例如 socks5://127.0.0.1:1080",
                &snapshot.config.proxy.url,
            ),
        }
    }

    fn apply_value(self, value: &str) -> AppResult<ConfigUpdateNote> {
        match self {
            Self::AuthPhpsessid => {
                set_auth_phpsessid(value)?;
                Ok(ConfigUpdateNote::None)
            }
            Self::AuthUserId => {
                set_auth_user_id(value)?;
                Ok(ConfigUpdateNote::None)
            }
            _ => self.apply_regular_value(value),
        }
    }

    fn apply_regular_value(self, value: &str) -> AppResult<ConfigUpdateNote> {
        let mut config = Config::load()?;
        let previous_batch_layout = config.download.batch_layout;

        match self {
            Self::DownloadBatchLayout => {
                config.download.batch_layout = parse_batch_layout_value(value)?;
            }
            Self::DownloadCount => config.download.count = parse_usize(self.key(), value)?,
            Self::DownloadSort => config.download.sort = parse_sort_value(value)?,
            Self::DownloadR18 => config.download.r18 = parse_bool(self.key(), value)?,
            Self::DownloadAi => config.download.ai = parse_bool(self.key(), value)?,
            Self::DownloadConcurrent => {
                config.download.concurrent = parse_usize(self.key(), value)?;
            }
            Self::DownloadTimeout => config.download.timeout = parse_u64(self.key(), value)?,
            Self::DownloadRetry => config.download.retry = parse_usize(self.key(), value)?,
            Self::DownloadWithTags => config.download.with_tags = parse_bool(self.key(), value)?,
            Self::DownloadRootsIllust => config.download.roots.illust = parse_string(value)?,
            Self::DownloadRootsUser => config.download.roots.user = parse_string(value)?,
            Self::DownloadRootsBookmark => config.download.roots.bookmark = parse_string(value)?,
            Self::DownloadRootsKeyword => config.download.roots.keyword = parse_string(value)?,
            Self::DownloadRootsRanking => config.download.roots.ranking = parse_string(value)?,
            Self::ProxyUrl => config.proxy.url = value.to_string(),
            Self::AuthPhpsessid | Self::AuthUserId => unreachable!("认证字段不走常规配置写入"),
        }

        config.save()?;
        Ok(
            if self == Self::DownloadBatchLayout
                && config.download.batch_layout != previous_batch_layout
            {
                ConfigUpdateNote::BatchLayoutChanged
            } else {
                ConfigUpdateNote::None
            },
        )
    }
}

fn render_sort_order(value: SortOrder) -> &'static str {
    match value {
        SortOrder::DateDesc => "date_desc",
        SortOrder::DateAsc => "date_asc",
    }
}

fn render_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn render_optional_value(value: Option<&str>) -> String {
    value
        .map(render_text_value)
        .unwrap_or_else(|| UNSET_VALUE.to_string())
}

fn render_text_value(value: &str) -> String {
    if value.trim().is_empty() {
        UNSET_VALUE.to_string()
    } else {
        value.to_string()
    }
}

fn parse_string(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CrawlerError::InvalidInput("配置值不能为空".to_string()).into());
    }

    Ok(trimmed.to_string())
}

fn parse_bool(key: &str, value: &str) -> AppResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(CrawlerError::InvalidInput(format!("{key} 需要布尔值（true/false）")).into()),
    }
}

fn parse_usize(key: &str, value: &str) -> AppResult<usize> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| CrawlerError::InvalidInput(format!("{key} 需要无符号整数")).into())
}

fn parse_u64(key: &str, value: &str) -> AppResult<u64> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| CrawlerError::InvalidInput(format!("{key} 需要无符号整数")).into())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::tempdir;

    use crate::{
        auth::Credential,
        cli::config::SetConfigArgs,
        config::{BatchLayoutStrategy, Config, DownloadRootsConfig, SortOrder},
        error::AppResult,
        test_support::{EnvVarGuard, lock_env},
    };

    use super::{
        CONFIG_SET_USAGE, ConfigFieldKey, ConfigPrompter, ConfigSnapshot, SetInvocation,
        render_config_table, render_set_help_output, render_show_output,
        resolve_prompted_update_for_field, resolve_set_invocation, set,
    };

    struct StubPrompter {
        text: String,
        optional_text: String,
        usize_value: usize,
        u64_value: u64,
        bool_value: bool,
        sort_value: SortOrder,
        batch_layout_value: BatchLayoutStrategy,
    }

    impl Default for StubPrompter {
        fn default() -> Self {
            Self {
                text: "stub".to_string(),
                optional_text: String::new(),
                usize_value: 7,
                u64_value: 42,
                bool_value: true,
                sort_value: SortOrder::DateAsc,
                batch_layout_value: BatchLayoutStrategy::Flat,
            }
        }
    }

    impl ConfigPrompter for StubPrompter {
        fn prompt_text(&self, _message: &str, _help: &str, _default: &str) -> AppResult<String> {
            Ok(self.text.clone())
        }

        fn prompt_optional_text(
            &self,
            _message: &str,
            _help: &str,
            _default: &str,
        ) -> AppResult<String> {
            Ok(self.optional_text.clone())
        }

        fn prompt_usize(&self, _message: &str, _help: &str, _default: usize) -> AppResult<usize> {
            Ok(self.usize_value)
        }

        fn prompt_u64(&self, _message: &str, _help: &str, _default: u64) -> AppResult<u64> {
            Ok(self.u64_value)
        }

        fn prompt_bool(&self, _message: &str, _help: &str, _default: bool) -> AppResult<bool> {
            Ok(self.bool_value)
        }

        fn prompt_sort_order(
            &self,
            _message: &str,
            _help: &str,
            _default: SortOrder,
        ) -> AppResult<SortOrder> {
            Ok(self.sort_value)
        }

        fn prompt_batch_layout(
            &self,
            _message: &str,
            _help: &str,
            _default: BatchLayoutStrategy,
        ) -> AppResult<BatchLayoutStrategy> {
            Ok(self.batch_layout_value)
        }
    }

    #[tokio::test]
    async fn config_set_can_update_auth_keys() {
        let _lock = lock_env().await;
        let temp = tempdir().unwrap();
        let xdg_home = temp.path().join(".config");
        let _xdg = EnvVarGuard::set("XDG_CONFIG_HOME", &xdg_home);

        set(SetConfigArgs {
            help: false,
            key: Some("auth.phpsessid".to_string()),
            value: Some("cookie-value".to_string()),
        })
        .await
        .unwrap();
        set(SetConfigArgs {
            help: false,
            key: Some("auth.user_id".to_string()),
            value: Some("12345678".to_string()),
        })
        .await
        .unwrap();

        let credential = Credential::load().unwrap().unwrap();
        assert_eq!(credential.phpsessid, "cookie-value");
        assert_eq!(credential.user_id(), Some("12345678"));
    }

    #[test]
    fn config_show_and_set_help_reuse_same_table() {
        let mut config = Config::default();
        config.download.count = 12;
        config.download.sort = SortOrder::DateAsc;
        config.download.roots = DownloadRootsConfig {
            illust: "/tmp/illust".to_string(),
            user: "/tmp/user".to_string(),
            bookmark: "/tmp/bookmark".to_string(),
            keyword: "/tmp/keyword".to_string(),
            ranking: "/tmp/ranking".to_string(),
        };
        config.proxy.url = "socks5://127.0.0.1:1080".to_string();

        let snapshot = ConfigSnapshot {
            config,
            credential: Some(
                Credential::new_with_user_id("cookie-value", Some("12345678")).unwrap(),
            ),
        };

        let table = render_config_table(&snapshot);
        let show_output = render_show_output(&snapshot, Path::new("/tmp/hpd"));
        let help_output = render_set_help_output(&snapshot, Path::new("/tmp/hpd"));

        assert!(table.contains("配置字段"));
        assert!(table.contains("当前值"));
        assert!(table.contains("auth.phpsessid"));
        assert!(table.contains("cookie-value"));
        assert!(table.contains("download.sort"));
        assert!(table.contains("date_asc"));
        assert!(table.contains("download.roots.ranking"));
        assert!(table.contains("/tmp/ranking"));
        assert!(table.contains("proxy.url"));
        assert!(table.contains("socks5://127.0.0.1:1080"));

        assert!(show_output.contains(&table));
        assert!(help_output.contains(&table));
        assert!(help_output.contains(CONFIG_SET_USAGE));
    }

    #[test]
    fn config_table_marks_missing_values_as_unset() {
        let snapshot = ConfigSnapshot {
            config: Config::default(),
            credential: None,
        };

        let table = render_config_table(&snapshot);

        assert!(table.contains("auth.phpsessid"));
        assert!(table.contains("<未设置>"));
        assert!(table.contains("proxy.url"));
    }

    #[test]
    fn config_set_key_only_enters_prompt_mode() {
        let invocation = resolve_set_invocation(SetConfigArgs {
            help: false,
            key: Some("download.batch_layout".to_string()),
            value: None,
        });

        assert_eq!(
            invocation,
            SetInvocation::Prompt("download.batch_layout".to_string())
        );
    }

    #[test]
    fn prompted_batch_layout_uses_select_result() {
        let snapshot = ConfigSnapshot {
            config: Config::default(),
            credential: None,
        };
        let prompter = StubPrompter {
            batch_layout_value: BatchLayoutStrategy::PerIllust,
            ..Default::default()
        };

        let resolved = resolve_prompted_update_for_field(
            ConfigFieldKey::DownloadBatchLayout,
            &snapshot,
            &prompter,
        )
        .unwrap();

        assert_eq!(resolved.key, "download.batch_layout");
        assert_eq!(resolved.value, "per_illust");
    }

    #[test]
    fn prompted_proxy_url_can_be_cleared() {
        let mut config = Config::default();
        config.proxy.url = "socks5://127.0.0.1:1080".to_string();
        let snapshot = ConfigSnapshot {
            config,
            credential: None,
        };
        let prompter = StubPrompter {
            optional_text: String::new(),
            ..Default::default()
        };

        let resolved =
            resolve_prompted_update_for_field(ConfigFieldKey::ProxyUrl, &snapshot, &prompter)
                .unwrap();

        assert_eq!(resolved.key, "proxy.url");
        assert!(resolved.value.is_empty());
    }
}
