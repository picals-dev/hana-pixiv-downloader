//! `picals-crawler download illust` 命令。

use crate::{
    cli::download::IllustArgs,
    commands::download_common::{
        DownloadPresentation, build_replay_command, create_shared_session,
        finalize_download_result, load_required_credential, print_download_config_table,
        print_download_summary, render_order_label, resolve_layout, resolve_options,
    },
    config::DownloadMode,
    crawler::illust::IllustCrawler,
    error::AppResult,
    pixiv::url::extract_illust_id,
};
use std::sync::Arc;

pub async fn run(args: IllustArgs) -> AppResult<()> {
    let illust_id = extract_illust_id(&args.target)?;
    let options = resolve_options(DownloadMode::Illust, &args.common.to_overrides())?;
    let layout = resolve_layout(&options, &illust_id)?;
    let target_directory = layout.context_dir().to_path_buf();
    print_download_config_table(
        &DownloadPresentation {
            mode_label: "作品下载".to_string(),
            subject_label: illust_id.clone(),
            candidate_count: None,
            planned_count: None,
            order_label: render_order_label(DownloadMode::Illust, options.sort),
        },
        &options,
        &target_directory,
    );

    if options.dry_run {
        return Ok(());
    }

    let credential = load_required_credential()?;
    let session = create_shared_session(&options, &credential)?;
    let crawler = IllustCrawler::new(illust_id, options, Arc::clone(&session))?;
    let result = crawler.run().await?;
    let result = finalize_download_result(
        session,
        build_replay_command(
            DownloadMode::Illust,
            &crawler.context.options,
            &crawler.illust_id,
            None,
        ),
        result,
    )
    .await?;
    print_download_summary(&target_directory, &result);
    Ok(())
}
