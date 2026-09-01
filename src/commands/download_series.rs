//! `hpd download series` 命令。

use std::sync::Arc;

use eyre::WrapErr;

use crate::{
    cli::download::{CommonDownloadArgs, SeriesArgs},
    commands::download_common::{
        build_replay_command, confirm_bulk_plan, create_shared_session, finalize_download_result,
        load_required_credential, print_download_summary, resolve_layout, resolve_options,
    },
    config::DownloadMode,
    crawler::series::SeriesCrawler,
    error::AppResult,
    net::PixivNetSession,
    pixiv::url::extract_series_id,
};

pub(crate) async fn run(args: SeriesArgs) -> AppResult<()> {
    let (user_id, series_id) = extract_series_id(&args.target)?;
    run_target(series_id, user_id, args.common).await
}

pub(crate) async fn run_target(
    series_id: String,
    user_id: Option<String>,
    common: CommonDownloadArgs,
) -> AppResult<()> {
    let options = resolve_options(DownloadMode::Series, &common.to_overrides())?;
    let layout = resolve_layout(&options, &series_id)?;
    let target_directory = layout.context_dir().to_path_buf();
    let credential = load_required_credential()?;
    let session = create_shared_session(&options, &credential)?;

    let probe = probe_series_count(&session, &series_id, user_id.as_deref()).await?;
    let Some(options) =
        confirm_bulk_plan(options, &probe, "系列下载", &series_id, &target_directory)?
    else {
        return Ok(());
    };

    let crawler = SeriesCrawler::new(series_id, user_id, options, Arc::clone(&session));
    let result = crawler.run().await?;
    let result = finalize_download_result(
        session,
        build_replay_command(
            DownloadMode::Series,
            &crawler.context.options,
            &crawler.series_id,
            None,
        ),
        result,
    )
    .await?;
    print_download_summary(&target_directory, &result);

    Ok(())
}

/// 拉取系列首页元信息，构造下载确认对话所需的探测摘要。
async fn probe_series_count(
    session: &Arc<PixivNetSession>,
    series_id: &str,
    user_id: Option<&str>,
) -> AppResult<crate::commands::download_common::BatchProbeSummary> {
    let value = session
        .fetch_series_page(series_id, user_id, 1)
        .await
        .wrap_err(format!(
            "获取系列 {series_id} 失败（系列不存在、已删除，或为不支持的小说系列）"
        ))?;
    let meta = crate::pixiv::selector::select_series_meta(&value)?;
    Ok(crate::commands::download_common::BatchProbeSummary {
        candidate_count: meta.total,
        count_source: "系列总话数（illustSeries.total）",
        subject_label: format!("系列 《{}》", meta.title),
    })
}
