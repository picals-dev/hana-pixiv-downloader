//! ugoira 下载与 GIF 转换。

use std::{
    fs::File,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use gif::{Encoder, Frame, Repeat};
use tokio::fs;
use zip::ZipArchive;

use crate::{
    error::{AppResult, CrawlerError},
    failure::FailureStage,
    net::{PixivNetSession, transfer::TransferChunkObserver},
};

use super::UgoiraDownloadPlan;

const WORKSPACE_DIR: &str = ".picals-workspace";
const UGOIRA_ARCHIVE_NAME: &str = "source.zip";
const UGOIRA_TEMP_GIF_NAME: &str = "output.gif.part";

#[derive(Debug)]
pub(crate) struct UgoiraDownloadError {
    pub stage: FailureStage,
    pub error: eyre::Report,
}

pub(crate) async fn download_ugoira_with_progress(
    session: &PixivNetSession,
    plan: &UgoiraDownloadPlan,
    on_chunk: Option<Arc<TransferChunkObserver>>,
) -> Result<u64, UgoiraDownloadError> {
    let workspace_root = workspace_root_for_target(&plan.target_path);
    let archive_path = workspace_root.join(UGOIRA_ARCHIVE_NAME);
    let temp_gif_path = workspace_root.join(UGOIRA_TEMP_GIF_NAME);

    let result = async {
        cleanup_workspace(&workspace_root)
            .await
            .map_err(|error| UgoiraDownloadError {
                stage: FailureStage::Convert,
                error,
            })?;
        fs::create_dir_all(&workspace_root)
            .await
            .map_err(|error| UgoiraDownloadError {
                stage: FailureStage::Convert,
                error: error.into(),
            })?;

        let archive_bytes = session
            .download_ugoira_archive(&plan.source_url, &plan.illust_id, &archive_path, on_chunk)
            .await
            .map_err(|error| UgoiraDownloadError {
                stage: FailureStage::Download,
                error,
            })?;

        let convert_plan = plan.clone();
        let convert_archive_path = archive_path.clone();
        let convert_temp_gif_path = temp_gif_path.clone();
        tokio::task::spawn_blocking(move || {
            convert_archive_to_gif(&convert_archive_path, &convert_temp_gif_path, &convert_plan)
        })
        .await
        .map_err(|error| UgoiraDownloadError {
            stage: FailureStage::Convert,
            error: eyre::eyre!("GIF 编码任务异常终止: {error}"),
        })?
        .map_err(|error| UgoiraDownloadError {
            stage: FailureStage::Convert,
            error,
        })?;

        if let Some(parent) = plan.target_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|error| UgoiraDownloadError {
                    stage: FailureStage::Convert,
                    error: error.into(),
                })?;
        }
        fs::rename(&temp_gif_path, &plan.target_path)
            .await
            .map_err(|error| UgoiraDownloadError {
                stage: FailureStage::Convert,
                error: error.into(),
            })?;

        Ok::<u64, UgoiraDownloadError>(archive_bytes)
    }
    .await;

    let cleanup_result = cleanup_workspace(&workspace_root).await;

    match (result, cleanup_result) {
        (Ok(bytes), Ok(())) => Ok(bytes),
        (Ok(_), Err(error)) => Err(UgoiraDownloadError {
            stage: FailureStage::Convert,
            error,
        }),
        (Err(error), _) => Err(error),
    }
}

pub(crate) fn target_path_for_ugoira(directory: &Path, illust_id: &str) -> PathBuf {
    directory.join(format!("{illust_id}.gif"))
}

pub fn quantize_delay_centiseconds(delay_ms: u64) -> u16 {
    let centiseconds = delay_ms.div_ceil(10).max(1);
    centiseconds.min(u16::MAX as u64) as u16
}

fn workspace_root_for_target(target_path: &Path) -> PathBuf {
    target_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(WORKSPACE_DIR)
}

