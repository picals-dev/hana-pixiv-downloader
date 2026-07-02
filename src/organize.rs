//! 批量下载目录整理引擎。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use eyre::Context;

use crate::{
    config::{BatchLayoutStrategy, Config, DownloadMode, DownloadRootsConfig, expand_home_dir},
    error::AppResult,
    output::{ArtworkInventory, ArtworkInventoryEntry, ArtworkPlacement, OutputLayout},
    utils::progress::OrganizeProgress,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct OrganizeExecutionReport {
    pub scanned_contexts: usize,
    pub recognized_artworks: usize,
    pub moved_files: usize,
    pub skipped_files: usize,
    pub unknown_files: usize,
    pub conflicts: usize,
    pub deleted_empty_dirs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct OrganizeRunPlan {
    pub contexts: Vec<ContextOrganizePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrganizeMovePlan {
    pub inventory: ArtworkInventory,
    pub placement: ArtworkPlacement,
    pub moves: Vec<OrganizeFileMove>,
    pub skipped_files: Vec<PathBuf>,
    pub conflicts: Vec<OrganizeFileConflict>,
    pub cleanup_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrganizeFileMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrganizeFileConflict {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextOrganizePlan {
    pub mode: DownloadMode,
    pub context_dir: PathBuf,
    pub move_plans: Vec<OrganizeMovePlan>,
    pub unknown_files: Vec<PathBuf>,
}

impl OrganizeRunPlan {
    pub(crate) fn summary(&self) -> OrganizeExecutionReport {
        let mut report = OrganizeExecutionReport {
            scanned_contexts: self.contexts.len(),
            ..OrganizeExecutionReport::default()
        };

        let mut cleanup_dirs = BTreeSet::new();
        for context in &self.contexts {
            report.unknown_files += context.unknown_files.len();
            report.recognized_artworks += context.move_plans.len();

            for move_plan in &context.move_plans {
                report.moved_files += move_plan.moves.len();
                report.skipped_files += move_plan.skipped_files.len();
                report.conflicts += move_plan.conflicts.len();
                cleanup_dirs.extend(move_plan.cleanup_dirs.iter().cloned());
            }
        }

        report.deleted_empty_dirs = cleanup_dirs.len();
        report
    }

    pub(crate) fn has_pending_moves(&self) -> bool {
        self.contexts
            .iter()
            .any(|context| context.move_plans.iter().any(|plan| !plan.moves.is_empty()))
    }
}

pub(crate) fn build_run_plan(config: &Config) -> AppResult<OrganizeRunPlan> {
    let mut contexts = Vec::new();
    for (mode, root) in batch_root_paths(&config.download.roots)? {
        if !root.exists() {
            continue;
        }

        let mut entries = fs::read_dir(&root)
            .with_context(|| format!("读取 batch root 失败: {}", root.display()))?
            .collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.path());

        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if !file_type.is_dir() {
                continue;
            }

            contexts.push(plan_context(mode, &path, config.download.batch_layout)?);
        }
    }

    Ok(OrganizeRunPlan { contexts })
}

pub(crate) fn execute_run_plan(plan: &OrganizeRunPlan) -> AppResult<OrganizeExecutionReport> {
    let mut report = plan.summary();
    let progress = OrganizeProgress::new(
        report.recognized_artworks as u64,
        report.unknown_files as u64,
    );
    let mut deleted_empty_dirs = 0usize;

    for context in &plan.contexts {
        for move_plan in &context.move_plans {
            for planned_move in &move_plan.moves {
                if let Some(parent) = planned_move.to.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("创建目标目录失败: {}", parent.display()))?;
                }
                fs::rename(&planned_move.from, &planned_move.to).with_context(|| {
                    format!(
                        "移动文件失败: {} -> {}",
                        planned_move.from.display(),
                        planned_move.to.display()
                    )
                })?;
            }

            progress.record_artwork(
                move_plan.moves.len() as u64,
                move_plan.skipped_files.len() as u64,
                move_plan.conflicts.len() as u64,
            );
        }

        let mut cleanup_dirs = context
            .move_plans
            .iter()
            .flat_map(|plan| plan.cleanup_dirs.iter().cloned())
            .collect::<Vec<_>>();
        cleanup_dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        cleanup_dirs.dedup();

        for cleanup_dir in cleanup_dirs {
            if is_directory_empty(&cleanup_dir)? {
                fs::remove_dir(&cleanup_dir)
                    .with_context(|| format!("删除空目录失败: {}", cleanup_dir.display()))?;
                deleted_empty_dirs += 1;
            }
        }
    }

    progress.finish_with_message("目录整理完成");
    report.deleted_empty_dirs = deleted_empty_dirs;
    Ok(report)
}

fn batch_root_paths(roots: &DownloadRootsConfig) -> AppResult<Vec<(DownloadMode, PathBuf)>> {
    Ok(vec![
        (DownloadMode::User, expand_home_dir(Path::new(&roots.user))?),
        (
            DownloadMode::Bookmark,
            expand_home_dir(Path::new(&roots.bookmark))?,
        ),
        (
            DownloadMode::Keyword,
            expand_home_dir(Path::new(&roots.keyword))?,
        ),
        (
            DownloadMode::Ranking,
            expand_home_dir(Path::new(&roots.ranking))?,
        ),
    ])
}

fn plan_context(
    mode: DownloadMode,
    context_dir: &Path,
    batch_layout: BatchLayoutStrategy,
) -> AppResult<ContextOrganizePlan> {
    let mut inventory_entries = BTreeMap::<String, Vec<ArtworkInventoryEntry>>::new();
    let mut unknown_files = Vec::new();
    let mut cleanup_candidates = BTreeSet::new();
    let layout = OutputLayout::from_context_dir(mode, context_dir.to_path_buf());

    let mut entries = fs::read_dir(context_dir)
        .with_context(|| format!("读取 context 目录失败: {}", context_dir.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            unknown_files.push(path);
            continue;
        }

        if file_type.is_file() {
            if path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value == "tags.json")
            {
                continue;
            }

            if let Some(illust_id) = detect_illust_id_from_file(&path) {
                let file_name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_string();
                inventory_entries
                    .entry(illust_id)
                    .or_default()
                    .push(ArtworkInventoryEntry::existing(file_name, path));
            } else {
                unknown_files.push(path);
            }
            continue;
        }

        if file_type.is_dir() {
            if is_numeric_dir_name(&path) {
                scan_illust_dir(
                    &path,
                    &mut inventory_entries,
                    &mut unknown_files,
                    &mut cleanup_candidates,
                )?;
            } else {
                unknown_files.push(path);
            }
        }
    }

    let mut move_plans = Vec::new();
    for (illust_id, entries) in inventory_entries {
        let inventory = ArtworkInventory::new(illust_id, entries)?;
        let placement = layout.placement_for_inventory(batch_layout, &inventory)?;
        move_plans.push(build_move_plan(
            context_dir,
            inventory,
            placement,
            &cleanup_candidates,
        ));
    }

    Ok(ContextOrganizePlan {
        mode,
        context_dir: context_dir.to_path_buf(),
        move_plans,
        unknown_files,
    })
}

