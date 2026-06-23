//! Pixiv 共享网络会话 façade。

use std::{
    env,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use eyre::eyre;
use log::warn;
use reqwest::header::REFERER;
use serde_json::Value;
use url::Url;

use crate::{
    auth::Credential,
    config::ResolvedDownloadOptions,
    error::{AppResult, CrawlerError},
};

use super::{
    NetEvent, SessionObserver,
    catalog::{
        CurrentUserPage, PixivCatalog, RequestSpec, extract_header_user_id, extract_json_body,
    },
    client::NetClients,
    policy::{retry_delay_for_error, retry_delay_for_status},
    state::SharedState,
    transfer::{
        TransferChunkObserver, ensure_file_exists_and_nonempty, stream_response_to_temp_file,
    },
};

const BASE_URL_ENV_KEY: &str = "PICALS_PIXIV_BASE_URL";

type SleepFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type NowFn = Arc<dyn Fn() -> SystemTime + Send + Sync>;
type SleepFn = Arc<dyn Fn(Duration) -> SleepFuture + Send + Sync>;

static SESSION_ID_ALLOCATOR: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct PixivNetSession {
    session_id: u64,
    attempt_limit: usize,
    catalog: PixivCatalog,
    clients: NetClients,
    state: Arc<SharedState>,
    hooks: RuntimeHooks,
}

impl std::fmt::Debug for PixivNetSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixivNetSession")
            .field("session_id", &self.session_id)
            .field("attempt_limit", &self.attempt_limit)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct PixivNetSessionBuilder {
    options: ResolvedDownloadOptions,
    credential: Credential,
    base_url: Url,
    hooks: RuntimeHooks,
}

#[derive(Clone)]
struct RuntimeHooks {
    now: NowFn,
    sleep: SleepFn,
    observer: Option<SessionObserver>,
}

#[derive(Debug, Clone, Copy)]
struct AttemptContext<'a> {
    session_id: u64,
    host: super::HostKind,
    kind: super::RequestKind,
    attempt: usize,
    url: &'a str,
}

enum AttemptOutcome<T> {
    Success(T),
    Retry {
        delay: Duration,
        reason: String,
        cooldown: bool,
    },
    Fail(eyre::Report),
}

impl Default for RuntimeHooks {
    fn default() -> Self {
        Self {
            now: Arc::new(SystemTime::now),
            sleep: Arc::new(|duration| Box::pin(tokio::time::sleep(duration))),
            observer: None,
        }
    }
}

impl PixivNetSessionBuilder {
    pub fn new(options: ResolvedDownloadOptions, credential: Credential, base_url: Url) -> Self {
        Self {
            options,
            credential,
            base_url,
            hooks: RuntimeHooks::default(),
        }
    }

    pub fn with_observer(mut self, observer: SessionObserver) -> Self {
        self.hooks.observer = Some(observer);
        self
    }

    pub fn build(self) -> AppResult<PixivNetSession> {
        PixivNetSession::new_with_hooks(self.options, self.credential, self.base_url, self.hooks)
    }
}

impl PixivNetSession {
    pub fn new(options: ResolvedDownloadOptions, credential: Credential) -> AppResult<Self> {
        let base_url = resolve_base_url(None)?;
        Self::new_with_base_url(options, credential, base_url)
    }

    pub fn new_with_base_url(
        options: ResolvedDownloadOptions,
        credential: Credential,
        base_url: Url,
    ) -> AppResult<Self> {
        Self::new_with_hooks(options, credential, base_url, RuntimeHooks::default())
    }

    fn new_with_hooks(
        options: ResolvedDownloadOptions,
        credential: Credential,
        base_url: Url,
        hooks: RuntimeHooks,
    ) -> AppResult<Self> {
        Ok(Self {
            session_id: SESSION_ID_ALLOCATOR.fetch_add(1, Ordering::Relaxed),
            attempt_limit: options.retry.max(1),
            catalog: PixivCatalog::new(base_url)?,
            clients: NetClients::new(&options, &credential)?,
            state: Arc::new(SharedState::default()),
            hooks,
        })
    }

    pub fn builder(
        options: ResolvedDownloadOptions,
        credential: Credential,
        base_url: Url,
    ) -> PixivNetSessionBuilder {
        PixivNetSessionBuilder::new(options, credential, base_url)
    }

