//! `picals-crawler download ranking` 命令。

use crate::{
    cli::download::RankingArgs,
    commands::download_common::{
        ensure_ranking_defaults, load_required_credential, print_download_summary, resolve_options,
    },
    crawler::ranking::RankingCrawler,
    error::AppResult,
};

pub async fn run(args: RankingArgs) -> AppResult<()> {
    let options = resolve_options(&args.to_overrides())?;
    ensure_ranking_defaults(&options)?;
    let mode = args.mode.as_api_mode().to_string();
    let target_directory = options.directory.join(format!("ranking-{mode}"));

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
    print_download_summary(&target_directory, &result);
    Ok(())
}
