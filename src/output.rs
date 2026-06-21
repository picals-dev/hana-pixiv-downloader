//! 下载输出路径解析。

use std::path::{Path, PathBuf};

use crate::{
    config::DownloadMode,
    error::{AppResult, CrawlerError},
};

const KEYWORD_SEGMENT_MAX_LEN: usize = 80;
const HASH_SUFFIX_LEN: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLayout {
    mode: DownloadMode,
    root: PathBuf,
    context_dir: PathBuf,
}

impl OutputLayout {
    pub fn context_dir(&self) -> &Path {
        &self.context_dir
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn illust_dir(&self, illust_id: &str) -> AppResult<PathBuf> {
        let illust_id = validate_controlled_segment("illustId", illust_id)?;

        Ok(match self.mode {
            DownloadMode::Illust => self.context_dir.clone(),
            DownloadMode::User
            | DownloadMode::Bookmark
            | DownloadMode::Keyword
            | DownloadMode::Ranking => self.context_dir.join(illust_id),
        })
    }
}

pub fn resolve_output_layout(
    mode: DownloadMode,
    root: &Path,
    subject: &str,
) -> AppResult<OutputLayout> {
    let root = root.to_path_buf();
    let context_dir = match mode {
        DownloadMode::Illust => root.join(validate_controlled_segment("illustId", subject)?),
        DownloadMode::User => root.join(validate_controlled_segment("userId", subject)?),
        DownloadMode::Bookmark => root.join(validate_controlled_segment("userId", subject)?),
        DownloadMode::Keyword => root.join(normalize_keyword_segment(subject)),
        DownloadMode::Ranking => root.join(validate_controlled_segment("ranking mode", subject)?),
    };

    Ok(OutputLayout {
        mode,
        root,
        context_dir,
    })
}

fn validate_controlled_segment<'a>(label: &str, value: &'a str) -> AppResult<&'a str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CrawlerError::InvalidInput(format!("{label} 不能为空")).into());
    }

    Ok(trimmed)
}

fn normalize_keyword_segment(query: &str) -> String {
    let original = query.trim();
    let hash = short_hash(original);

    let mut normalized = String::new();
    let mut previous_was_space = false;
    let mut changed = false;

    for ch in original.chars() {
        let mapped = if ch.is_control() || is_reserved_path_char(ch) {
            changed = true;
            '_'
        } else {
            ch
        };

        if mapped.is_whitespace() {
            if normalized.is_empty() || previous_was_space {
                if !normalized.is_empty() {
                    changed = true;
                }
                previous_was_space = true;
                continue;
            }

            normalized.push(' ');
            previous_was_space = true;
            if mapped != ' ' {
                changed = true;
            }
            continue;
        }

        previous_was_space = false;
        normalized.push(mapped);
    }

    let trimmed = normalized.trim().trim_end_matches('.').trim().to_string();
    if trimmed != original {
        changed = true;
    }

    if trimmed.is_empty() || trimmed.chars().all(|ch| ch == '_' || ch == ' ') {
        return format!("keyword-{hash}");
    }

    let mut base = trimmed;
    let truncated = truncate_to_boundary(&base, KEYWORD_SEGMENT_MAX_LEN);
    if truncated.len() != base.len() {
        changed = true;
        base = truncated;
    }

    if changed {
        let limit = KEYWORD_SEGMENT_MAX_LEN.saturating_sub(HASH_SUFFIX_LEN);
        let shortened = truncate_to_boundary(&base, limit);
        return format!("{shortened}-{hash}");
    }

    base
}

fn truncate_to_boundary(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }

    value.chars().take(max_len).collect()
}

fn is_reserved_path_char(ch: char) -> bool {
    matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
}

fn short_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("{:08x}", (hash & 0xffff_ffff) as u32)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::config::DownloadMode;

    use super::{normalize_keyword_segment, resolve_output_layout};

    #[test]
    fn illust_layout_points_directly_to_illust_directory() {
        let layout = resolve_output_layout(
            DownloadMode::Illust,
            Path::new("/tmp/illust-root"),
            "123456",
        )
        .unwrap();

        assert_eq!(layout.context_dir(), Path::new("/tmp/illust-root/123456"));
        assert_eq!(
            layout.illust_dir("123456").unwrap(),
            Path::new("/tmp/illust-root/123456")
        );
    }

    #[test]
    fn user_layout_nests_illust_under_user_directory() {
        let layout =
            resolve_output_layout(DownloadMode::User, Path::new("/tmp/user-root"), "998877")
                .unwrap();

        assert_eq!(layout.context_dir(), Path::new("/tmp/user-root/998877"));
        assert_eq!(
            layout.illust_dir("123456").unwrap(),
            Path::new("/tmp/user-root/998877/123456")
        );
    }

    #[test]
    fn bookmark_layout_uses_self_user_id_as_context() {
        let layout = resolve_output_layout(
            DownloadMode::Bookmark,
            Path::new("/tmp/bookmark-root"),
            "12345678",
        )
        .unwrap();

        assert_eq!(
            layout.context_dir(),
            Path::new("/tmp/bookmark-root/12345678")
        );
        assert_eq!(
            layout.illust_dir("87654321").unwrap(),
            Path::new("/tmp/bookmark-root/12345678/87654321")
        );
    }

    #[test]
    fn ranking_layout_uses_mode_as_context() {
        let layout = resolve_output_layout(
            DownloadMode::Ranking,
            Path::new("/tmp/ranking-root"),
            "daily",
        )
        .unwrap();

        assert_eq!(layout.context_dir(), Path::new("/tmp/ranking-root/daily"));
        assert_eq!(
            layout.illust_dir("123456").unwrap(),
            Path::new("/tmp/ranking-root/daily/123456")
        );
    }

    #[test]
    fn keyword_segment_is_normalized_and_disambiguated() {
        let segment = normalize_keyword_segment("  初音/ミク  ");

        assert!(segment.starts_with("初音_ミク-"));
        assert!(segment.len() <= 80);
        assert!(!segment.contains('/'));
    }

    #[test]
    fn keyword_segment_uses_fallback_when_empty_after_normalization() {
        let segment = normalize_keyword_segment("///");

        assert!(segment.starts_with("keyword-"));
        assert_eq!(segment.len(), "keyword-".len() + 8);
    }

    #[test]
    fn keyword_layout_uses_normalized_segment_as_context() {
        let layout = resolve_output_layout(
            DownloadMode::Keyword,
            Path::new("/tmp/keyword-root"),
            "  初音/ミク  ",
        )
        .unwrap();

        let context = layout.context_dir().to_string_lossy();
        assert!(context.starts_with("/tmp/keyword-root/初音_ミク-"));
    }
}
