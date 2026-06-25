//! `hpd download keyword` 命令。

use crate::{
    cli::download::KeywordArgs,
    commands::download_common::{
        build_replay_command, confirm_bulk_plan, create_shared_session, finalize_download_result,
        load_required_credential, print_download_summary, probe_keyword_count, resolve_layout,
        resolve_options,
    },
    config::DownloadMode,
    crawler::keyword::{KeywordCrawler, KeywordMode},
    error::AppResult,
};
use std::sync::Arc;

pub(crate) async fn run(args: KeywordArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Keyword, &args.to_overrides())?;
    let layout = resolve_layout(&options, &args.query)?;
    let target_directory = layout.context_dir().to_path_buf();
    let credential = load_required_credential()?;
    let session = create_shared_session(&options, &credential)?;
    let keyword_mode = if args.r18 { "r18" } else { "safe" };
    let order = match options.sort {
        crate::config::SortOrder::DateDesc => "date_d",
        crate::config::SortOrder::DateAsc => "date",
    };
    let probe = probe_keyword_count(&session, &args.query, order, keyword_mode, options.ai).await?;
    let subject_label = format!("{} ({})", args.query, keyword_mode);
    let Some(options) = confirm_bulk_plan(
        options,
        &probe,
        "关键词下载",
        &subject_label,
        &target_directory,
    )?
    else {
        return Ok(());
    };

    let crawler = KeywordCrawler::new(
        args.query.clone(),
        if args.r18 {
            KeywordMode::R18
        } else {
            KeywordMode::Safe
        },
        options,
        Arc::clone(&session),
    );
    let result = crawler.run().await?;
    let result = finalize_download_result(
        session,
        build_replay_command(
            DownloadMode::Keyword,
            &crawler.context.options,
            &crawler.query,
            Some(if args.r18 { "r18" } else { "safe" }),
        ),
        result,
    )
    .await?;
    print_download_summary(&target_directory, &result);
    Ok(())
}
