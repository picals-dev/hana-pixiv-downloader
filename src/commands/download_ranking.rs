//! `picals-crawler download ranking` 命令。

use crate::{
    cli::download::RankingArgs,
    commands::download_common::{
        DownloadPresentation, build_replay_command, create_shared_session, ensure_ranking_defaults,
        finalize_download_result, load_required_credential, print_bulk_probe_summary,
        print_download_config_table, print_download_summary, probe_ranking_count,
        render_order_label, resolve_layout, resolve_options, resolve_planned_count,
        resolve_ranking_mode,
    },
    config::DownloadMode,
    crawler::ranking::RankingCrawler,
    error::AppResult,
};
use std::sync::Arc;

pub async fn run(args: RankingArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Ranking, &args.to_overrides())?;
    ensure_ranking_defaults(&options)?;
    let mode = resolve_ranking_mode(args.mode)?;
    let layout = resolve_layout(&options, &mode)?;
    let target_directory = layout.context_dir().to_path_buf();
    let credential = load_required_credential()?;
    let session = create_shared_session(&options, &credential)?;
    let probe = probe_ranking_count(&session, &mode).await?;
    print_bulk_probe_summary(&probe);
    let planned_count = resolve_planned_count(&options, probe.candidate_count)?;
    let mut options = options;
    options.count = planned_count;
    print_download_config_table(
        &DownloadPresentation {
            mode_label: "排行榜下载".to_string(),
            subject_label: mode.clone(),
            candidate_count: Some(probe.candidate_count),
            planned_count: Some(planned_count),
            order_label: render_order_label(DownloadMode::Ranking, options.sort),
        },
        &options,
        &target_directory,
    );

    if options.dry_run {
        return Ok(());
    }

    let crawler = RankingCrawler::new(mode, options, Arc::clone(&session))?;
    let result = crawler.run().await?;
    let result = finalize_download_result(
        session,
        build_replay_command(
            DownloadMode::Ranking,
            &crawler.context.options,
            &crawler.mode,
            None,
        ),
        result,
    )
    .await?;
    print_download_summary(&target_directory, &result);
    Ok(())
}
