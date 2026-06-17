//! Pixiv URL 解析工具。

use url::Url;

use crate::error::CrawlerError;

pub fn extract_user_id(input: &str) -> Result<String, CrawlerError> {
    extract_pixiv_id(input, "users")
        .map_err(|_| CrawlerError::InvalidInput(format!("无法识别画师 ID 或 URL: {input}")))
}

pub fn extract_illust_id(input: &str) -> Result<String, CrawlerError> {
    extract_pixiv_id(input, "artworks")
        .map_err(|_| CrawlerError::InvalidInput(format!("无法识别作品 ID 或 URL: {input}")))
}

fn extract_pixiv_id(input: &str, segment: &str) -> Result<String, CrawlerError> {
    let trimmed = input.trim();

    if !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Ok(trimmed.to_string());
    }

    let url = Url::parse(trimmed)?;
    let segments: Vec<_> = url
        .path_segments()
        .ok_or_else(|| CrawlerError::InvalidInput(format!("URL 缺少路径: {input}")))?
        .collect();

    let id = segments
        .windows(2)
        .find_map(|window| match window {
            [prefix, id] if *prefix == segment && !id.is_empty() => Some((*id).to_string()),
            _ => None,
        })
        .ok_or_else(|| CrawlerError::InvalidInput(format!("URL 中未找到 {segment} ID")))?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::{extract_illust_id, extract_user_id};

    #[test]
    fn user_id_can_be_extracted_from_plain_number() {
        assert_eq!(extract_user_id("12345678").unwrap(), "12345678");
    }

    #[test]
    fn user_id_can_be_extracted_from_url() {
        let url = "https://www.pixiv.net/users/12345678";
        assert_eq!(extract_user_id(url).unwrap(), "12345678");
    }

    #[test]
    fn illust_id_can_be_extracted_from_url() {
        let url = "https://www.pixiv.net/artworks/87654321";
        assert_eq!(extract_illust_id(url).unwrap(), "87654321");
    }
}
