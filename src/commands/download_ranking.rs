//! `hpd download ranking` 命令。

use crate::{
    cli::download::RankingArgs,
    commands::download_common::{
        build_replay_command, confirm_bulk_plan, create_shared_session, ensure_ranking_defaults,
        finalize_download_result, load_required_credential, print_download_summary,
        probe_ranking_count, resolve_layout, resolve_options, resolve_ranking_mode,
    },
    config::DownloadMode,
    crawler::ranking::RankingCrawler,
    error::AppResult,
};
use std::sync::Arc;

pub(crate) async fn run(args: RankingArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Ranking, &args.to_overrides())?;
    ensure_ranking_defaults(&options)?;
    let mode = resolve_ranking_mode(args.mode)?;
    let layout = resolve_layout(&options, &mode)?;
    let target_directory = layout.context_dir().to_path_buf();
    let credential = load_required_credential()?;
    let session = create_shared_session(&options, &credential)?;
    let probe = probe_ranking_count(&session, &mode).await?;
    let Some(options) = confirm_bulk_plan(options, &probe, "排行榜下载", &mode, &target_directory)?
    else {
        return Ok(());
    };

    let crawler = RankingCrawler::new(mode, options, Arc::clone(&session));
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
