//! reqwest client 构造与 host 分离。

use std::time::Duration;

use reqwest::{
    Client, Proxy,
    header::{COOKIE, HeaderMap, HeaderValue, USER_AGENT},
};

use crate::{
    auth::Credential,
    config::ResolvedDownloadOptions,
    error::{AppResult, CrawlerError},
};

use super::{HostKind, RequestKind, policy::host_timeout};

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub(crate) struct NetClients {
    metadata: Client,
    image: Client,
}

impl NetClients {
    pub(crate) fn new(
        options: &ResolvedDownloadOptions,
        credential: &Credential,
    ) -> AppResult<Self> {
        Ok(Self {
            metadata: build_metadata_client(options, credential)?,
            image: build_image_client(options)?,
        })
    }

    pub(crate) fn client_for(&self, kind: RequestKind) -> &Client {
        match kind.host_kind() {
            HostKind::Metadata => &self.metadata,
            HostKind::Image => &self.image,
        }
    }
}

fn build_metadata_client(
    options: &ResolvedDownloadOptions,
    credential: &Credential,
) -> AppResult<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&format!("PHPSESSID={}", credential.phpsessid))
            .map_err(|error| CrawlerError::Auth(error.to_string()))?,
    );

    build_client(options, headers, host_timeout(RequestKind::Homepage))
}

fn build_image_client(options: &ResolvedDownloadOptions) -> AppResult<Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
    build_client(options, headers, host_timeout(RequestKind::ImageDownload))
}

fn build_client(
    options: &ResolvedDownloadOptions,
    headers: HeaderMap,
    timeout: Duration,
) -> AppResult<Client> {
    let mut builder = Client::builder()
        .default_headers(headers)
        .timeout(timeout)
        .user_agent(DEFAULT_USER_AGENT);

    if let Some(proxy_url) = options.proxy_url.as_deref() {
        builder = builder.proxy(
            Proxy::all(proxy_url)
                .map_err(|error| CrawlerError::Config(format!("代理配置无效: {error}")))?,
        );
    }

    Ok(builder.build()?)
}
