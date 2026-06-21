//! `picals-crawler download keyword` 命令。

use crate::{
    cli::download::KeywordArgs,
    commands::download_common::{
        build_replay_command, finalize_download_result, load_required_credential,
        print_download_summary, resolve_layout, resolve_options,
    },
    config::DownloadMode,
    crawler::keyword::{KeywordCrawler, KeywordMode},
    error::AppResult,
};

pub async fn run(args: KeywordArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Keyword, &args.to_overrides())?;
    let layout = resolve_layout(&options, &args.query)?;
    let target_directory = layout.context_dir().to_path_buf();

    if options.dry_run {
        println!(
            "将下载关键词 {} 的搜索结果（模式：{}，dry-run）",
            args.query,
            if args.r18 { "r18" } else { "safe" }
        );
        println!("下载目录: {}", target_directory.display());
        println!("下载数量: {}", options.count);
        println!("排序方式: {:?}", options.sort);
        println!("并发下载数: {}", options.concurrent);
        return Ok(());
    }

    let credential = load_required_credential()?;
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
