//! Pixiv Ajax API 响应解析函数。

use serde_json::Value;

use crate::error::CrawlerError;

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
