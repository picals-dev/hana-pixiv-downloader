//! Pixiv 请求运行时。

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, SystemTime},
};

use eyre::eyre;
use jiff::fmt::rfc2822;
use log::warn;
use reqwest::{
    Client, Proxy,
    header::{COOKIE, HeaderMap, HeaderValue, REFERER, USER_AGENT},
};
use serde_json::Value;
use tokio::sync::Mutex as AsyncMutex;
use url::Url;

use crate::{
    auth::Credential,
    collector::DEFAULT_USER_AGENT,
    config::ResolvedDownloadOptions,
    error::{AppResult, CrawlerError},
};

type SleepFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type NowFn = Arc<dyn Fn() -> SystemTime + Send + Sync>;
type SleepFn = Arc<dyn Fn(Duration) -> SleepFuture + Send + Sync>;
type EventObserver = Arc<dyn Fn(RequestEvent) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestClass {
    Homepage,
    UserProfile,
    IllustPages,
    IllustDetail,
    KeywordSearch,
    Ranking,
    Bookmark,
    ImageDownload,
}

impl RequestClass {
    fn id(self) -> u64 {
        match self {
            Self::Homepage => 1,
            Self::UserProfile => 2,
            Self::IllustPages => 3,
            Self::IllustDetail => 4,
            Self::KeywordSearch => 5,
            Self::Ranking => 6,
            Self::Bookmark => 7,
            Self::ImageDownload => 8,
        }
    }

    fn label(self) -> &'static str {
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

