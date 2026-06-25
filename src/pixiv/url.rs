//! Pixiv URL 解析工具。

use url::Url;

use crate::error::CrawlerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PixivUrlTarget {
    User { user_id: String },
    Illust { illust_id: String },
    Keyword { query: String },
}

pub(crate) fn extract_user_id(input: &str) -> Result<String, CrawlerError> {
    if let Some(user_id) = extract_numeric_id(input) {
        return Ok(user_id);
    }

    match parse_pixiv_url_target(input)? {
        PixivUrlTarget::User { user_id } => Ok(user_id),
        _ => Err(CrawlerError::InvalidInput(format!(
            "无法识别画师 ID 或 URL: {input}"
        ))),
    }
}

pub(crate) fn extract_illust_id(input: &str) -> Result<String, CrawlerError> {
    if let Some(illust_id) = extract_numeric_id(input) {
        return Ok(illust_id);
    }

    match parse_pixiv_url_target(input)? {
        PixivUrlTarget::Illust { illust_id } => Ok(illust_id),
        _ => Err(CrawlerError::InvalidInput(format!(
            "无法识别作品 ID 或 URL: {input}"
        ))),
    }
}

pub(crate) fn parse_pixiv_url_target(input: &str) -> Result<PixivUrlTarget, CrawlerError> {
    let trimmed = input.trim();
    let url = Url::parse(trimmed)?;
    let segments: Vec<_> = url
        .path_segments()
        .ok_or_else(|| CrawlerError::InvalidInput(format!("URL 缺少路径: {input}")))?
        .collect();

    match segments.as_slice() {
        ["users", user_id, ..] if !user_id.is_empty() => Ok(PixivUrlTarget::User {
            user_id: (*user_id).to_string(),
        }),
        ["artworks", illust_id, ..] if !illust_id.is_empty() => Ok(PixivUrlTarget::Illust {
            illust_id: (*illust_id).to_string(),
        }),
        ["tags", query] | ["tags", query, "artworks"] if !query.is_empty() => {
            Ok(PixivUrlTarget::Keyword {
                query: decode_path_segment(query)?,
            })
        }
        _ => Err(CrawlerError::InvalidInput(format!(
            "暂不支持从该 Pixiv URL 自动识别下载类型: {input}"
        ))),
    }
}

fn extract_numeric_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    (!trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| trimmed.to_string())
}

fn decode_path_segment(input: &str) -> Result<String, CrawlerError> {
    let mut bytes = Vec::with_capacity(input.len());
    let raw = input.as_bytes();
    let mut index = 0;

    while index < raw.len() {
        match raw[index] {
            b'%' => {
                if index + 2 >= raw.len() {
                    return Err(CrawlerError::InvalidInput(format!(
                        "URL 路径包含不完整的百分号编码: {input}"
                    )));
                }

                let high = decode_hex(raw[index + 1])?;
                let low = decode_hex(raw[index + 2])?;
                bytes.push((high << 4) | low);
                index += 3;
            }
            byte => {
                bytes.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(bytes)
        .map_err(|_| CrawlerError::InvalidInput(format!("URL 路径不是合法的 UTF-8 编码: {input}")))
}

fn decode_hex(value: u8) -> Result<u8, CrawlerError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(CrawlerError::InvalidInput(format!(
            "URL 路径包含非法的百分号编码: {}",
            value as char
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{PixivUrlTarget, extract_illust_id, extract_user_id, parse_pixiv_url_target};

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

    #[test]
    fn keyword_can_be_extracted_from_tag_url() {
        let url = "https://www.pixiv.net/tags/%E9%AD%94%E6%B3%95%E5%B0%91%E5%A5%B3%E3%83%8E%E9%AD%94%E5%A5%B3%E8%A3%81%E5%88%A4";
        assert_eq!(
            parse_pixiv_url_target(url).unwrap(),
            PixivUrlTarget::Keyword {
                query: "魔法少女ノ魔女裁判".to_string()
            }
        );
    }

    #[test]
    fn keyword_can_be_extracted_from_tag_artworks_url() {
        let url = "https://www.pixiv.net/tags/%E5%88%9D%E9%9F%B3%E3%83%9F%E3%82%AF/artworks";
        assert_eq!(
            parse_pixiv_url_target(url).unwrap(),
            PixivUrlTarget::Keyword {
                query: "初音ミク".to_string()
            }
        );
    }

    #[test]
    fn unsupported_pixiv_url_returns_error() {
        let url = "https://www.pixiv.net/novel/show.php?id=1";
        let error = parse_pixiv_url_target(url).unwrap_err();
        assert!(format!("{error:#}").contains("暂不支持从该 Pixiv URL 自动识别下载类型"));
    }
}
