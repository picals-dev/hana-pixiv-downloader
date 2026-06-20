//! `picals-crawler download illust` 命令。

use crate::{
    cli::download::IllustArgs,
    commands::download_common::{
        load_required_credential, print_download_summary, resolve_options,
    },
    crawler::illust::IllustCrawler,
    error::AppResult,
    utils::url::extract_illust_id,
};

pub async fn run(args: IllustArgs) -> AppResult<()> {
    let illust_id = extract_illust_id(&args.target)?;
    let options = resolve_options(&args.common.to_overrides())?;
    let target_directory = options.directory.join(&illust_id);

    if options.dry_run {
        println!("将下载作品 {} 的全部图片（dry-run）", illust_id);
        println!("下载目录: {}", target_directory.display());
        println!("下载数量: {}", options.count);
        println!("排序方式: {:?}", options.sort);
        println!("并发下载数: {}", options.concurrent);
        return Ok(());
    }

    let credential = load_required_credential()?;
    let crawler = IllustCrawler::new(illust_id, credential, options)?;
    let result = crawler.run().await?;
    print_download_summary(&target_directory, &result);
    Ok(())
}
