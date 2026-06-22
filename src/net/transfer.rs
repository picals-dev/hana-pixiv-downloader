//! 图片流式写盘。

use std::path::{Path, PathBuf};

use futures::StreamExt;
use tokio::{fs, io::AsyncWriteExt};

use crate::error::{AppResult, CrawlerError};

use super::catalog::{ensure_content_length, ensure_non_empty_body};

pub async fn stream_response_to_temp_file(
    response: reqwest::Response,
    target_path: &Path,
) -> AppResult<u64> {
    let content_length = response.content_length();
    let temp_path = temporary_download_path(target_path);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let _ = fs::remove_file(&temp_path).await;
    let stream_result = async {
        let mut file = fs::File::create(&temp_path).await?;
        let mut stream = response.bytes_stream();
        let mut written = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            written += chunk.len() as u64;
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);

        ensure_non_empty_body(written as usize)?;
        ensure_content_length(content_length, written)?;
        fs::rename(&temp_path, target_path).await?;
        Ok::<u64, eyre::Report>(written)
    }
    .await;

    if stream_result.is_err() {
        let _ = fs::remove_file(&temp_path).await;
    }

    stream_result
}

pub fn temporary_download_path(path: &Path) -> PathBuf {
    let mut extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    if extension.is_empty() {
        extension = "part".to_string();
    } else {
        extension.push_str(".part");
    }

    path.with_extension(extension)
}

pub fn ensure_file_exists_and_nonempty(path: &Path) -> Result<bool, CrawlerError> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file() && metadata.len() > 0),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CrawlerError::Io(error)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::temporary_download_path;

    #[test]
    fn temporary_path_uses_part_extension() {
        let path = PathBuf::from("/tmp/a.png");
        assert_eq!(
            temporary_download_path(&path),
            PathBuf::from("/tmp/a.png.part")
        );
    }
}
