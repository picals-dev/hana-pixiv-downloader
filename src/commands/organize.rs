//! `hpd organize` 命令。

use eyre::{Report, eyre};
use inquire::{Confirm, InquireError};

use crate::{
    cli::organize::OrganizeCommand,
    config::Config,
    error::AppResult,
    organize::{OrganizeExecutionReport, OrganizeRunPlan, build_run_plan, execute_run_plan},
};

pub(crate) async fn run(args: OrganizeCommand) -> AppResult<()> {
    let config = Config::load()?;
    let _ = run_with_config(&config, args.dry_run, args.yes)?;
    Ok(())
}

pub(crate) fn run_with_config(
    config: &Config,
    dry_run: bool,
    auto_confirm: bool,
) -> AppResult<OrganizeExecutionReport> {
    let plan = build_run_plan(config)?;
    let summary = plan.summary();

    println!(
        "当前批量目录布局: {}",
        config.download.batch_layout.display_name()
    );
    print_summary(&summary, dry_run);
    print_plan_preview(&plan);

    if dry_run {
        println!("预览模式不会执行任何移动或删除操作。");
        return Ok(summary);
    }

    if !plan.has_pending_moves() {
        println!("未发现需要移动的已识别 HPD 产物。");
        return Ok(summary);
    }

    if !auto_confirm {
        let confirmed = Confirm::new("确认按当前批量目录布局整理这些已识别文件？")
            .with_default(true)
            .prompt()
            .map_err(map_inquire_error)?;
        if !confirmed {
            return Err(eyre!("操作已取消"));
        }
    }

    let report = execute_run_plan(&plan)?;
    print_summary(&report, false);
    Ok(report)
}

fn print_plan_preview(plan: &OrganizeRunPlan) {
    for context in &plan.contexts {
        if context.move_plans.is_empty() && context.unknown_files.is_empty() {
            continue;
        }

        println!();
        println!("context: {}", context.context_dir.display());

        for move_plan in &context.move_plans {
            for planned_move in &move_plan.moves {
                println!(
                    "  MOVE {} -> {}",
                    planned_move.from.display(),
                    planned_move.to.display()
                );
            }
            for conflict in &move_plan.conflicts {
                println!(
                    "  CONFLICT {} -> {}",
                    conflict.from.display(),
                    conflict.to.display()
                );
            }
        }

        if !context.unknown_files.is_empty() {
            println!("  KEEP 未识别条目 {} 个", context.unknown_files.len());
        }
    }
}

fn print_summary(report: &OrganizeExecutionReport, dry_run: bool) {
    println!();
    if dry_run {
        println!("整理预览统计：");
    } else {
        println!("整理结果统计：");
    }
    println!("  扫描 context 数: {}", report.scanned_contexts);
    println!("  识别作品数: {}", report.recognized_artworks);
    println!("  计划/已移动文件数: {}", report.moved_files);
    println!("  跳过文件数: {}", report.skipped_files);
    println!("  未识别文件数: {}", report.unknown_files);
    println!("  冲突文件数: {}", report.conflicts);
    println!("  删除空目录数: {}", report.deleted_empty_dirs);
}

fn map_inquire_error(error: InquireError) -> Report {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            eyre!("操作已取消")
        }
        other => Report::new(other).wrap_err("交互式输入失败"),
    }
}
