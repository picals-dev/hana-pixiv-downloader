//! `picals-crawler download bookmark` 命令。

use crate::{
    cli::download::BookmarkArgs,
    commands::download_common::{
        DownloadPresentation, build_replay_command, create_shared_session,
        finalize_download_result, load_required_credential, print_bulk_probe_summary,
        print_download_config_table, print_download_summary, probe_bookmark_count,
        render_order_label, resolve_layout, resolve_options, resolve_planned_count,
    },
    config::DownloadMode,
    crawler::bookmark::BookmarkCrawler,
    error::AppResult,
};
use std::sync::Arc;

pub async fn run(args: BookmarkArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Bookmark, &args.to_overrides())?;
    let credential = load_required_credential()?;
    let user_id = credential.require_user_id()?.to_string();
    let layout = resolve_layout(&options, &user_id)?;
    let target_directory = layout.context_dir().to_path_buf();
    let session = create_shared_session(&options, &credential)?;
    let probe = probe_bookmark_count(&session, &user_id).await?;
    print_bulk_probe_summary(&probe);
    let planned_count = resolve_planned_count(&options, probe.candidate_count)?;
    let mut options = options;
    options.count = planned_count;
    print_download_config_table(
        &DownloadPresentation {
            mode_label: "收藏下载".to_string(),
            subject_label: format!("当前账号 {}", user_id),
            candidate_count: Some(probe.candidate_count),
            planned_count: Some(planned_count),
            order_label: render_order_label(DownloadMode::Bookmark, options.sort),
        },
        &options,
        &target_directory,
    );

    if options.dry_run {
        return Ok(());
    }

    let crawler = BookmarkCrawler::new(user_id, options, Arc::clone(&session))?;
    let result = crawler.run().await?;
    let result = finalize_download_result(
        session,
        build_replay_command(
            DownloadMode::Bookmark,
            &crawler.context.options,
            &crawler.user_id,
            None,
        ),
        result,
    )
    .await?;
    print_download_summary(&target_directory, &result);
    Ok(())
}
