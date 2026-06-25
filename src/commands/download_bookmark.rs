//! `picals-crawler download bookmark` 命令。

use crate::{
    cli::download::BookmarkArgs,
    commands::download_common::{
        build_replay_command, confirm_bulk_plan, create_shared_session, finalize_download_result,
        load_required_credential, print_download_summary, probe_bookmark_count, resolve_layout,
        resolve_options,
    },
    config::DownloadMode,
    crawler::bookmark::BookmarkCrawler,
    error::AppResult,
};
use std::sync::Arc;

pub(crate) async fn run(args: BookmarkArgs) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Bookmark, &args.to_overrides())?;
    let credential = load_required_credential()?;
    let user_id = credential.require_user_id()?.to_string();
    let layout = resolve_layout(&options, &user_id)?;
    let target_directory = layout.context_dir().to_path_buf();
    let session = create_shared_session(&options, &credential)?;
    let probe = probe_bookmark_count(&session, &user_id).await?;
    let subject_label = format!("当前账号 {user_id}");
    let Some(options) = confirm_bulk_plan(
        options,
        &probe,
        "收藏下载",
        &subject_label,
        &target_directory,
    )?
    else {
        return Ok(());
    };

    let crawler = BookmarkCrawler::new(user_id, options, Arc::clone(&session));
    let result = crawler.run().await?;
    let result = finalize_download_result(
        session,
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
