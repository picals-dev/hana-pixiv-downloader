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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IllustType {
    Image,
    Ugoira,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UgoiraFrame {
    pub file: String,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UgoiraMetadata {
    pub original_src: String,
    pub mime_type: Option<String>,
    pub frames: Vec<UgoiraFrame>,
}

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

pub fn select_keyword_total(value: &Value) -> Result<usize, CrawlerError> {
    let total = value
        .pointer("/body/illustManga/total")
        .and_then(Value::as_u64)
        .ok_or_else(|| CrawlerError::Parse("缺少 body.illustManga.total".to_string()))?;

    usize::try_from(total)
        .map_err(|_| CrawlerError::Parse(format!("关键词作品总数超出 usize 范围: {total}")))
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

pub fn select_ranking_total(value: &Value) -> Result<usize, CrawlerError> {
    let total = value
        .get("rank_total")
        .and_then(Value::as_u64)
        .ok_or_else(|| CrawlerError::Parse("缺少 rank_total 字段".to_string()))?;

    usize::try_from(total)
        .map_err(|_| CrawlerError::Parse(format!("排行榜总数超出 usize 范围: {total}")))
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

pub fn select_bookmark_total(value: &Value) -> Result<usize, CrawlerError> {
    let total = value
        .pointer("/body/total")
        .and_then(Value::as_u64)
        .ok_or_else(|| CrawlerError::Parse("缺少 body.total 字段".to_string()))?;

    usize::try_from(total)
        .map_err(|_| CrawlerError::Parse(format!("收藏作品总数超出 usize 范围: {total}")))
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

pub fn select_illust_type(value: &Value) -> Result<IllustType, CrawlerError> {
    let illust_type = value
        .pointer("/body/illustType")
        .and_then(|value| match value {
            Value::Number(number) => number.as_u64(),
            Value::String(text) => text.parse::<u64>().ok(),
            _ => None,
        })
        .ok_or_else(|| CrawlerError::Parse("缺少 body.illustType".to_string()))?;

    Ok(match illust_type {
        2 => IllustType::Ugoira,
        _ => IllustType::Image,
    })
}

pub fn select_ugoira_metadata(value: &Value) -> Result<UgoiraMetadata, CrawlerError> {
    let original_src = value
        .pointer("/body/originalSrc")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CrawlerError::Parse("缺少 body.originalSrc".to_string()))?;
    let frames = value
        .pointer("/body/frames")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlerError::Parse("缺少 body.frames 数组".to_string()))?;

    let mut parsed_frames = Vec::with_capacity(frames.len());
    for frame in frames {
        let file = frame
            .get("file")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CrawlerError::Parse("ugoira frame 缺少 file".to_string()))?;
        let delay_ms = frame
            .get("delay")
            .and_then(Value::as_u64)
            .ok_or_else(|| CrawlerError::Parse("ugoira frame 缺少 delay".to_string()))?;
        parsed_frames.push(UgoiraFrame {
            file: file.to_string(),
            delay_ms,
        });
    }

    if parsed_frames.is_empty() {
        return Err(CrawlerError::Parse("ugoira frames 为空".to_string()));
    }

    Ok(UgoiraMetadata {
        original_src: original_src.to_string(),
        mime_type: value
            .pointer("/body/mime_type")
            .and_then(Value::as_str)
            .map(str::to_string),
        frames: parsed_frames,
    })
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

pub fn count_user_illust_ids(value: &Value) -> Result<usize, CrawlerError> {
    Ok(select_user_illust_ids(value)?.len())
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

#[cfg(test)]
mod tests {
    use super::{IllustType, select_illust_type, select_ugoira_metadata};

    #[test]
    fn illust_type_two_is_ugoira() {
        let value = serde_json::json!({
            "body": {
                "illustType": 2
            }
        });

        assert_eq!(select_illust_type(&value).unwrap(), IllustType::Ugoira);
    }

    #[test]
    fn ugoira_metadata_can_be_selected() {
        let value = serde_json::json!({
            "body": {
                "originalSrc": "https://i.pximg.net/img-zip-ugoira/example.zip",
                "mime_type": "image/png",
                "frames": [
                    { "file": "000000.png", "delay": 60 },
                    { "file": "000001.png", "delay": 120 }
                ]
            }
        });

        let metadata = select_ugoira_metadata(&value).unwrap();
        assert_eq!(
            metadata.original_src,
            "https://i.pximg.net/img-zip-ugoira/example.zip"
        );
        assert_eq!(metadata.mime_type.as_deref(), Some("image/png"));
        assert_eq!(metadata.frames.len(), 2);
        assert_eq!(metadata.frames[0].file, "000000.png");
        assert_eq!(metadata.frames[0].delay_ms, 60);
    }
}