    pub fn session_id(&self) -> u64 {
        self.session_id
    }

    pub async fn fetch_current_user_homepage(&self) -> AppResult<CurrentUserPage> {
        let spec = self.catalog.homepage()?;
        let (headers, body) = self
            .execute(spec, |response| async move {
                let headers = response.headers().clone();
                let body = response.text().await?;
                Ok((headers, body))
            })
            .await?;
        self.catalog
            .parse_current_user_page(extract_header_user_id(&headers), body)
    }

    pub async fn fetch_user_profile_all(&self, user_id: &str) -> AppResult<Value> {
        let spec = self.catalog.user_profile_all(user_id)?;
        self.get_json(spec).await
    }

    pub async fn fetch_illust_pages(&self, illust_id: &str) -> AppResult<Value> {
        let spec = self.catalog.illust_pages(illust_id)?;
        self.get_json(spec).await
    }

    pub async fn fetch_illust_detail(&self, illust_id: &str) -> AppResult<Value> {
        let spec = self.catalog.illust_detail(illust_id)?;
        self.get_json(spec).await
    }

    pub async fn fetch_keyword_page(
        &self,
        keyword: &str,
        order: &str,
        mode: &str,
        page: usize,
        include_ai: bool,
    ) -> AppResult<Value> {
        let spec = self
            .catalog
            .keyword_page(keyword, order, mode, page, include_ai)?;
        self.get_json(spec).await
    }

    pub async fn fetch_ranking_page(&self, mode: &str, page: usize) -> AppResult<Value> {
        let spec = self.catalog.ranking_page(mode, page)?;
        self.get_json(spec).await
    }

    pub async fn fetch_bookmark_page(
        &self,
        user_id: &str,
        offset: usize,
        limit: usize,
    ) -> AppResult<Value> {
        let spec = self.catalog.bookmark_page(user_id, offset, limit)?;
        self.get_json(spec).await
    }

    pub async fn download_original_image(
        &self,
        image_url: &str,
        illust_id: &str,
        target_path: &Path,
    ) -> AppResult<u64> {
        self.download_original_image_with_progress(image_url, illust_id, target_path, None)
            .await
    }

    pub(crate) async fn download_original_image_with_progress(
        &self,
        image_url: &str,
        illust_id: &str,
        target_path: &Path,
        on_chunk: Option<Arc<TransferChunkObserver>>,
    ) -> AppResult<u64> {
        let spec = self.catalog.image_download(image_url, illust_id)?;
        if ensure_file_exists_and_nonempty(target_path)? {
            return Ok(0);
        }

        let target_string = target_path.display().to_string();
        let session_id = self.session_id;
        let bytes = self
            .execute(spec, |response| {
                let target_path = target_path.to_path_buf();
                let target_string = target_string.clone();
                let on_chunk = on_chunk.clone();
                let observer = self.hooks.observer.clone();
                async move {
                    let chunk_observer = if on_chunk.is_some() || observer.is_some() {
                        let target_string = target_string.clone();
                        let on_chunk = on_chunk.clone();
                        let observer = observer.clone();
                        Some(Arc::new(move |bytes| {
                            if let Some(on_chunk) = &on_chunk {
                                on_chunk(bytes);
                            }
                            if let Some(observer) = &observer {
                                observer(NetEvent::TransferProgress {
                                    session_id,
                                    bytes,
                                    target_path: target_string.clone(),
                                });
                            }
                        }) as Arc<TransferChunkObserver>)
                    } else {
                        None
                    };

                    stream_response_to_temp_file(response, &target_path, chunk_observer).await
                }
            })
            .await?;
        self.emit(NetEvent::TransferCompleted {
            session_id: self.session_id,
            bytes,
            target_path: target_string,
        });
        Ok(bytes)
    }

    async fn get_json(&self, spec: RequestSpec) -> AppResult<Value> {
        self.execute(spec, |response| async move {
            let value = response.json::<Value>().await?;
            extract_json_body(value)
        })
        .await
    }

