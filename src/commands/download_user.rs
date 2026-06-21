//! `picals-crawler download user` 命令。

use crate::{
    auth::Credential,
    cli::download::UserArgs,
    commands::download_common::{
        build_replay_command, finalize_download_result, print_download_summary, resolve_layout,
    },
    config::{Config, DownloadMode, EnvOverrides, SortOrder},
    crawler::user::UserCrawler,
    error::{AppResult, CrawlerError},
    utils::url::extract_user_id,
};

pub async fn run(args: UserArgs) -> AppResult<()> {
    let artist_id = extract_user_id(&args.target)?;
    let config = Config::load()?;
    let env = EnvOverrides::from_process_env()?;
    let options =
        config.resolve_download_options(DownloadMode::User, &env, &args.common.to_overrides())?;
    ensure_sort_supported(options.sort)?;
    let layout = resolve_layout(&options, &artist_id)?;
    let target_directory = layout.context_dir().to_path_buf();
    let credential = Credential::load()?;

    if options.dry_run {
        println!("将下载画师 {} 的作品（dry-run）", artist_id);
        println!("下载目录: {}", target_directory.display());
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
    let crawler = UserCrawler::new(artist_id, credential, options)?;
    let result = crawler.run().await?;
    let result = finalize_download_result(
        &crawler.context.credential,
        build_replay_command(
            DownloadMode::User,
            &crawler.context.options,
            &crawler.artist_id,
            None,
        ),
        result,
    )
    .await?;
    print_download_summary(&target_directory, &result);

    Ok(())
}

fn ensure_sort_supported(sort: SortOrder) -> AppResult<()> {
    if sort == SortOrder::PopularDesc {
        return Err(CrawlerError::InvalidInput(
            "download user 在当前版本暂不支持 --sort popular_desc".to_string(),
        )
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::SortOrder;

    use super::ensure_sort_supported;

    #[test]
    fn user_download_rejects_popular_sort() {
        let error = ensure_sort_supported(SortOrder::PopularDesc).unwrap_err();
        assert!(format!("{error:#}").contains("暂不支持 --sort popular_desc"));
    }

    #[test]
    fn user_download_accepts_date_sorts() {
        ensure_sort_supported(SortOrder::DateDesc).unwrap();
        ensure_sort_supported(SortOrder::DateAsc).unwrap();
    }
}
