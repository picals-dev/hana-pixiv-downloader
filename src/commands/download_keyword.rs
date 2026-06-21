//! `picals-crawler download keyword` 命令。

use crate::{
    cli::download::KeywordArgs,
    collector::PixivCollector,
    commands::download_common::{
        DownloadPresentation, build_replay_command, finalize_download_result,
        load_required_credential, print_bulk_probe_summary, print_download_config_table,
        print_download_summary, probe_keyword_count, render_order_label, resolve_layout,
        resolve_options, resolve_planned_count,
    },
    config::DownloadMode,
    crawler::keyword::{KeywordCrawler, KeywordMode},
    error::AppResult,
};

pub async fn run(args: KeywordArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Keyword, &args.to_overrides())?;
    let layout = resolve_layout(&options, &args.query)?;
    let target_directory = layout.context_dir().to_path_buf();
    let credential = load_required_credential()?;
    let collector = PixivCollector::new(&options, &credential)?;
    let keyword_mode = if args.r18 { "r18" } else { "safe" };
    let order = match options.sort {
        crate::config::SortOrder::DateDesc => "date_d",
        crate::config::SortOrder::DateAsc => "date",
        crate::config::SortOrder::PopularDesc => "popular_d",
    };
    let probe =
        probe_keyword_count(&collector, &args.query, order, keyword_mode, options.ai).await?;
    print_bulk_probe_summary(&probe);
    let planned_count = resolve_planned_count(&options, probe.candidate_count)?;
    let mut options = options;
    options.count = planned_count;
    print_download_config_table(
        &DownloadPresentation {
            mode_label: "关键词下载".to_string(),
            subject_label: format!("{} ({})", args.query, keyword_mode),
            candidate_count: Some(probe.candidate_count),
            planned_count: Some(planned_count),
            order_label: render_order_label(DownloadMode::Keyword, options.sort),
        },
        &options,
        &target_directory,
    );

    if options.dry_run {
        return Ok(());
    }

    let crawler = KeywordCrawler::new(
        args.query.clone(),
        if args.r18 {
            KeywordMode::R18
        } else {
            KeywordMode::Safe
        },
        credential,
        options,
    )?;
    let result = crawler.run().await?;
    let result = finalize_download_result(
        &crawler.context.credential,
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
