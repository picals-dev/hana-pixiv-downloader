//! `picals-crawler download bookmark` 命令。

use crate::{
    cli::download::BookmarkArgs,
    commands::download_common::{
        load_required_credential, print_download_summary, resolve_options,
    },
    crawler::bookmark::BookmarkCrawler,
    error::AppResult,
};

pub async fn run(args: BookmarkArgs) -> AppResult<()> {
    let options = resolve_options(&args.to_overrides())?;
    let target_directory = options.directory.join("bookmark");
    let credential = load_required_credential()?;
    let user_id = credential.require_user_id()?.to_string();

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
    print_download_summary(&target_directory, &result);
    Ok(())
}
