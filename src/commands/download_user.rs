//! `picals-crawler download user` 命令。

use crate::{
    auth::Credential,
    cli::download::UserArgs,
    config::{Config, EnvOverrides},
    crawler::user::UserCrawler,
    error::{AppResult, CrawlerError},
    utils::url::extract_user_id,
};

pub async fn run(args: UserArgs) -> AppResult<()> {
    let artist_id = extract_user_id(&args.target)?;
    let config = Config::load()?;
    let env = EnvOverrides::from_process_env();
    let options = config.resolve_download_options(&env, &args.common.to_overrides())?;
    let credential = Credential::load()?;

    if options.dry_run {
        println!("将下载画师 {} 的作品（dry-run）", artist_id);
        println!("下载目录: {}", options.directory.display());
        println!("下载数量: {}", options.count);
        println!("排序方式: {:?}", options.sort);
        println!("并发下载数: {}", options.concurrent);
        println!(
            "认证状态: {}",
            if credential.is_some() {
                "已配置"
            } else {
                "未配置"
            }
        );
        return Ok(());
    }

    let credential = credential.ok_or(CrawlerError::MissingCredential)?;
    let crawler = UserCrawler::new(artist_id, credential, options);
    let result = crawler.run().await?;

    println!(
        "下载完成：总数 {}，成功 {}，跳过 {}，失败 {}",
        result.total, result.downloaded, result.skipped, result.failed
    );

    Ok(())
}
