//! `hpd download user` 命令。

use std::sync::Arc;

use crate::{
    cli::download::UserArgs,
    commands::download_common::{
        build_replay_command, confirm_bulk_plan, create_shared_session, finalize_download_result,
        load_required_credential, print_download_summary, probe_user_count, resolve_layout,
        resolve_options,
    },
    config::DownloadMode,
    crawler::user::UserCrawler,
    error::AppResult,
    pixiv::url::extract_user_id,
};

pub(crate) async fn run(args: UserArgs) -> AppResult<()> {
    let artist_id = extract_user_id(&args.target)?;
    let options = resolve_options(DownloadMode::User, &args.common.to_overrides())?;
    let layout = resolve_layout(&options, &artist_id)?;
    let target_directory = layout.context_dir().to_path_buf();
    let credential = load_required_credential()?;
    let session = create_shared_session(&options, &credential)?;
    let probe = probe_user_count(&session, &artist_id).await?;
    let Some(options) =
        confirm_bulk_plan(options, &probe, "画师下载", &artist_id, &target_directory)?
    else {
        return Ok(());
    };

    let crawler = UserCrawler::new(artist_id, options, Arc::clone(&session));
    let result = crawler.run().await?;
    let result = finalize_download_result(
        session,
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