fn scan_illust_dir(
    illust_dir: &Path,
    inventory_entries: &mut BTreeMap<String, Vec<ArtworkInventoryEntry>>,
    unknown_files: &mut Vec<PathBuf>,
    cleanup_candidates: &mut BTreeSet<PathBuf>,
) -> AppResult<()> {
    let mut entries = fs::read_dir(illust_dir)
        .with_context(|| format!("读取作品目录失败: {}", illust_dir.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    let mut has_recognized_file = false;
    let mut has_unknown_file = false;

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            unknown_files.push(path);
            has_unknown_file = true;
            continue;
        }

        if file_type.is_file() {
            if let Some(illust_id) = detect_illust_id_from_file(&path) {
                let file_name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_string();
                inventory_entries
                    .entry(illust_id)
                    .or_default()
                    .push(ArtworkInventoryEntry::existing(file_name, path));
                has_recognized_file = true;
            } else {
                unknown_files.push(path);
                has_unknown_file = true;
            }
            continue;
        }

        unknown_files.push(path);
        has_unknown_file = true;
    }

    if has_recognized_file && !has_unknown_file {
        cleanup_candidates.insert(illust_dir.to_path_buf());
    }

    Ok(())
}

#[cfg(all(test, unix))]
fn create_symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(all(test, windows))]
fn create_symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

