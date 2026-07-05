use crate::{
    auth::Credential,
    config::{Config, SortOrder},
    error::{AppResult, CrawlerError},
};

pub(in crate::commands::config_cmd) const CONFIG_SET_USAGE: &str = "hpd config set <KEY> <VALUE>";
const UNSET_VALUE: &str = "<未设置>";

#[derive(Debug, Clone)]
pub(in crate::commands::config_cmd) struct ConfigSnapshot {
    pub(in crate::commands::config_cmd) config: Config,
    pub(in crate::commands::config_cmd) credential: Option<Credential>,
}

pub(in crate::commands::config_cmd) fn load_config_snapshot() -> AppResult<ConfigSnapshot> {
    Ok(ConfigSnapshot {
        config: Config::load()?,
        credential: Credential::load()?,
    })
}

pub(in crate::commands::config_cmd) fn render_sort_order(value: SortOrder) -> &'static str {
    match value {
        SortOrder::DateDesc => "date_desc",
        SortOrder::DateAsc => "date_asc",
    }
}

pub(in crate::commands::config_cmd) fn render_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

pub(in crate::commands::config_cmd) fn render_optional_value(value: Option<&str>) -> String {
    value
        .map(render_text_value)
        .unwrap_or_else(|| UNSET_VALUE.to_string())
}

pub(in crate::commands::config_cmd) fn render_text_value(value: &str) -> String {
    if value.trim().is_empty() {
        UNSET_VALUE.to_string()
    } else {
        value.to_string()
    }
}

pub(in crate::commands::config_cmd) fn parse_string(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CrawlerError::InvalidInput("配置值不能为空".to_string()).into());
    }

    Ok(trimmed.to_string())
}

pub(in crate::commands::config_cmd) fn parse_bool(key: &str, value: &str) -> AppResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(CrawlerError::InvalidInput(format!("{key} 需要布尔值（true/false）")).into()),
    }
}

pub(in crate::commands::config_cmd) fn parse_usize(key: &str, value: &str) -> AppResult<usize> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| CrawlerError::InvalidInput(format!("{key} 需要无符号整数")).into())
}

pub(in crate::commands::config_cmd) fn parse_u64(key: &str, value: &str) -> AppResult<u64> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| CrawlerError::InvalidInput(format!("{key} 需要无符号整数")).into())
}
