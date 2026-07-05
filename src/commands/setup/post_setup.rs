use inquire::Confirm;

use super::super::prompt_support::map_inquire_error;
use crate::{
    config::{Config, SortOrder},
    error::AppResult,
    organize::OrganizeExecutionReport,
};

pub(crate) fn maybe_run_post_setup_organize(previous: &Config, current: &Config) -> AppResult<()> {
    match evaluate_layout_change(previous, current) {
        LayoutChangeAction::None => Ok(()),
        LayoutChangeAction::OfferOrganizeNow => {
            println!();
            let confirmed =
                Confirm::new("批量目录布局已变化，是否立即整理当前 batch roots 下的已有目录？")
                    .with_default(false)
                    .prompt()
                    .map_err(map_inquire_error)?;
            if !confirmed {
                println!("你可以稍后手动运行 `hpd organize --dry-run` 或 `hpd organize --yes`。");
                return Ok(());
            }

            match crate::commands::organize::run_with_config(current, false, true) {
                Ok(report) => print_post_organize_result(&report),
                Err(error) => {
                    println!("立即整理失败：{error}");
                    println!("配置已经写入，你可以稍后运行 `hpd organize --yes` 继续整理。");
                }
            }
            Ok(())
        }
        LayoutChangeAction::ExplainCrossRootOnly => {
            println!();
            println!("本次修改了 batch roots。");
            println!("当前版本的 hpd organize 只支持当前 root 内原地整理，不负责跨 root 迁移。");
            println!("如需整理，请先确认最终 roots 后，再运行：");
            println!("  hpd organize --dry-run");
            Ok(())
        }
    }
}

pub(crate) fn print_setup_summary(phpsessid: &str, user_id: &str, config: &Config) {
    println!();
    println!("请确认以下配置摘要：");
    println!("  auth.phpsessid = {phpsessid}");
    println!("  auth.user_id = {user_id}");
    println!("  download.roots.illust = {}", config.download.roots.illust);
    println!("  download.roots.user = {}", config.download.roots.user);
    println!(
        "  download.roots.bookmark = {}",
        config.download.roots.bookmark
    );
    println!(
        "  download.roots.keyword = {}",
        config.download.roots.keyword
    );
    println!(
        "  download.roots.ranking = {}",
        config.download.roots.ranking
    );
    println!(
        "  download.batch_layout = {}",
        config.download.batch_layout.display_name()
    );
    println!("  download.count = {}", config.download.count);
    println!("  download.sort = {}", render_sort(config.download.sort));
    println!("  download.r18 = {}", config.download.r18);
    println!("  download.ai = {}", config.download.ai);
    println!("  download.concurrent = {}", config.download.concurrent);
    println!("  download.timeout = {}", config.download.timeout);
    println!("  download.retry = {}", config.download.retry);
    println!("  download.with_tags = {}", config.download.with_tags);
    println!("  proxy.url = {}", render_optional(&config.proxy.url));
}

pub(crate) fn print_setup_success_hint() {
    println!();
    println!("✅ 配置完成！你现在可以通过以下命令继续查看或修改：");
    println!("  hpd config show");
    println!("  hpd config set auth.phpsessid <PHPSESSID>");
    println!("  hpd config set auth.user_id <USER_ID>");
    println!("  hpd download user <画师ID>");
    println!("  hpd download <Pixiv URL>");
    println!("  hpd download bookmark");
    println!("查看完整帮助: hpd --help");
}

fn print_post_organize_result(report: &OrganizeExecutionReport) {
    println!(
        "已完成当前 batch roots 原地整理：移动 {}，跳过 {}，冲突 {}，未识别 {}。",
        report.moved_files, report.skipped_files, report.conflicts, report.unknown_files
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutChangeAction {
    None,
    OfferOrganizeNow,
    ExplainCrossRootOnly,
}

fn evaluate_layout_change(previous: &Config, current: &Config) -> LayoutChangeAction {
    if !batch_roots_unchanged(previous, current) {
        return LayoutChangeAction::ExplainCrossRootOnly;
    }

    if previous.download.batch_layout == current.download.batch_layout {
        return LayoutChangeAction::None;
    }

    LayoutChangeAction::OfferOrganizeNow
}

fn batch_roots_unchanged(previous: &Config, current: &Config) -> bool {
    previous.download.roots.user == current.download.roots.user
        && previous.download.roots.bookmark == current.download.roots.bookmark
        && previous.download.roots.keyword == current.download.roots.keyword
        && previous.download.roots.ranking == current.download.roots.ranking
}

fn render_sort(sort: SortOrder) -> &'static str {
    match sort {
        SortOrder::DateDesc => "date_desc",
        SortOrder::DateAsc => "date_asc",
    }
}

fn render_optional(value: &str) -> &str {
    if value.trim().is_empty() {
        "<未设置>"
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{BatchLayoutStrategy, Config, DownloadRootsConfig, SortOrder};

    use super::{LayoutChangeAction, evaluate_layout_change, render_sort};

    #[test]
    fn render_sort_uses_public_config_values() {
        assert_eq!(render_sort(SortOrder::DateDesc), "date_desc");
        assert_eq!(render_sort(SortOrder::DateAsc), "date_asc");
    }

    #[test]
    fn setup_layout_matrix_offers_organize_when_only_layout_changes() {
        let previous = Config::default();
        let mut current = previous.clone();
        current.download.batch_layout = BatchLayoutStrategy::Flat;

        assert_eq!(
            evaluate_layout_change(&previous, &current),
            LayoutChangeAction::OfferOrganizeNow
        );
    }

    #[test]
    fn setup_layout_matrix_explains_cross_root_when_batch_roots_change_only() {
        let previous = Config::default();
        let mut current = previous.clone();
        current.download.roots = DownloadRootsConfig {
            illust: previous.download.roots.illust.clone(),
            user: "/tmp/other-user-root".to_string(),
            bookmark: previous.download.roots.bookmark.clone(),
            keyword: previous.download.roots.keyword.clone(),
            ranking: previous.download.roots.ranking.clone(),
        };

        assert_eq!(
            evaluate_layout_change(&previous, &current),
            LayoutChangeAction::ExplainCrossRootOnly
        );
    }

    #[test]
    fn setup_layout_matrix_ignores_illust_root_for_batch_organize_prompt() {
        let previous = Config::default();
        let mut current = previous.clone();
        current.download.batch_layout = BatchLayoutStrategy::PerIllust;
        current.download.roots = DownloadRootsConfig {
            illust: "/tmp/other-illust-root".to_string(),
            user: previous.download.roots.user.clone(),
            bookmark: previous.download.roots.bookmark.clone(),
            keyword: previous.download.roots.keyword.clone(),
            ranking: previous.download.roots.ranking.clone(),
        };

        assert_eq!(
            evaluate_layout_change(&previous, &current),
            LayoutChangeAction::OfferOrganizeNow
        );
    }
}
