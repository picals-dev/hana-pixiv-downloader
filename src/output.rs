//! 下载输出路径解析。

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::{
    config::{BatchLayoutStrategy, DownloadMode},
    error::{AppResult, CrawlerError},
};

const KEYWORD_SEGMENT_MAX_LEN: usize = 80;
const HASH_SUFFIX_LEN: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputLayout {
    mode: DownloadMode,
    context_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtworkInventoryEntry {
    pub file_name: String,
    pub source_path: Option<PathBuf>,
}

impl ArtworkInventoryEntry {
    pub(crate) fn planned(file_name: impl Into<String>) -> Self {
        Self {
            file_name: file_name.into(),
            source_path: None,
        }
    }

    pub(crate) fn existing(file_name: impl Into<String>, source_path: PathBuf) -> Self {
        Self {
            file_name: file_name.into(),
            source_path: Some(source_path),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtworkInventory {
    pub illust_id: String,
    pub output_count: usize,
    pub entries: Vec<ArtworkInventoryEntry>,
}

impl ArtworkInventory {
    pub(crate) fn new(
        illust_id: impl Into<String>,
        entries: Vec<ArtworkInventoryEntry>,
    ) -> AppResult<Self> {
        let illust_id = validate_controlled_segment("illustId", &illust_id.into())?.to_string();
        let output_count = entries
            .iter()
            .map(|entry| entry.file_name.clone())
            .collect::<BTreeSet<_>>()
            .len();

        Ok(Self {
            illust_id,
            output_count,
            entries,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArtworkPlacement {
    pub illust_id: String,
    pub output_count: usize,
    pub context_dir: PathBuf,
    pub target_dir: PathBuf,
    pub batch_layout: BatchLayoutStrategy,
}

impl ArtworkPlacement {
    pub(crate) fn target_path(&self, file_name: &str) -> PathBuf {
        self.target_dir.join(file_name)
    }
}

impl OutputLayout {
    pub(crate) fn context_dir(&self) -> &Path {
        &self.context_dir
    }

    pub(crate) fn from_context_dir(mode: DownloadMode, context_dir: PathBuf) -> Self {
        Self { mode, context_dir }
    }

    pub(crate) fn placement_for_inventory(
        &self,
        batch_layout: BatchLayoutStrategy,
        inventory: &ArtworkInventory,
    ) -> AppResult<ArtworkPlacement> {
        let illust_id = validate_controlled_segment("illustId", &inventory.illust_id)?.to_string();
        let target_dir = if !self.mode.is_batch() {
            self.context_dir.clone()
        } else {
            match batch_layout {
                BatchLayoutStrategy::PerIllust => self.context_dir.join(&illust_id),
                BatchLayoutStrategy::Mixed if inventory.output_count <= 1 => {
                    self.context_dir.clone()
                }
                BatchLayoutStrategy::Mixed => self.context_dir.join(&illust_id),
                BatchLayoutStrategy::Flat => self.context_dir.clone(),
            }
        };

        Ok(ArtworkPlacement {
            illust_id,
            output_count: inventory.output_count,
            context_dir: self.context_dir.clone(),
            target_dir,
            batch_layout,
        })
    }
}

pub(crate) fn resolve_output_layout(
    mode: DownloadMode,
    root: &Path,
    subject: &str,
) -> AppResult<OutputLayout> {
    let context_dir = match mode {
        DownloadMode::Illust => root.join(validate_controlled_segment("illustId", subject)?),
        DownloadMode::User => root.join(validate_controlled_segment("userId", subject)?),
        DownloadMode::Bookmark => root.join(validate_controlled_segment("userId", subject)?),
        DownloadMode::Keyword => root.join(normalize_keyword_segment(subject)),
        DownloadMode::Ranking => root.join(validate_controlled_segment("ranking mode", subject)?),
    };

    Ok(OutputLayout { mode, context_dir })
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

    use crate::config::{BatchLayoutStrategy, DownloadMode};

    use super::{
        ArtworkInventory, ArtworkInventoryEntry, normalize_keyword_segment, resolve_output_layout,
    };

    #[test]
    fn illust_layout_always_uses_context_dir_regardless_of_batch_layout() {
        let layout = resolve_output_layout(
            DownloadMode::Illust,
            Path::new("/tmp/illust-root"),
            "123456",
        )
        .unwrap();
        let inventory = ArtworkInventory::new(
            "123456",
            vec![ArtworkInventoryEntry::planned("123456_p0.png")],
        )
        .unwrap();

        assert_eq!(layout.context_dir(), Path::new("/tmp/illust-root/123456"));
        assert_eq!(
            layout
                .placement_for_inventory(BatchLayoutStrategy::Flat, &inventory)
                .unwrap()
                .target_dir,
            Path::new("/tmp/illust-root/123456")
        );
    }

    #[test]
    fn per_illust_layout_nests_everything_under_illust_directory() {
        let layout =
            resolve_output_layout(DownloadMode::User, Path::new("/tmp/user-root"), "998877")
                .unwrap();
        let inventory = ArtworkInventory::new(
            "123456",
            vec![ArtworkInventoryEntry::planned("123456_p0.png")],
        )
        .unwrap();

        assert_eq!(layout.context_dir(), Path::new("/tmp/user-root/998877"));
        assert_eq!(
            layout
                .placement_for_inventory(BatchLayoutStrategy::PerIllust, &inventory)
                .unwrap()
                .target_dir,
            Path::new("/tmp/user-root/998877/123456")
        );
    }

    #[test]
    fn flat_layout_keeps_batch_outputs_in_context_root() {
        let layout =
            resolve_output_layout(DownloadMode::User, Path::new("/tmp/user-root"), "998877")
                .unwrap();
        let inventory = ArtworkInventory::new(
            "123456",
            vec![ArtworkInventoryEntry::planned("123456_p0.png")],
        )
        .unwrap();

        assert_eq!(
            layout
                .placement_for_inventory(BatchLayoutStrategy::Flat, &inventory)
                .unwrap()
                .target_dir,
            Path::new("/tmp/user-root/998877")
        );
    }

    #[test]
    fn mixed_layout_keeps_single_output_in_context_root() {
        let layout = resolve_output_layout(
            DownloadMode::Bookmark,
            Path::new("/tmp/bookmark-root"),
            "12345678",
        )
        .unwrap();
        let inventory = ArtworkInventory::new(
            "87654321",
            vec![ArtworkInventoryEntry::planned("87654321.gif")],
        )
        .unwrap();

        assert_eq!(
            layout
                .placement_for_inventory(BatchLayoutStrategy::Mixed, &inventory)
                .unwrap()
                .target_dir,
            Path::new("/tmp/bookmark-root/12345678")
        );
    }

    #[test]
    fn mixed_layout_puts_multi_output_artwork_in_illust_directory() {
        let layout = resolve_output_layout(
            DownloadMode::Ranking,
            Path::new("/tmp/ranking-root"),
            "daily",
        )
        .unwrap();
        let inventory = ArtworkInventory::new(
            "123456",
            vec![
                ArtworkInventoryEntry::planned("123456_p0.png"),
                ArtworkInventoryEntry::planned("123456_p1.png"),
            ],
        )
        .unwrap();

        assert_eq!(
            layout
                .placement_for_inventory(BatchLayoutStrategy::Mixed, &inventory)
                .unwrap()
                .target_dir,
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

        assert_eq!(
            layout.context_dir().parent(),
            Some(Path::new("/tmp/keyword-root"))
        );

        let context_name = layout
            .context_dir()
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap();
        assert!(context_name.starts_with("初音_ミク-"));
    }
}