    async fn execute<T, P, Fut>(&self, spec: RequestSpec, parser: P) -> AppResult<T>
    where
        P: Fn(reqwest::Response) -> Fut,
        Fut: Future<Output = AppResult<T>>,
    {
        let url_string = spec.url.to_string();

        for attempt_index in 0..self.attempt_limit {
            let context = AttemptContext {
                session_id: self.session_id,
                host: spec.kind.host_kind(),
                kind: spec.kind,
                attempt: attempt_index + 1,
                url: &url_string,
            };

            self.enforce_cooldown(context.host).await;
            self.emit_attempt(&context);

            match self.run_single_attempt(&context, &spec, &parser).await {
                AttemptOutcome::Success(value) => return Ok(value),
                AttemptOutcome::Retry {
                    delay,
                    reason,
                    cooldown,
                } => {
                    self.handle_retry(&context, delay, reason, cooldown).await;
                    continue;
                }
                AttemptOutcome::Fail(error) => {
                    self.emit_failure(&context, &error);
                    return Err(error);
                }
            }
        }

        unreachable!("请求至少会执行一次")
    }

    fn build_request(&self, spec: &RequestSpec) -> reqwest::RequestBuilder {
        let mut request = self.clients.client_for(spec.kind).get(spec.url.clone());
        if let Some(referer) = spec.referer.clone() {
            request = request.header(REFERER, referer);
        }
        request
    }

    async fn run_single_attempt<T, P, Fut>(
        &self,
        context: &AttemptContext<'_>,
        spec: &RequestSpec,
        parser: &P,
    ) -> AttemptOutcome<T>
    where
        P: Fn(reqwest::Response) -> Fut,
        Fut: Future<Output = AppResult<T>>,
    {
        match self.build_request(spec).send().await {
            Ok(response) => self.classify_response(context, parser, response).await,
            Err(error) => self.classify_transport_error(context, eyre!(error)),
        }
    }

    async fn classify_response<T, P, Fut>(
        &self,
        context: &AttemptContext<'_>,
        parser: &P,
        response: reqwest::Response,
    ) -> AttemptOutcome<T>
    where
        P: Fn(reqwest::Response) -> Fut,
        Fut: Future<Output = AppResult<T>>,
    {
        let status = response.status();
        if status.is_success() {
            return match parser(response).await {
                Ok(value) => AttemptOutcome::Success(value),
                Err(error) => self.classify_execution_error(context, error),
            };
        }

        let headers = response.headers().clone();
        if let Some(delay) = retry_delay_for_status(
            context.kind,
            context.attempt - 1,
            self.attempt_limit,
            status,
            &headers,
            (self.hooks.now)(),
        ) {
            return AttemptOutcome::Retry {
                delay,
                reason: format!("HTTP {}", status.as_u16()),
                cooldown: super::is_cooldown_status(status),
            };
        }

        AttemptOutcome::Fail(eyre!(CrawlerError::HttpStatus {
            status: status.as_u16(),
            context: format!("请求 {} 失败，URL: {}", context.kind.label(), context.url),
        }))
    }

    fn classify_transport_error<T>(
        &self,
        context: &AttemptContext<'_>,
        error: eyre::Report,
    ) -> AttemptOutcome<T> {
        self.classify_execution_error(context, error)
    }

    fn classify_execution_error<T>(
        &self,
        context: &AttemptContext<'_>,
        error: eyre::Report,
    ) -> AttemptOutcome<T> {
        if let Some(delay) = retry_delay_for_error(
            context.kind,
            context.attempt - 1,
            self.attempt_limit,
            &error,
        ) {
            return AttemptOutcome::Retry {
                delay,
                reason: error.to_string(),
                cooldown: false,
            };
        }

        AttemptOutcome::Fail(error)
    }

    async fn handle_retry(
        &self,
        context: &AttemptContext<'_>,
        delay: Duration,
        reason: String,
        cooldown: bool,
    ) {
        if cooldown {
            self.state
                .extend_cooldown(context.host, (self.hooks.now)(), delay)
                .await;
            self.emit(NetEvent::Cooldown {
                session_id: context.session_id,
                host: context.host,
                delay,
            });
        }

        self.emit(NetEvent::Retry {
            session_id: context.session_id,
            host: context.host,
            kind: context.kind,
            attempt: context.attempt,
            delay,
            reason,
        });
        (self.hooks.sleep)(delay).await;
    }

    fn emit_attempt(&self, context: &AttemptContext<'_>) {
        self.emit(NetEvent::Attempt {
            session_id: context.session_id,
            host: context.host,
            kind: context.kind,
            attempt: context.attempt,
            url: context.url.to_string(),
        });
    }

