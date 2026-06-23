//! 失败清单回放执行。

use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use crate::{
    auth::Credential,
    downloader::image::DownloadItem,
    error::AppResult,
    failure::{FailureRecord, FailureStage, ReplayCommand},
    net::{PixivNetSession, resolve_base_url, test_hook::attach_session_observer},
    output::resolve_output_layout,
    pixiv::selector::select_page_original_urls,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReplayExecutionReport {
    pub attempted: usize,
    pub recovered: usize,
    pub skipped_non_retryable: usize,
    pub remaining_records: Vec<FailureRecord>,
}

pub async fn replay_failures(
    credential: &Credential,
    command: &ReplayCommand,
    records: Vec<FailureRecord>,
) -> AppResult<ReplayExecutionReport> {
    let options = command.options().to_resolved(command.mode());
    let base_url = resolve_base_url(None)?;
    let builder = attach_session_observer(PixivNetSession::builder(
        options.clone(),
        credential.clone(),
        base_url,
    ));
    let session = Arc::new(builder.build()?);
    replay_failures_with_session(session, command, records).await
}

pub async fn replay_failures_with_session(
    session: Arc<PixivNetSession>,
    command: &ReplayCommand,
    records: Vec<FailureRecord>,
) -> AppResult<ReplayExecutionReport> {
    let options = command.options().to_resolved(command.mode());
    let layout = resolve_output_layout(command.mode(), &options.directory, command.subject())?;

    let mut report = ReplayExecutionReport::default();
    let mut tag_record_indices = Vec::new();

    for (index, record) in records.iter().cloned().enumerate() {
        if !record.retryable {
            report.skipped_non_retryable += 1;
            report.remaining_records.push(record);
            continue;
        }

        match record.stage {
            FailureStage::Tags => {
                tag_record_indices.push(index);
            }
            FailureStage::Download => {
                report.attempted += 1;
                if replay_download_record(Arc::clone(&session), &options, record.clone()).await? {
                    report.recovered += 1;
                } else {
                    report.remaining_records.push(record);
                }
            }
            FailureStage::Collect => {
                report.attempted += 1;
                match replay_collect_record(
                    Arc::clone(&session),
                    &layout,
                    options.clone(),
                    record.clone(),
                )
                .await?
                {
                    Some(failures) if failures.is_empty() => report.recovered += 1,
                    Some(failures) => report.remaining_records.extend(failures),
                    None => report.remaining_records.push(record),
                }
            }
        }
    }

    if !tag_record_indices.is_empty() {
        let tag_records = tag_record_indices
            .into_iter()
            .map(|index| records[index].clone())
            .collect::<Vec<_>>();
        let tag_report =
            replay_tag_records(Arc::clone(&session), &layout, &options, tag_records).await?;
        report.attempted += tag_report.attempted;
        report.recovered += tag_report.recovered;
        report.skipped_non_retryable += tag_report.skipped_non_retryable;
        report
            .remaining_records
            .extend(tag_report.remaining_records);
    }

    Ok(report)
}

async fn replay_download_record(
    session: Arc<PixivNetSession>,
    options: &crate::config::ResolvedDownloadOptions,
    record: FailureRecord,
) -> AppResult<bool> {
    let Some(image_url) = record.image_url.clone() else {
        return Ok(false);
    };
    let Some(illust_id) = record.illust_id.clone() else {
        return Ok(false);
    };
    let Some(target_path) = record.target_path.clone() else {
        return Ok(false);
    };
    let target_dir = PathBuf::from(&target_path)
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let item = DownloadItem {
        illust_id,
        image_url,
        target_dir,
    };

    let result = crate::crawler::shared::download_items(
        options.clone(),
        options.directory.clone(),
        session,
        &[item],
    )
    .await?;

    Ok(result.failure_records.is_empty())
}

async fn replay_collect_record(
    session: Arc<PixivNetSession>,
    layout: &crate::output::OutputLayout,
    options: crate::config::ResolvedDownloadOptions,
    record: FailureRecord,
) -> AppResult<Option<Vec<FailureRecord>>> {
    let Some(illust_id) = record.illust_id.clone() else {
        return Ok(None);
    };

    let pages = session.fetch_illust_pages(&illust_id).await?;
    let image_urls = select_page_original_urls(&pages)?;
    let target_dir = layout.illust_dir(&illust_id)?;
    let items = image_urls
        .into_iter()
        .map(|image_url| DownloadItem {
            illust_id: illust_id.clone(),
            image_url,
            target_dir: target_dir.clone(),
        })
        .collect::<Vec<_>>();

    let result = crate::crawler::shared::download_items(
        options.clone(),
        layout.context_dir().to_path_buf(),
        session,
        &items,
    )
    .await?;

    Ok(Some(result.failure_records))
}

async fn replay_tag_records(
    session: Arc<PixivNetSession>,
    layout: &crate::output::OutputLayout,
    options: &crate::config::ResolvedDownloadOptions,
    records: Vec<FailureRecord>,
) -> AppResult<ReplayExecutionReport> {
    let mut report = ReplayExecutionReport::default();
    let mut illust_ids = BTreeSet::new();

    for record in &records {
        if let Some(illust_id) = record.illust_id.as_deref() {
            illust_ids.insert(illust_id.to_string());
        } else {
            report.remaining_records.push(record.clone());
        }
    }

    if illust_ids.is_empty() {
        report.skipped_non_retryable += report.remaining_records.len();
        return Ok(report);
    }

    report.attempted += illust_ids.len();
    let failures = crate::crawler::shared::export_tags_json(
        &session,
        &illust_ids.into_iter().collect::<Vec<_>>(),
        layout.context_dir(),
        options,
    )
    .await;

    if failures.is_empty() {
        report.recovered += report.attempted;
    } else {
        report.recovered += report.attempted.saturating_sub(failures.len());
        report.remaining_records.extend(failures);
    }

    Ok(report)
}
