//! `picals-crawler download ranking` 命令。

use crate::{
    cli::download::RankingArgs,
    commands::download_common::{
        build_replay_command, ensure_ranking_defaults, finalize_download_result,
        load_required_credential, print_download_summary, resolve_layout, resolve_options,
    },
    config::DownloadMode,
    crawler::ranking::RankingCrawler,
    error::AppResult,
};

pub async fn run(args: RankingArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Ranking, &args.to_overrides())?;
    ensure_ranking_defaults(&options)?;
    let mode = args.mode.as_api_mode().to_string();
    let layout = resolve_layout(&options, &mode)?;
    let target_directory = layout.context_dir().to_path_buf();

    if options.dry_run {
        println!("将下载排行榜 {} 的作品（dry-run）", mode);
        println!("下载目录: {}", target_directory.display());
        println!("下载数量: {}", options.count);
        println!("并发下载数: {}", options.concurrent);
        return Ok(());
    }

    let credential = load_required_credential()?;
    let crawler = RankingCrawler::new(mode, credential, options)?;
    let result = crawler.run().await?;
    let result = finalize_download_result(
        &crawler.context.credential,
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
