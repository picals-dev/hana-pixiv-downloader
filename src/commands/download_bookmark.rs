//! `picals-crawler download bookmark` 命令。

use crate::{
    cli::download::BookmarkArgs,
    commands::download_common::{
        build_replay_command, finalize_download_result, load_required_credential,
        print_download_summary, resolve_layout, resolve_options,
    },
    config::DownloadMode,
    crawler::bookmark::BookmarkCrawler,
    error::AppResult,
};

pub async fn run(args: BookmarkArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Bookmark, &args.to_overrides())?;
    let credential = load_required_credential()?;
    let user_id = credential.require_user_id()?.to_string();
    let layout = resolve_layout(&options, &user_id)?;
    let target_directory = layout.context_dir().to_path_buf();

    if options.dry_run {
        println!("将下载当前账号的收藏作品（dry-run）");
        println!("下载目录: {}", target_directory.display());
        println!("下载数量: {}", options.count);
        println!("并发下载数: {}", options.concurrent);
        println!("当前账号 userId: {user_id}");
        return Ok(());
    }

    let crawler = BookmarkCrawler::new(user_id, credential, options)?;
    let result = crawler.run().await?;
    let result = finalize_download_result(
        &crawler.context.credential,
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
