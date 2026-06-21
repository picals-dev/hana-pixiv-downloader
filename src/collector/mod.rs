//! Pixiv API 采集基础设施。

pub mod selector;

use std::env;

use serde_json::Value;
use url::{Url, form_urlencoded};

use crate::{
    PIXIV_BASE_URL,
    auth::Credential,
    config::ResolvedDownloadOptions,
    error::AppResult,
    net::{PixivRequestRuntime, RequestClass},
};

const BASE_URL_ENV_KEY: &str = "PICALS_PIXIV_BASE_URL";
pub const DEFAULT_BASE_URL: &str = PIXIV_BASE_URL;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct PixivCollector {
    runtime: PixivRequestRuntime,
    base_url: Url,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUserPage {
    pub header_user_id: Option<String>,
    pub html: String,
}

impl PixivCollector {
    pub fn new(options: &ResolvedDownloadOptions, credential: &Credential) -> AppResult<Self> {
        Self::new_with_base_url(options, credential, resolve_base_url(None)?)
    }

    pub fn new_with_base_url(
        options: &ResolvedDownloadOptions,
        credential: &Credential,
        base_url: Url,
    ) -> AppResult<Self> {
        Ok(Self {
            runtime: PixivRequestRuntime::new(options, credential)?,
            base_url,
        })
    }

    pub async fn fetch_user_profile_all(&self, user_id: &str) -> AppResult<Value> {
        let path = format!("/ajax/user/{user_id}/profile/all?lang=zh");
        let referer = self.referer_url(&format!("/users/{user_id}/illustrations"))?;
        self.get_json(&path, Some(referer), RequestClass::UserProfile)
            .await
    }

    pub async fn fetch_illust_pages(&self, illust_id: &str) -> AppResult<Value> {
        let path = format!("/ajax/illust/{illust_id}/pages?lang=zh");
        let referer = self.referer_url(&format!("/artworks/{illust_id}"))?;
        self.get_json(&path, Some(referer), RequestClass::IllustPages)
            .await
    }

    pub async fn fetch_illust_detail(&self, illust_id: &str) -> AppResult<Value> {
        let path = format!("/ajax/illust/{illust_id}?lang=zh");
        let referer = self.referer_url(&format!("/artworks/{illust_id}"))?;
        self.get_json(&path, Some(referer), RequestClass::IllustDetail)
            .await
    }

    pub async fn fetch_keyword_page(
        &self,
        keyword: &str,
        order: &str,
        mode: &str,
        page: usize,
        include_ai: bool,
    ) -> AppResult<Value> {
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
        let path = format!("/ajax/search/artworks/{encoded_keyword}?{}", query.finish());
        let referer = self.referer_url(&format!("/tags/{encoded_keyword}/artworks"))?;
        self.get_json(&path, Some(referer), RequestClass::KeywordSearch)
            .await
    }

    pub async fn fetch_ranking_page(&self, mode: &str, page: usize) -> AppResult<Value> {
        let path = format!("/ranking.php?mode={mode}&p={page}&format=json");
        let referer = self.referer_url("/ranking.php")?;
        self.get_json(&path, Some(referer), RequestClass::Ranking)
            .await
    }

    pub async fn fetch_bookmark_page(
        &self,
        user_id: &str,
        offset: usize,
        limit: usize,
    ) -> AppResult<Value> {
        let path = format!(
            "/ajax/user/{user_id}/illusts/bookmarks?tag=&offset={offset}&limit={limit}&rest=show&lang=zh"
        );
        let referer = self.referer_url("/bookmark.php?type=user")?;
        self.get_json(&path, Some(referer), RequestClass::Bookmark)
            .await
    }

    pub async fn fetch_current_user_homepage(&self) -> AppResult<CurrentUserPage> {
        let url = self.base_url.join("/")?;
        let response = self
            .runtime
            .get_text(url, None, RequestClass::Homepage)
            .await?;

        let header_user_id = response
            .headers
            .get("x-userid")
            .or_else(|| response.headers.get("x-user-id"))
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        Ok(CurrentUserPage {
            header_user_id,
            html: response.body,
        })
    }

    async fn get_json(
        &self,
        path: &str,
        referer: Option<String>,
        class: RequestClass,
    ) -> AppResult<Value> {
        let url = self.base_url.join(path)?;
        self.runtime.get_json(url, referer, class).await
    }

    fn referer_url(&self, path: &str) -> AppResult<String> {
        Ok(self.base_url.join(path)?.to_string())
    }
}

pub fn resolve_base_url(explicit_base_url: Option<&Url>) -> AppResult<Url> {
    if let Some(base_url) = explicit_base_url {
        return Ok(base_url.clone());
    }

    if let Some(base_url) = env::var(BASE_URL_ENV_KEY)
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(Url::parse(&base_url)?);
    }

    Ok(Url::parse(DEFAULT_BASE_URL)?)
}
