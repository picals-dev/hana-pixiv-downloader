//! `picals-crawler download user` 命令。

use crate::{
    auth::Credential,
    cli::download::UserArgs,
    collector::PixivCollector,
    commands::download_common::{
        DownloadPresentation, build_replay_command, finalize_download_result,
        print_bulk_probe_summary, print_download_config_table, print_download_summary,
        probe_user_count, render_order_label, resolve_layout, resolve_planned_count,
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
    let credential = credential.ok_or(CrawlerError::MissingCredential)?;
    let collector = PixivCollector::new(&options, &credential)?;
    let probe = probe_user_count(&collector, &artist_id).await?;
    print_bulk_probe_summary(&probe);
    let planned_count = resolve_planned_count(&options, probe.candidate_count)?;
    let mut options = options;
    options.count = planned_count;
    print_download_config_table(
        &DownloadPresentation {
            mode_label: "画师下载".to_string(),
            subject_label: artist_id.clone(),
            candidate_count: Some(probe.candidate_count),
            planned_count: Some(planned_count),
            order_label: render_order_label(DownloadMode::User, options.sort),
        },
        &options,
        &target_directory,
    );

    if options.dry_run {
        return Ok(());
    }

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
