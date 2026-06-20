//! Pixiv Ajax API 响应解析函数。

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

use crate::error::CrawlerError;

static CURRENT_USER_ID_PATTERNS: LazyLock<[Regex; 4]> = LazyLock::new(|| {
    [
        Regex::new(r#"pixiv\.user\.id\s*=\s*"(\d+)""#).expect("valid regex"),
        Regex::new(r#"user_id:\s*['"](\d+)['"]"#).expect("valid regex"),
        Regex::new(r#"["']userId["']\s*:\s*["'](\d+)["']"#).expect("valid regex"),
        Regex::new(r#"["']user_id["']\s*:\s*["'](\d+)["']"#).expect("valid regex"),
    ]
});

pub fn select_user_illust_ids(value: &Value) -> Result<Vec<String>, CrawlerError> {
    let body = value
        .get("body")
        .and_then(Value::as_object)
        .ok_or_else(|| CrawlerError::Parse("缺少 body 字段".to_string()))?;

    let mut ids = Vec::new();
    collect_map_keys(body.get("illusts"), &mut ids);
    collect_map_keys(body.get("manga"), &mut ids);
    ids.sort();
    ids.dedup();

    Ok(ids)
}

pub fn select_keyword_illust_ids(value: &Value) -> Result<Vec<String>, CrawlerError> {
    let items = value
        .pointer("/body/illustManga/data")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlerError::Parse("缺少 body.illustManga.data 数组".to_string()))?;

    let mut ids = Vec::new();

    for item in items {
        if let Some(id) = item.get("id").and_then(Value::as_str) {
            ids.push(id.to_string());
        }
    }

    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub fn select_ranking_illust_ids(value: &Value) -> Result<Vec<String>, CrawlerError> {
    let items = value
        .get("contents")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlerError::Parse("缺少 contents 数组".to_string()))?;

    let mut ids = Vec::new();

    for item in items {
        match item.get("illust_id") {
            Some(Value::String(id)) => ids.push(id.clone()),
            Some(Value::Number(id)) => ids.push(id.to_string()),
            _ => {}
        }
    }

    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub fn select_bookmark_illust_ids(value: &Value) -> Result<Vec<String>, CrawlerError> {
    let items = value
        .pointer("/body/works")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlerError::Parse("缺少 body.works 数组".to_string()))?;

    let mut ids = Vec::new();

    for item in items {
        match item.get("id") {
            Some(Value::String(id)) => ids.push(id.clone()),
            Some(Value::Number(id)) => ids.push(id.to_string()),
            _ => {}
        }
    }

    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub fn select_current_user_id(
    header_user_id: Option<&str>,
    html: &str,
) -> Result<String, CrawlerError> {
    if let Some(user_id) = header_user_id {
        return normalize_user_id(user_id);
    }

    for pattern in CURRENT_USER_ID_PATTERNS.iter() {
        if let Some(captures) = pattern.captures(html)
            && let Some(user_id) = captures.get(1)
        {
            return normalize_user_id(user_id.as_str());
        }
    }

    Err(CrawlerError::Parse(
        "当前账号身份无法解析：未能从首页响应头或 HTML 中提取 userId".to_string(),
    ))
}

pub fn select_page_original_urls(value: &Value) -> Result<Vec<String>, CrawlerError> {
    let items = value
        .get("body")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlerError::Parse("缺少 body 数组".to_string()))?;

    let mut urls = Vec::new();
    for item in items {
        if let Some(url) = item
            .get("urls")
            .and_then(|urls| urls.get("original"))
            .and_then(Value::as_str)
        {
            urls.push(url.to_string());
        }
    }

    Ok(urls)
}

pub fn select_illust_tags(value: &Value) -> Result<Vec<String>, CrawlerError> {
    let tags = value
        .pointer("/body/tags/tags")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlerError::Parse("缺少 body.tags.tags 数组".to_string()))?;

    let mut result = Vec::new();

    for tag in tags {
        if let Some(translated) = tag
            .get("translation")
            .and_then(|translation| translation.get("en"))
            .and_then(Value::as_str)
        {
            result.push(translated.to_string());
            continue;
        }

        if let Some(raw_tag) = tag.get("tag").and_then(Value::as_str) {
            result.push(raw_tag.to_string());
        }
    }

    Ok(result)
}

fn collect_map_keys(value: Option<&Value>, ids: &mut Vec<String>) {
    if let Some(map) = value.and_then(Value::as_object) {
        ids.extend(map.keys().cloned());
    }
}

fn normalize_user_id(user_id: &str) -> Result<String, CrawlerError> {
    let trimmed = user_id.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(CrawlerError::Parse(format!(
            "当前账号身份无法解析：userId 不是纯数字: {user_id}"
        )));
    }

    Ok(trimmed.to_string())
}