    fn emit_failure(&self, context: &AttemptContext<'_>, error: &eyre::Report) {
        self.emit(NetEvent::Failure {
            session_id: context.session_id,
            host: context.host,
            kind: context.kind,
            attempts: context.attempt,
            reason: error.to_string(),
        });
    }

    async fn enforce_cooldown(&self, host: super::HostKind) {
        let Some(wait_duration) = self
            .state
            .cooldown_remaining(host, (self.hooks.now)())
            .await
        else {
            return;
        };

        (self.hooks.sleep)(wait_duration).await;
    }

    fn emit(&self, event: NetEvent) {
        match &event {
            NetEvent::Retry {
                kind,
                attempt,
                delay,
                reason,
                ..
            } => warn!(
                "request.retry class={} attempt={} delay_ms={} reason={}",
                kind.label(),
                attempt,
                delay.as_millis(),
                reason
            ),
            NetEvent::Failure {
                kind,
                attempts,
                reason,
                ..
            } => warn!(
                "request.failure class={} attempts={} reason={}",
                kind.label(),
                attempts,
                reason
            ),
            _ => {}
        }

        if let Some(observer) = &self.hooks.observer {
            observer(event);
        }
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

    super::catalog::default_metadata_base_url()
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, SystemTime},
    };

    use jiff::{Timestamp, fmt::rfc2822::DateTimePrinter};
    use reqwest::header::HeaderMap;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use crate::{
        auth::Credential,
        config::{DownloadConfig, DownloadMode, ResolvedDownloadOptions, SortOrder},
        net::{HostKind, NetEvent},
    };
    use url::Url;

    use super::{PixivNetSession, RuntimeHooks};

    #[derive(Clone)]
    struct ManualClock {
        now: Arc<Mutex<SystemTime>>,
        sleeps: Arc<Mutex<Vec<Duration>>>,
        events: Arc<Mutex<Vec<NetEvent>>>,
    }

    impl ManualClock {
        fn new(start: SystemTime) -> Self {
            Self {
                now: Arc::new(Mutex::new(start)),
                sleeps: Arc::new(Mutex::new(Vec::new())),
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn hooks(&self) -> RuntimeHooks {
            let now = self.now.clone();
            let sleep_now = self.now.clone();
            let sleeps = self.sleeps.clone();
            let events = self.events.clone();

            RuntimeHooks {
                now: Arc::new(move || *now.lock().unwrap()),
                sleep: Arc::new(move |duration| {
                    let sleeps = sleeps.clone();
                    let sleep_now = sleep_now.clone();
                    Box::pin(async move {
                        sleeps.lock().unwrap().push(duration);
                        let mut current = sleep_now.lock().unwrap();
                        *current += duration;
                    })
                }),
                observer: Some(Arc::new(move |event| {
                    events.lock().unwrap().push(event);
                })),
            }
        }
    }

    fn options(timeout: u64, retry: usize) -> ResolvedDownloadOptions {
        let defaults = DownloadConfig::default();
        ResolvedDownloadOptions {
            mode: DownloadMode::Illust,
            directory: "/tmp/picals".into(),
            count: defaults.count,
            sort: SortOrder::DateDesc,
            r18: defaults.r18,
            ai: defaults.ai,
            concurrent: defaults.concurrent,
            timeout,
            retry,
            with_tags: false,
            proxy_url: None,
            dry_run: false,
        }
    }

    #[test]
    fn retry_after_supports_http_date() {
        let timestamp = Timestamp::from_second(10).unwrap();
        let header_value = DateTimePrinter::new()
            .timestamp_to_rfc9110_string(&timestamp)
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("Retry-After", header_value.parse().unwrap());

        let delay =
            crate::net::policy::parse_retry_after(&headers, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn session_retries_429_with_host_cooldown() {
        let server = MockServer::start().await;
        let clock = ManualClock::new(SystemTime::UNIX_EPOCH);
        let session = PixivNetSession::new_with_hooks(
            options(5, 2),
            Credential::new("cookie").unwrap(),
            server.uri().parse().unwrap(),
            clock.hooks(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/ajax/illust/123456/pages"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "2"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/ajax/illust/123456/pages"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"error": false, "body": []})),
            )
            .mount(&server)
            .await;

        session.fetch_illust_pages("123456").await.unwrap();

        assert_eq!(
            clock.sleeps.lock().unwrap().as_slice(),
            &[Duration::from_secs(2)]
        );
        assert!(clock.events.lock().unwrap().iter().any(|event| matches!(
            event,
            NetEvent::Cooldown {
                host: HostKind::Metadata,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn session_retries_503_without_host_cooldown() {
        let server = MockServer::start().await;
        let clock = ManualClock::new(SystemTime::UNIX_EPOCH);
        let session = PixivNetSession::new_with_hooks(
            options(5, 2),
            Credential::new("cookie").unwrap(),
            server.uri().parse().unwrap(),
            clock.hooks(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/ajax/illust/123456/pages"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/ajax/illust/123456/pages"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"error": false, "body": []})),
            )
            .mount(&server)
            .await;

        session.fetch_illust_pages("123456").await.unwrap();

        assert_eq!(clock.sleeps.lock().unwrap().len(), 1);
        assert!(!clock.events.lock().unwrap().iter().any(|event| matches!(
            event,
            NetEvent::Cooldown {
                host: HostKind::Metadata,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn image_download_does_not_send_cookie_header() {
        let server = MockServer::start().await;
        let events = Arc::new(Mutex::new(Vec::<NetEvent>::new()));
        let observer_events = Arc::clone(&events);
        let session = PixivNetSession::builder(
            options(5, 2),
            Credential::new("cookie").unwrap(),
            server.uri().parse().unwrap(),
        )
        .with_observer(Arc::new(move |event| {
            observer_events.lock().unwrap().push(event);
        }))
        .build()
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let target_path = temp.path().join("123456_p0.png");

        Mock::given(method("GET"))
            .and(path("/img-original/123456_p0.png"))
            .and(header(
                "referer",
                format!("{}/artworks/123456", server.uri()),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
            .mount(&server)
            .await;

        session
            .download_original_image(
                &format!("{}/img-original/123456_p0.png", server.uri()),
                "123456",
                &target_path,
            )
            .await
            .unwrap();

        assert!(target_path.exists());
        let observed = events.lock().unwrap();
        assert!(observed.iter().any(|event| matches!(
            event,
            NetEvent::TransferProgress { bytes, .. } if *bytes > 0
        )));
        assert!(observed.iter().any(|event| matches!(
            event,
            NetEvent::TransferCompleted { bytes, .. } if *bytes > 0
        )));
    }

    #[tokio::test]
    async fn metadata_request_sends_cookie_header() {
        let server = MockServer::start().await;
        let session = PixivNetSession::new_with_base_url(
            options(5, 2),
            Credential::new("cookie").unwrap(),
            server.uri().parse().unwrap(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/ajax/user/123456/profile/all"))
            .and(header("cookie", "PHPSESSID=cookie"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "error": false,
                "body": { "illusts": {}, "manga": {} }
            })))
            .mount(&server)
            .await;

        session.fetch_user_profile_all("123456").await.unwrap();
    }

    #[tokio::test]
    async fn current_user_homepage_falls_back_to_html_user_id() {
        let server = MockServer::start().await;
        let session = PixivNetSession::new_with_base_url(
            options(5, 2),
            Credential::new("cookie").unwrap(),
            server.uri().parse().unwrap(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html><body>\"userId\":\"12345678\"</body></html>"),
            )
            .mount(&server)
            .await;

        let page = session.fetch_current_user_homepage().await.unwrap();
        assert_eq!(page.header_user_id.as_deref(), Some("12345678"));
    }

    #[tokio::test]
    async fn session_exposes_stable_identity() {
        let session = PixivNetSession::new_with_base_url(
            options(5, 2),
            Credential::new("cookie").unwrap(),
            Url::parse("https://www.pixiv.net").unwrap(),
        )
        .unwrap();
        assert!(session.session_id() > 0);
    }

    #[tokio::test]
    async fn stream_download_failure_cleans_part_file() {
        let server = MockServer::start().await;
        let session = PixivNetSession::new_with_base_url(
            options(5, 1),
            Credential::new("cookie").unwrap(),
            server.uri().parse().unwrap(),
        )
        .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let target_path = temp.path().join("123456_p0.png");

        Mock::given(method("GET"))
            .and(path("/img-original/bad.png"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let error = session
            .download_original_image(
                &format!("{}/img-original/bad.png", server.uri()),
                "123456",
                &target_path,
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("500"));
        assert!(!target_path.exists());
        assert!(!temp.path().join("123456_p0.png.part").exists());
    }
}
