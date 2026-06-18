//! Pixiv API 采集基础设施。

pub mod selector;

use std::env;

use reqwest::{
    Client, Proxy,
    header::{COOKIE, HeaderMap, HeaderValue, REFERER, USER_AGENT},
};
use serde_json::Value;
use url::Url;

use crate::{
    PIXIV_BASE_URL,
    auth::Credential,
    config::ResolvedDownloadOptions,
    error::{AppResult, CrawlerError},
};

const BASE_URL_ENV_KEY: &str = "PICALS_PIXIV_BASE_URL";
pub const DEFAULT_BASE_URL: &str = PIXIV_BASE_URL;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct PixivCollector {
    client: Client,
    base_url: Url,
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
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&credential.cookie_header())
                .map_err(|error| CrawlerError::Auth(error.to_string()))?,
        );

        let mut builder = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(options.timeout));

        if let Some(proxy_url) = options.proxy_url.as_deref() {
            builder = builder.proxy(
                Proxy::all(proxy_url)
                    .map_err(|error| CrawlerError::Config(format!("代理配置无效: {error}")))?,
            );
        }

        let client = builder.build()?;

        Ok(Self { client, base_url })
    }

    pub async fn fetch_user_profile_all(&self, user_id: &str) -> AppResult<Value> {
        let path = format!("/ajax/user/{user_id}/profile/all?lang=zh");
        let referer = self.referer_url(&format!("/users/{user_id}/illustrations"))?;
        self.get_json(&path, Some(referer)).await
    }

    pub async fn fetch_illust_pages(&self, illust_id: &str) -> AppResult<Value> {
        let path = format!("/ajax/illust/{illust_id}/pages?lang=zh");
        let referer = self.referer_url(&format!("/artworks/{illust_id}"))?;
        self.get_json(&path, Some(referer)).await
    }

    pub async fn fetch_illust_detail(&self, illust_id: &str) -> AppResult<Value> {
        let path = format!("/ajax/illust/{illust_id}?lang=zh");
        let referer = self.referer_url(&format!("/artworks/{illust_id}"))?;
        self.get_json(&path, Some(referer)).await
    }

    async fn get_json(&self, path: &str, referer: Option<String>) -> AppResult<Value> {
        let url = self.base_url.join(path)?;
        let mut request = self.client.get(url);

        if let Some(referer) = referer {
            request = request.header(REFERER, referer);
        }

        let response = request.send().await?.error_for_status()?;
        Ok(response.json::<Value>().await?)
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