fn build_move_plan(
    context_dir: &Path,
    inventory: ArtworkInventory,
    placement: ArtworkPlacement,
    cleanup_candidates: &BTreeSet<PathBuf>,
) -> OrganizeMovePlan {
    let mut moves = Vec::new();
    let mut skipped_files = Vec::new();
    let mut conflicts = Vec::new();
    let mut reserved_targets = BTreeSet::new();
    let mut cleanup_state = BTreeMap::<PathBuf, CleanupState>::new();

    for entry in &inventory.entries {
        let Some(source_path) = entry.source_path.as_ref() else {
            continue;
        };
        let target_path = placement.target_path(&entry.file_name);
        let source_parent = source_path.parent().map(Path::to_path_buf);

        let action = if *source_path == target_path {
            skipped_files.push(source_path.clone());
            CleanupAction::Blocked
        } else if target_path.exists() || !reserved_targets.insert(target_path.clone()) {
            conflicts.push(OrganizeFileConflict {
                from: source_path.clone(),
                to: target_path,
            });
            CleanupAction::Blocked
        } else {
            moves.push(OrganizeFileMove {
                from: source_path.clone(),
                to: target_path,
            });
            CleanupAction::Moved
        };

        if let Some(parent) =
            source_parent.filter(|path| path != context_dir && cleanup_candidates.contains(path))
        {
            cleanup_state.entry(parent).or_default().record(action);
        }
    }

    let cleanup_dirs = cleanup_state
        .into_iter()
        .filter_map(|(dir, state)| state.should_cleanup().then_some(dir))
        .collect();

    OrganizeMovePlan {
        inventory,
        placement,
        moves,
        skipped_files,
        conflicts,
        cleanup_dirs,
    }
}

fn detect_illust_id_from_file(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    parse_image_like_file_name(file_name).or_else(|| parse_ugoira_file_name(file_name))
}

fn parse_image_like_file_name(file_name: &str) -> Option<String> {
    let (stem, extension) = file_name.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }

    let (illust_id, page) = stem.split_once("_p")?;
    (is_numeric_text(illust_id) && is_numeric_text(page)).then(|| illust_id.to_string())
}

fn parse_ugoira_file_name(file_name: &str) -> Option<String> {
    let (stem, extension) = file_name.rsplit_once('.')?;
    (extension.eq_ignore_ascii_case("gif") && is_numeric_text(stem)).then(|| stem.to_string())
}

fn is_numeric_dir_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(is_numeric_text)
}

fn is_numeric_text(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit())
}