    fn policy(self) -> RequestPolicy {
        match self {
            Self::ImageDownload => RequestPolicy {
                base_delay: Duration::from_millis(300),
                max_delay: Duration::from_secs(8),
                default_cooldown: Duration::from_secs(5),
            },
            Self::Homepage
            | Self::UserProfile
            | Self::IllustPages
            | Self::IllustDetail
            | Self::KeywordSearch
            | Self::Ranking
            | Self::Bookmark => RequestPolicy {
                base_delay: Duration::from_millis(200),
                max_delay: Duration::from_secs(5),
                default_cooldown: Duration::from_secs(3),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestEvent {
    Attempt {
        class: RequestClass,
        attempt: usize,
        url: String,
    },
    Retry {
        class: RequestClass,
        attempt: usize,
        delay: Duration,
        reason: String,
    },
    Failure {
        class: RequestClass,
        attempts: usize,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct TextResponse {
    pub headers: HeaderMap,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct BinaryResponse {
    pub content_length: Option<u64>,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct PixivRequestRuntime {
    client: Client,
    attempt_limit: usize,
    hooks: RuntimeHooks,
    state: Arc<AsyncMutex<RuntimeState>>,
}

impl std::fmt::Debug for PixivRequestRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PixivRequestRuntime")
            .field("attempt_limit", &self.attempt_limit)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct RuntimeHooks {
    now: NowFn,
    sleep: SleepFn,
    observer: Option<EventObserver>,
}

#[derive(Debug, Default)]
struct RuntimeState {
    cooldown_until: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy)]
struct RequestPolicy {
    base_delay: Duration,
    max_delay: Duration,
    default_cooldown: Duration,
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

impl PixivRequestRuntime {
    pub fn new(options: &ResolvedDownloadOptions, credential: &Credential) -> AppResult<Self> {
        Self::new_with_hooks(options, credential, RuntimeHooks::default())
    }

    fn new_with_hooks(
        options: &ResolvedDownloadOptions,
        credential: &Credential,
        hooks: RuntimeHooks,
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
            .timeout(Duration::from_secs(options.timeout))
            .user_agent(DEFAULT_USER_AGENT);

        if let Some(proxy_url) = options.proxy_url.as_deref() {
            builder = builder.proxy(
                Proxy::all(proxy_url)
                    .map_err(|error| CrawlerError::Config(format!("代理配置无效: {error}")))?,
            );
        }

        Ok(Self {
            client: builder.build()?,
            attempt_limit: options.retry.max(1),
            hooks,
            state: Arc::new(AsyncMutex::new(RuntimeState::default())),
        })
    }

    pub async fn get_json(
        &self,
        url: Url,
        referer: Option<String>,
        class: RequestClass,
    ) -> AppResult<Value> {
        self.execute(url, referer, class, |response| async move {
            Ok(response.json::<Value>().await?)
        })
        .await
    }

    pub async fn get_text(
        &self,
        url: Url,
        referer: Option<String>,
        class: RequestClass,
    ) -> AppResult<TextResponse> {
        self.execute(url, referer, class, |response| async move {
            let headers = response.headers().clone();
            let body = response.text().await?;
            Ok(TextResponse { headers, body })
        })
        .await
    }

    pub async fn get_bytes(
        &self,
        url: Url,
        referer: Option<String>,
        class: RequestClass,
    ) -> AppResult<BinaryResponse> {
        self.execute(url, referer, class, |response| async move {
            let content_length = response.content_length();
            let bytes = response.bytes().await?;
            if bytes.is_empty() {
                return Err(eyre!(CrawlerError::DownloadInterrupted(
                    "下载到空文件".to_string()
                )));
            }

            if let Some(expected_length) = content_length
                && bytes.len() as u64 != expected_length
            {
                return Err(eyre!(CrawlerError::DownloadInterrupted(
                    "下载内容长度不匹配".to_string()
                )));
            }

            Ok(BinaryResponse {
                content_length,
                bytes: bytes.to_vec(),
            })
        })
        .await
    }

    async fn execute<T, P, Fut>(
        &self,
        url: Url,
        referer: Option<String>,
        class: RequestClass,
        parser: P,
    ) -> AppResult<T>
    where
        P: Fn(reqwest::Response) -> Fut,
        Fut: Future<Output = AppResult<T>>,
    {
        let policy = class.policy();
        let url_string = url.to_string();

        for attempt in 0..self.attempt_limit {
            self.enforce_cooldown().await;
            self.emit(RequestEvent::Attempt {
                class,
                attempt: attempt + 1,
                url: url_string.clone(),
            });

            let mut request = self.client.get(url.clone());
            if let Some(referer) = referer.clone() {
                request = request.header(REFERER, referer);
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        match parser(response).await {
                            Ok(value) => return Ok(value),
                            Err(error) => {
                                if let Some(delay) = retry_delay_for_error(
                                    class,
                                    attempt,
                                    self.attempt_limit,
                                    &error,
                                ) {
                                    self.emit(RequestEvent::Retry {
                                        class,
                                        attempt: attempt + 1,
                                        delay,
                                        reason: error.to_string(),
                                    });
                                    (self.hooks.sleep)(delay).await;
                                    continue;
                                }

                                self.emit(RequestEvent::Failure {
                                    class,
                                    attempts: attempt + 1,
                                    reason: error.to_string(),
                                });
                                return Err(error);
                            }
                        }
                    }

                    let headers = response.headers().clone();
                    if let Some(delay) = retry_delay_for_status(
                        class,
                        attempt,
                        self.attempt_limit,
                        status,
                        &headers,
                        policy,
                        (self.hooks.now)(),
                    ) {
                        if status.as_u16() == 429 {
                            self.set_cooldown(delay).await;
                        }
                        self.emit(RequestEvent::Retry {
                            class,
                            attempt: attempt + 1,
                            delay,
                            reason: format!("HTTP {}", status.as_u16()),
                        });
                        (self.hooks.sleep)(delay).await;
                        continue;
                    }

                    let error = eyre!(CrawlerError::HttpStatus {
                        status: status.as_u16(),
                        context: format!("请求 {} 失败，URL: {}", class.label(), url_string),
                    });
                    self.emit(RequestEvent::Failure {
                        class,
                        attempts: attempt + 1,
                        reason: error.to_string(),
                    });
                    return Err(error);
                }
                Err(error) => {
                    let report = eyre!(error);
                    if let Some(delay) =
                        retry_delay_for_error(class, attempt, self.attempt_limit, &report)
                    {
                        self.emit(RequestEvent::Retry {
                            class,
                            attempt: attempt + 1,
                            delay,
                            reason: report.to_string(),
                        });
                        (self.hooks.sleep)(delay).await;
                        continue;
                    }

                    self.emit(RequestEvent::Failure {
                        class,
                        attempts: attempt + 1,
                        reason: report.to_string(),
                    });
                    return Err(report);
                }
            }
        }

        unreachable!("请求运行时至少会执行一次请求")
    }

    async fn enforce_cooldown(&self) {
        let wait_duration = {
            let mut state = self.state.lock().await;
            let Some(deadline) = state.cooldown_until else {
                return;
            };
            let now = (self.hooks.now)();
            if deadline <= now {
                state.cooldown_until = None;
                return;
            }
            deadline.duration_since(now).unwrap_or_default()
        };

        (self.hooks.sleep)(wait_duration).await;
    }

    async fn set_cooldown(&self, delay: Duration) {
        let mut state = self.state.lock().await;
        let deadline = (self.hooks.now)() + delay;
        state.cooldown_until = match state.cooldown_until {
            Some(existing) if existing > deadline => Some(existing),
            _ => Some(deadline),
        };
    }

    fn emit(&self, event: RequestEvent) {
        match &event {
            RequestEvent::Retry {
                class,
                attempt,
                delay,
                reason,
            } => {
                warn!(
                    "request.retry class={} attempt={} delay_ms={} reason={}",
                    class.label(),
                    attempt,
                    delay.as_millis(),
                    reason
                );
            }
            RequestEvent::Failure {
                class,
                attempts,
                reason,
            } => {
                warn!(
                    "request.failure class={} attempts={} reason={}",
                    class.label(),
                    attempts,
                    reason
                );
            }
            _ => {}
        }

        if let Some(observer) = &self.hooks.observer {
            observer(event);
        }
    }
}

fn retry_delay_for_status(
    class: RequestClass,
    attempt: usize,
    attempt_limit: usize,
    status: reqwest::StatusCode,
    headers: &HeaderMap,
    policy: RequestPolicy,
    now: SystemTime,
) -> Option<Duration> {
    if attempt + 1 >= attempt_limit {
        return None;
    }

    match status.as_u16() {
        429 => Some(parse_retry_after(headers, now).unwrap_or(policy.default_cooldown)),
        408 | 425 | 500 | 502 | 503 | 504 => Some(
            parse_retry_after(headers, now)
                .unwrap_or_else(|| compute_backoff_delay(class, attempt, policy)),
        ),
        _ => None,
    }
}

fn retry_delay_for_error(
    class: RequestClass,
    attempt: usize,
    attempt_limit: usize,
    error: &eyre::Report,
) -> Option<Duration> {
    if attempt + 1 >= attempt_limit {
        return None;
    }

    if let Some(reqwest_error) = error.downcast_ref::<reqwest::Error>()
        && (reqwest_error.is_timeout() || reqwest_error.is_connect() || reqwest_error.is_request())
    {
        return Some(compute_backoff_delay(class, attempt, class.policy()));
    }

    if let Some(CrawlerError::DownloadInterrupted(_)) = error.downcast_ref::<CrawlerError>() {
        return Some(compute_backoff_delay(class, attempt, class.policy()));
    }

    None
}

fn compute_backoff_delay(class: RequestClass, attempt: usize, policy: RequestPolicy) -> Duration {
    let multiplier = 1u32 << attempt.min(5);
    let base_ms = policy.base_delay.as_millis() as u64;
    let raw_ms = base_ms.saturating_mul(multiplier as u64);
    let capped_ms = raw_ms.min(policy.max_delay.as_millis() as u64);
    let jitter_ms = ((attempt as u64 + 1) * 37 + class.id() * 17) % 97;
    Duration::from_millis(capped_ms.saturating_add(jitter_ms))
}

fn parse_retry_after(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    let value = headers.get("Retry-After")?.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let timestamp = rfc2822::DateTimeParser::new().parse_timestamp(value).ok()?;
    let retry_at: SystemTime = timestamp.into();
    retry_at.duration_since(now).ok()
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
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::{
        auth::Credential,
        config::{DownloadConfig, DownloadMode, ResolvedDownloadOptions, SortOrder},
    };

    use super::{
        PixivRequestRuntime, RequestClass, RequestEvent, RuntimeHooks, compute_backoff_delay,
        parse_retry_after,
    };

    #[derive(Clone)]
    struct ManualClock {
        now: Arc<Mutex<SystemTime>>,
        sleeps: Arc<Mutex<Vec<Duration>>>,
        events: Arc<Mutex<Vec<RequestEvent>>>,
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
    fn retry_after_supports_delta_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert("Retry-After", "3".parse().unwrap());

        let delay = parse_retry_after(&headers, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(delay, Duration::from_secs(3));
    }

    #[test]
    fn retry_after_supports_http_date() {
        let timestamp = Timestamp::from_second(10).unwrap();
        let header_value = DateTimePrinter::new()
            .timestamp_to_rfc9110_string(&timestamp)
            .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("Retry-After", header_value.parse().unwrap());

        let delay = parse_retry_after(&headers, SystemTime::UNIX_EPOCH).unwrap();
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[test]
    fn exponential_backoff_includes_jitter() {
        let delay1 = compute_backoff_delay(
            RequestClass::ImageDownload,
            0,
            RequestClass::ImageDownload.policy(),
        );
        let delay2 = compute_backoff_delay(
            RequestClass::ImageDownload,
            1,
            RequestClass::ImageDownload.policy(),
        );

        assert!(delay1 >= Duration::from_millis(300));
        assert!(delay2 > delay1);
    }

    #[tokio::test]
    async fn runtime_retries_429_with_retry_after_cooldown() {
        let server = MockServer::start().await;
        let clock = ManualClock::new(SystemTime::UNIX_EPOCH);
        let runtime = PixivRequestRuntime::new_with_hooks(
            &options(5, 2),
            &Credential::new("cookie").unwrap(),
            clock.hooks(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/cooldown"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "2"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cooldown"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&server)
            .await;

        let url = Url::parse(&format!("{}/cooldown", server.uri())).unwrap();
        let response = runtime
            .get_json(url, None, RequestClass::IllustPages)
            .await
            .unwrap();

        assert_eq!(response["ok"], json!(true));
        assert_eq!(
            clock.sleeps.lock().unwrap().as_slice(),
            &[Duration::from_secs(2)]
        );
    }

    #[tokio::test]
    async fn runtime_retries_503_with_exponential_backoff() {
        let server = MockServer::start().await;
        let clock = ManualClock::new(SystemTime::UNIX_EPOCH);
        let runtime = PixivRequestRuntime::new_with_hooks(
            &options(5, 2),
            &Credential::new("cookie").unwrap(),
            clock.hooks(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/server-error"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/server-error"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&server)
            .await;

        let url = Url::parse(&format!("{}/server-error", server.uri())).unwrap();
        runtime
            .get_json(url, None, RequestClass::KeywordSearch)
            .await
            .unwrap();

        assert_eq!(
            clock.sleeps.lock().unwrap().as_slice(),
            &[compute_backoff_delay(
                RequestClass::KeywordSearch,
                0,
                RequestClass::KeywordSearch.policy()
            )]
        );
    }

    #[tokio::test]
    async fn runtime_retries_timeout_with_fresh_request() {
        let server = MockServer::start().await;
        let clock = ManualClock::new(SystemTime::UNIX_EPOCH);
        let runtime = PixivRequestRuntime::new_with_hooks(
            &options(1, 2),
            &Credential::new("cookie").unwrap(),
            clock.hooks(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/timeout"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(2))
                    .set_body_json(json!({"slow": true})),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/timeout"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&server)
            .await;

        let url = Url::parse(&format!("{}/timeout", server.uri())).unwrap();
        let response = runtime
            .get_json(url, None, RequestClass::UserProfile)
            .await
            .unwrap();

        assert_eq!(response["ok"], json!(true));
        assert_eq!(clock.sleeps.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn runtime_does_not_retry_401() {
        let server = MockServer::start().await;
        let clock = ManualClock::new(SystemTime::UNIX_EPOCH);
        let runtime = PixivRequestRuntime::new_with_hooks(
            &options(5, 3),
            &Credential::new("cookie").unwrap(),
            clock.hooks(),
        )
        .unwrap();

        Mock::given(method("GET"))
            .and(path("/auth-fail"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;

        let url = Url::parse(&format!("{}/auth-fail", server.uri())).unwrap();
        let error = runtime
            .get_json(url, None, RequestClass::Homepage)
            .await
            .unwrap_err();

        assert!(format!("{error:#}").contains("401"));
        assert!(clock.sleeps.lock().unwrap().is_empty());
    }
}
