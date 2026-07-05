use crate::{
    auth::Credential,
    config::{Config, parse_batch_layout_value, parse_sort_value},
    error::{AppResult, CrawlerError},
};

use super::{
    prompt::ConfigPrompter,
    shared::{
        ConfigSnapshot, parse_bool, parse_string, parse_u64, parse_usize, render_bool,
        render_optional_value, render_sort_order, render_text_value,
    },
};

pub(in crate::commands::config_cmd) const CONFIG_FIELDS: [ConfigFieldKey; 17] = [
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::commands::config_cmd) enum ConfigUpdateNote {
    None,
    BatchLayoutChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::commands::config_cmd) enum ConfigFieldKey {
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

pub(in crate::commands::config_cmd) fn parse_config_field_key(
    key: &str,
) -> AppResult<ConfigFieldKey> {
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

impl ConfigFieldKey {
    pub(in crate::commands::config_cmd) fn key(self) -> &'static str {
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

    pub(in crate::commands::config_cmd) fn current_value(
        self,
        snapshot: &ConfigSnapshot,
    ) -> String {
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

    pub(in crate::commands::config_cmd) fn prompt_value(
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

    pub(in crate::commands::config_cmd) fn apply_value(
        self,
        value: &str,
    ) -> AppResult<ConfigUpdateNote> {
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

#[cfg(test)]
mod tests {
    use crate::{
        auth::Credential,
        config::{BatchLayoutStrategy, Config, SortOrder},
        error::AppResult,
    };

    use super::{
        super::{prompt::ConfigPrompter, shared::ConfigSnapshot},
        ConfigFieldKey,
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

        let value = ConfigFieldKey::DownloadBatchLayout
            .prompt_value(&snapshot, &prompter)
            .unwrap();

        assert_eq!(value, "per_illust");
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

        let value = ConfigFieldKey::ProxyUrl
            .prompt_value(&snapshot, &prompter)
            .unwrap();

        assert!(value.is_empty());
    }

    #[test]
    fn auth_user_id_current_value_reads_credential() {
        let snapshot = ConfigSnapshot {
            config: Config::default(),
            credential: Some(
                Credential::new_with_user_id("cookie-value", Some("12345678")).unwrap(),
            ),
        };

        assert_eq!(
            ConfigFieldKey::AuthUserId.current_value(&snapshot),
            "12345678"
        );
    }
}
