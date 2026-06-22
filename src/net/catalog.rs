//! Pixiv 请求目录与请求语义。

use serde_json::Value;
use url::{Url, form_urlencoded};

use crate::{
    PIXIV_BASE_URL,
    error::{AppResult, CrawlerError},
    pixiv::selector::select_current_user_id,
};

const DEFAULT_IMAGE_BASE_URL: &str = "https://i.pximg.net";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostKind {
    Metadata,
    Image,
}

impl HostKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Image => "image",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestKind {
    Homepage,
    UserProfile,
    IllustPages,
    IllustDetail,
    KeywordSearch,
    Ranking,
    Bookmark,
    ImageDownload,
}

impl RequestKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Homepage => "homepage",
            Self::UserProfile => "user_profile",
            Self::IllustPages => "illust_pages",
            Self::IllustDetail => "illust_detail",
            Self::KeywordSearch => "keyword_search",
            Self::Ranking => "ranking",
            Self::Bookmark => "bookmark",
            Self::ImageDownload => "image_download",
        }
    }

    pub fn host_kind(self) -> HostKind {
        match self {
            Self::ImageDownload => HostKind::Image,
            Self::Homepage
            | Self::UserProfile
            | Self::IllustPages
            | Self::IllustDetail
            | Self::KeywordSearch
            | Self::Ranking
            | Self::Bookmark => HostKind::Metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSpec {
    pub kind: RequestKind,
    pub url: Url,
    pub referer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUserPage {
    pub header_user_id: Option<String>,
    pub html: String,
}

#[derive(Debug, Clone)]
pub struct PixivCatalog {
    metadata_base_url: Url,
}

impl PixivCatalog {
    pub fn new(metadata_base_url: Url) -> AppResult<Self> {
        let _ = Url::parse(DEFAULT_IMAGE_BASE_URL)?;
        Ok(Self { metadata_base_url })
    }

    pub fn homepage(&self) -> AppResult<RequestSpec> {
        Ok(RequestSpec {
            kind: RequestKind::Homepage,
            url: self.metadata_base_url.join("/")?,
            referer: None,
        })
    }

    pub fn user_profile_all(&self, user_id: &str) -> AppResult<RequestSpec> {
        self.metadata_request(
            RequestKind::UserProfile,
            &format!("/ajax/user/{user_id}/profile/all?lang=zh"),
            Some(&format!("/users/{user_id}/illustrations")),
        )
    }

    pub fn illust_pages(&self, illust_id: &str) -> AppResult<RequestSpec> {
        self.metadata_request(
            RequestKind::IllustPages,
            &format!("/ajax/illust/{illust_id}/pages?lang=zh"),
            Some(&format!("/artworks/{illust_id}")),
        )
    }

    pub fn illust_detail(&self, illust_id: &str) -> AppResult<RequestSpec> {
        self.metadata_request(
            RequestKind::IllustDetail,
            &format!("/ajax/illust/{illust_id}?lang=zh"),
            Some(&format!("/artworks/{illust_id}")),
        )
    }

    pub fn keyword_page(
        &self,
        keyword: &str,
        order: &str,
        mode: &str,
        page: usize,
        include_ai: bool,
    ) -> AppResult<RequestSpec> {
        let encoded_keyword: String = form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
        let mut query = form_urlencoded::Serializer::new(String::new());
        query.append_pair("word", keyword);
        query.append_pair("order", order);
        query.append_pair("mode", mode);
        query.append_pair("p", &page.to_string());
        query.append_pair("s_mode", "s_tag");
        query.append_pair("type", "all");
        if !include_ai {
            query.append_pair("ai_type", "1");
        }
        query.append_pair("lang", "zh");

        self.metadata_request(
            RequestKind::KeywordSearch,
            &format!("/ajax/search/artworks/{encoded_keyword}?{}", query.finish()),
            Some(&format!("/tags/{encoded_keyword}/artworks")),
        )
    }

    pub fn ranking_page(&self, mode: &str, page: usize) -> AppResult<RequestSpec> {
        self.metadata_request(
            RequestKind::Ranking,
            &format!("/ranking.php?mode={mode}&p={page}&format=json"),
            Some("/ranking.php"),
        )
    }

    pub fn bookmark_page(
        &self,
        user_id: &str,
        offset: usize,
        limit: usize,
    ) -> AppResult<RequestSpec> {
        self.metadata_request(
            RequestKind::Bookmark,
            &format!(
                "/ajax/user/{user_id}/illusts/bookmarks?tag=&offset={offset}&limit={limit}&rest=show&lang=zh"
            ),
            Some("/bookmark.php?type=user"),
        )
    }

    pub fn image_download(&self, image_url: &str, illust_id: &str) -> AppResult<RequestSpec> {
        let url = Url::parse(image_url)?;
        Ok(RequestSpec {
            kind: RequestKind::ImageDownload,
            url,
            referer: Some(
                self.metadata_base_url
                    .join(&format!("/artworks/{illust_id}"))?
                    .to_string(),
            ),
        })
    }

    pub fn parse_current_user_page(
        &self,
        header_user_id: Option<String>,
        html: String,
    ) -> AppResult<CurrentUserPage> {
        let header_user_id = header_user_id
            .filter(|value| !value.trim().is_empty())
            .or_else(|| select_current_user_id(None, &html).ok());
        Ok(CurrentUserPage {
            header_user_id,
            html,
        })
    }

    fn metadata_request(
        &self,
        kind: RequestKind,
        path: &str,
        referer_path: Option<&str>,
    ) -> AppResult<RequestSpec> {
        Ok(RequestSpec {
            kind,
            url: self.metadata_base_url.join(path)?,
            referer: referer_path
                .map(|value| self.metadata_base_url.join(value))
                .transpose()?
                .map(|url| url.to_string()),
        })
    }
}

pub fn default_metadata_base_url() -> AppResult<Url> {
    Ok(Url::parse(PIXIV_BASE_URL)?)
}

pub fn extract_header_user_id(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("x-userid")
        .or_else(|| headers.get("x-user-id"))
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

pub fn ensure_non_empty_body(bytes_len: usize) -> AppResult<()> {
    if bytes_len == 0 {
        return Err(CrawlerError::DownloadInterrupted("下载到空文件".to_string()).into());
    }
    Ok(())
}

pub fn ensure_content_length(content_length: Option<u64>, bytes_len: u64) -> AppResult<()> {
    if let Some(expected_length) = content_length
        && expected_length != bytes_len
    {
        return Err(CrawlerError::DownloadInterrupted("下载内容长度不匹配".to_string()).into());
    }
    Ok(())
}

pub fn extract_json_body(value: Value) -> AppResult<Value> {
    if value.get("error").and_then(Value::as_bool).unwrap_or(false) {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .unwrap_or("Pixiv 返回 error=true");
        return Err(CrawlerError::Parse(message.to_string()).into());
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::{HostKind, PixivCatalog, RequestKind};

    #[test]
    fn request_kind_maps_to_expected_host() {
        assert_eq!(RequestKind::UserProfile.host_kind(), HostKind::Metadata);
        assert_eq!(RequestKind::ImageDownload.host_kind(), HostKind::Image);
    }

    #[test]
    fn catalog_builds_expected_urls_and_referers() {
        let catalog = PixivCatalog::new(Url::parse("https://www.pixiv.net").unwrap()).unwrap();

        let spec = catalog.user_profile_all("123456").unwrap();
        assert_eq!(spec.kind, RequestKind::UserProfile);
        assert_eq!(
            spec.url.as_str(),
            "https://www.pixiv.net/ajax/user/123456/profile/all?lang=zh"
        );
        assert_eq!(
            spec.referer.as_deref(),
            Some("https://www.pixiv.net/users/123456/illustrations")
        );

        let ranking = catalog.ranking_page("daily", 2).unwrap();
        assert_eq!(ranking.kind, RequestKind::Ranking);
        assert_eq!(
            ranking.url.as_str(),
            "https://www.pixiv.net/ranking.php?mode=daily&p=2&format=json"
        );
        assert_eq!(
            ranking.referer.as_deref(),
            Some("https://www.pixiv.net/ranking.php")
        );
    }

    #[test]
    fn image_download_uses_metadata_referer() {
        let catalog = PixivCatalog::new(Url::parse("https://pixiv.example").unwrap()).unwrap();
        let spec = catalog
            .image_download("https://img.example/123456_p0.png", "123456")
            .unwrap();
        assert_eq!(spec.kind, RequestKind::ImageDownload);
        assert_eq!(spec.url.as_str(), "https://img.example/123456_p0.png");
        assert_eq!(
            spec.referer.as_deref(),
            Some("https://pixiv.example/artworks/123456")
        );
    }
}
