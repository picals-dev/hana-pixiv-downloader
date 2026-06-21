//! `picals-crawler retry` 命令。

use crate::{
    auth::Credential, cli::RetryCommand, error::AppResult, failure::FailureManifest,
    replay::replay_failures,
};

pub async fn run(args: RetryCommand) -> AppResult<()> {
    let manifest = FailureManifest::load_from(&args.manifest_path)?;
    let credential = Credential::load()?.ok_or(crate::error::CrawlerError::MissingCredential)?;
    let report = replay_failures(&credential, &manifest.command, manifest.records).await?;

    println!("读取失败清单: {}", args.manifest_path.display());
    println!("尝试回放项目: {}", report.attempted);
    println!("回放成功项目: {}", report.recovered);
    println!("跳过项目: {}", report.skipped_non_retryable);
    println!("剩余失败项目: {}", report.remaining_records.len());

    Ok(())
}