fn is_directory_empty(path: &Path) -> AppResult<bool> {
    if !path.exists() {
        return Ok(false);
    }

    Ok(fs::read_dir(path)?.next().is_none())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CleanupAction {
    Moved,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct CleanupState {
    moved: usize,
    blocked: usize,
}

impl CleanupState {
    fn record(&mut self, action: CleanupAction) {
        match action {
            CleanupAction::Moved => self.moved += 1,
            CleanupAction::Blocked => self.blocked += 1,
        }
    }

    fn should_cleanup(self) -> bool {
        self.moved > 0 && self.blocked == 0
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use tempfile::tempdir;

    use crate::{
        config::{BatchLayoutStrategy, DownloadMode},
        output::{ArtworkInventory, ArtworkInventoryEntry, OutputLayout},
    };

    use super::{
        build_move_plan, create_symlink_dir, detect_illust_id_from_file,
        parse_image_like_file_name, plan_context,
    };

    #[test]
    fn image_file_name_can_extract_illust_id() {
        assert_eq!(
            parse_image_like_file_name("123456_p0.png").as_deref(),
            Some("123456")
        );
    }

    #[test]
    fn gif_file_name_can_extract_illust_id() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("123456.gif");
        fs::write(&path, b"gif").unwrap();
        assert_eq!(detect_illust_id_from_file(&path).as_deref(), Some("123456"));
    }

    #[test]
    fn unknown_file_is_not_recognized() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("readme.txt");
        fs::write(&path, b"noop").unwrap();
        assert!(detect_illust_id_from_file(&path).is_none());
    }

    #[test]
    fn per_illust_to_mixed_single_output_moves_file_to_context_root() {
        let temp = tempdir().unwrap();
        let context_dir = temp.path().join("12345678");
        let source_dir = context_dir.join("987654");
        fs::create_dir_all(&source_dir).unwrap();
        let source_path = source_dir.join("987654_p0.png");
        fs::write(&source_path, b"ok").unwrap();

        let layout = OutputLayout::from_context_dir(DownloadMode::User, context_dir.clone());
        let inventory = ArtworkInventory::new(
            "987654",
            vec![ArtworkInventoryEntry::existing(
                "987654_p0.png",
                source_path.clone(),
            )],
        )
        .unwrap();
        let placement = layout
            .placement_for_inventory(BatchLayoutStrategy::Mixed, &inventory)
            .unwrap();

        let plan = build_move_plan(
            &context_dir,
            inventory,
            placement,
            &BTreeSet::from([source_dir.clone()]),
        );

        assert_eq!(plan.moves.len(), 1);
        assert_eq!(plan.moves[0].from, source_path);
        assert_eq!(plan.moves[0].to, context_dir.join("987654_p0.png"));
        assert_eq!(plan.cleanup_dirs, vec![source_dir]);
    }

    #[test]
    fn flat_to_per_illust_moves_file_into_artwork_directory() {
        let temp = tempdir().unwrap();
        let context_dir = temp.path().join("12345678");
        fs::create_dir_all(&context_dir).unwrap();
        let source_path = context_dir.join("987654_p0.png");
        fs::write(&source_path, b"ok").unwrap();

        let layout = OutputLayout::from_context_dir(DownloadMode::User, context_dir.clone());
        let inventory = ArtworkInventory::new(
            "987654",
            vec![ArtworkInventoryEntry::existing(
                "987654_p0.png",
                source_path.clone(),
            )],
        )
        .unwrap();
        let placement = layout
            .placement_for_inventory(BatchLayoutStrategy::PerIllust, &inventory)
            .unwrap();

        let plan = build_move_plan(&context_dir, inventory, placement, &BTreeSet::new());

        assert_eq!(plan.moves[0].to, context_dir.join("987654/987654_p0.png"));
    }

    #[test]
    fn conflict_is_reported_without_overwriting_target() {
        let temp = tempdir().unwrap();
        let context_dir = temp.path().join("12345678");
        let source_dir = context_dir.join("987654");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&context_dir).unwrap();
        let source_path = source_dir.join("987654_p0.png");
        let target_path = context_dir.join("987654_p0.png");
        fs::write(&source_path, b"old").unwrap();
        fs::write(&target_path, b"new").unwrap();

        let layout = OutputLayout::from_context_dir(DownloadMode::User, context_dir.clone());
        let inventory = ArtworkInventory::new(
            "987654",
            vec![ArtworkInventoryEntry::existing(
                "987654_p0.png",
                source_path.clone(),
            )],
        )
        .unwrap();
        let placement = layout
            .placement_for_inventory(BatchLayoutStrategy::Flat, &inventory)
            .unwrap();

        let plan = build_move_plan(
            &context_dir,
            inventory,
            placement,
            &BTreeSet::from([source_dir]),
        );

        assert!(plan.moves.is_empty());
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].from, source_path);
        assert_eq!(plan.conflicts[0].to, target_path);
    }

    #[test]
    fn symlinked_artwork_directory_is_treated_as_unknown_and_not_traversed() {
        let temp = tempdir().unwrap();
        let context_dir = temp.path().join("12345678");
        let outside_dir = temp.path().join("outside");
        let link_dir = context_dir.join("123456");
        fs::create_dir_all(&context_dir).unwrap();
        fs::create_dir_all(&outside_dir).unwrap();
        fs::write(outside_dir.join("123456_p0.png"), b"ok").unwrap();
        create_symlink_dir(&outside_dir, &link_dir).unwrap();

        let plan =
            plan_context(DownloadMode::User, &context_dir, BatchLayoutStrategy::Flat).unwrap();

        assert!(plan.move_plans.is_empty());
        assert_eq!(plan.unknown_files, vec![link_dir]);
    }
}