async fn cleanup_workspace(path: &Path) -> AppResult<()> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn convert_archive_to_gif(
    archive_path: &Path,
    target_path: &Path,
    plan: &UgoiraDownloadPlan,
) -> AppResult<()> {
    let archive_file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| CrawlerError::MediaConversion(format!("打开 ugoira zip 失败: {error}")))?;

    let first_frame = plan
        .metadata
        .frames
        .first()
        .ok_or_else(|| CrawlerError::MediaConversion("ugoira frames 为空".to_string()))?;
    let first_image = load_frame_rgba(&mut archive, &first_frame.file)?;
    let width = u16::try_from(first_image.width())
        .map_err(|_| CrawlerError::MediaConversion("ugoira 帧宽超出 GIF 支持范围".to_string()))?;
    let height = u16::try_from(first_image.height())
        .map_err(|_| CrawlerError::MediaConversion("ugoira 帧高超出 GIF 支持范围".to_string()))?;

    let mut writer = BufWriter::new(File::create(target_path)?);
    {
        let mut encoder = Encoder::new(&mut writer, width, height, &[]).map_err(|error| {
            CrawlerError::MediaConversion(format!("初始化 GIF 编码器失败: {error}"))
        })?;
        encoder.set_repeat(Repeat::Infinite).map_err(|error| {
            CrawlerError::MediaConversion(format!("设置 GIF 循环次数失败: {error}"))
        })?;
        write_gif_frame(
            &mut encoder,
            width,
            height,
            first_image.into_raw(),
            first_frame.delay_ms,
        )?;

        for frame in plan.metadata.frames.iter().skip(1) {
            let image = load_frame_rgba(&mut archive, &frame.file)?;
            if image.width() != u32::from(width) || image.height() != u32::from(height) {
                return Err(CrawlerError::MediaConversion(format!(
                    "ugoira 帧尺寸不一致: {} 不是 {}x{}",
                    frame.file, width, height
                ))
                .into());
            }

            write_gif_frame(
                &mut encoder,
                width,
                height,
                image.into_raw(),
                frame.delay_ms,
            )?;
        }
    }
    writer.flush()?;
    Ok(())
}

fn load_frame_rgba(
    archive: &mut ZipArchive<File>,
    frame_name: &str,
) -> Result<image::RgbaImage, eyre::Report> {
    let mut file = archive.by_name(frame_name).map_err(|error| {
        CrawlerError::MediaConversion(format!("ugoira zip 缺少帧 {frame_name}: {error}"))
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let image = image::load_from_memory(&bytes).map_err(|error| {
        CrawlerError::MediaConversion(format!("解析 ugoira 帧 {frame_name} 失败: {error}"))
    })?;
    Ok(image.into_rgba8())
}

fn write_gif_frame(
    encoder: &mut Encoder<&mut BufWriter<File>>,
    width: u16,
    height: u16,
    mut pixels: Vec<u8>,
    delay_ms: u64,
) -> AppResult<()> {
    let mut frame = Frame::from_rgba_speed(width, height, &mut pixels, 10);
    frame.delay = quantize_delay_centiseconds(delay_ms);
    encoder
        .write_frame(&frame)
        .map_err(|error| CrawlerError::MediaConversion(format!("写入 GIF 帧失败: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{quantize_delay_centiseconds, target_path_for_ugoira};

    #[test]
    fn delay_is_quantized_to_centiseconds() {
        assert_eq!(quantize_delay_centiseconds(1), 1);
        assert_eq!(quantize_delay_centiseconds(10), 1);
        assert_eq!(quantize_delay_centiseconds(11), 2);
    }

    #[test]
    fn ugoira_target_path_uses_gif_suffix() {
        assert_eq!(
            target_path_for_ugoira(Path::new("/tmp/output/123456"), "123456"),
            Path::new("/tmp/output/123456/123456.gif")
        );
    }
}
