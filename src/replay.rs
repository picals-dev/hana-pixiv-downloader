//! 失败清单回放执行。

use std::{collections::BTreeSet, path::Path, sync::Arc};

use crate::{
    auth::Credential,
    downloader::{ArtworkDownloadPlan, ImageArtworkPlan},
    error::AppResult,
    failure::{FailureRecord, FailureStage, ReplayCommand},
    net::{PixivNetSession, resolve_base_url, test_hook::attach_session_observer},
    output::resolve_output_layout,
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
            FailureStage::Collect | FailureStage::Download | FailureStage::Convert => {
                report.attempted += 1;
                match replay_artwork_record(
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

async fn replay_artwork_record(
    session: Arc<PixivNetSession>,
    layout: &crate::output::OutputLayout,
    options: crate::config::ResolvedDownloadOptions,
    record: FailureRecord,
) -> AppResult<Option<Vec<FailureRecord>>> {
    let Some(illust_id) = record.illust_id.clone() else {
        return Ok(None);
    };

    if record.stage == FailureStage::Download && is_direct_image_retry_record(&record) {
        return replay_image_download_record(session, options, record).await;
    }

    let planned = crate::crawler::shared::plan_artworks_for_illust_ids(
        &session,
        vec![illust_id],
        layout,
        &options,
    )
    .await;
    let mut failures = planned.failures;

    if planned.plans.is_empty() {
        return Ok(Some(failures));
    }

    let result = crate::crawler::shared::download_artworks(
        options,
        layout.context_dir().to_path_buf(),
        session,
        &planned.plans,
    )
    .await?;
    failures.extend(result.failure_records);

    Ok(Some(failures))
}

async fn replay_image_download_record(
    session: Arc<PixivNetSession>,
    options: crate::config::ResolvedDownloadOptions,
    record: FailureRecord,
) -> AppResult<Option<Vec<FailureRecord>>> {
    let Some(illust_id) = record.illust_id.clone() else {
        return Ok(None);
    };
    let Some(source_url) = record.source_url.clone() else {
        return Ok(None);
    };
    let Some(target_path) = record.target_path.clone() else {
        return Ok(None);
    };

    let target_dir = Path::new(&target_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let result = crate::crawler::shared::download_artworks(
        options.clone(),
        options.directory.clone(),
        session,
        &[ArtworkDownloadPlan::Images(ImageArtworkPlan {
            illust_id,
            image_urls: vec![source_url],
            target_dir,
        })],
    )
    .await?;

    Ok(Some(result.failure_records))
}

fn is_direct_image_retry_record(record: &FailureRecord) -> bool {
    let Some(target_path) = record.target_path.as_deref() else {
        return false;
    };

    !target_path.ends_with(".gif") && record.source_url.is_some()
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
    let failures = crate::crawler::shared::export_tags_json_for_illust_ids(
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
