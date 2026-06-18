//! 单图下载辅助函数。

use std::path::{Path, PathBuf};

use url::Url;

use crate::error::CrawlerError;

pub fn file_name_from_image_url(url: &str) -> Result<String, CrawlerError> {
    let url = Url::parse(url)?;
    let path = url.path().trim_end_matches('/');
    let file_name = path
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .ok_or_else(|| CrawlerError::InvalidInput(format!("无法从 URL 提取文件名: {url}")))?;

    Ok(file_name.to_string())
}

pub fn target_path_for_image(directory: &Path, url: &str) -> Result<PathBuf, CrawlerError> {
    Ok(directory.join(file_name_from_image_url(url)?))
}

pub fn illust_id_from_image_url(url: &str) -> Result<String, CrawlerError> {
    let file_name = file_name_from_image_url(url)?;
    let illust_id = file_name
        .split_once("_p")
        .map(|(prefix, _)| prefix)
        .filter(|prefix| !prefix.is_empty() && prefix.chars().all(|ch| ch.is_ascii_digit()))
        .ok_or_else(|| CrawlerError::InvalidInput(format!("无法从图片 URL 提取作品 ID: {url}")))?;

    Ok(illust_id.to_string())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{file_name_from_image_url, illust_id_from_image_url, target_path_for_image};

    #[test]
    fn image_file_name_can_be_extracted() {
        let url = "https://i.pximg.net/img-original/img/2024/01/02/03/04/05/123456_p0.png";
        assert_eq!(file_name_from_image_url(url).unwrap(), "123456_p0.png");
    }

    #[test]
    fn image_target_path_is_joined_correctly() {
        let url = "https://i.pximg.net/img-original/img/2024/01/02/03/04/05/123456_p0.png";
        let path = target_path_for_image(Path::new("/tmp/picals"), url).unwrap();
        assert!(path.ends_with("123456_p0.png"));
    }

    #[test]
    fn illust_id_can_be_extracted_from_image_url() {
        let url = "https://i.pximg.net/img-original/img/2024/01/02/03/04/05/123456_p0.png";
        assert_eq!(illust_id_from_image_url(url).unwrap(), "123456");
    }
}
