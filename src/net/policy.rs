//! 请求策略与退避计算。

use std::time::{Duration, SystemTime};

use reqwest::{StatusCode, header::HeaderMap};

use crate::error::CrawlerError;

use super::RequestKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RequestPolicy {
    pub timeout: Duration,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub default_cooldown: Duration,
}

pub(crate) fn policy_for(kind: RequestKind) -> RequestPolicy {
    match kind {
        RequestKind::ImageDownload | RequestKind::UgoiraDownload => RequestPolicy {
            timeout: Duration::from_secs(90),
            base_delay: Duration::from_millis(300),
            max_delay: Duration::from_secs(8),
            default_cooldown: Duration::from_secs(5),
        },
        RequestKind::Homepage
        | RequestKind::UserProfile
        | RequestKind::IllustPages
        | RequestKind::IllustDetail
        | RequestKind::UgoiraMeta
        | RequestKind::KeywordSearch
        | RequestKind::Ranking
        | RequestKind::Bookmark => RequestPolicy {
            timeout: Duration::from_secs(30),
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
            default_cooldown: Duration::from_secs(3),
        },
    }
}

pub(crate) fn retry_delay_for_status(
    kind: RequestKind,
    attempt: usize,
    attempt_limit: usize,
    status: StatusCode,
    headers: &HeaderMap,
    now: SystemTime,
) -> Option<Duration> {
    if attempt + 1 >= attempt_limit {
        return None;
    }

    let policy = policy_for(kind);
    match status {
        StatusCode::TOO_MANY_REQUESTS => {
            Some(parse_retry_after(headers, now).unwrap_or(policy.default_cooldown))
        }
        status if is_retryable_http_status(status) => Some(
            parse_retry_after(headers, now).unwrap_or_else(|| compute_backoff_delay(kind, attempt)),
        ),
        _ => None,
    }
}

pub(crate) fn is_retryable_http_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_EARLY
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

pub(crate) fn is_cooldown_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
}

pub(crate) fn retry_delay_for_error(
    kind: RequestKind,
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
        return Some(compute_backoff_delay(kind, attempt));
    }

    if let Some(CrawlerError::DownloadInterrupted(_)) = error.downcast_ref::<CrawlerError>() {
        return Some(compute_backoff_delay(kind, attempt));
    }

    None
}

pub(crate) fn compute_backoff_delay(kind: RequestKind, attempt: usize) -> Duration {
    let policy = policy_for(kind);
    let multiplier = 1u32 << attempt.min(5);
    let base_ms = policy.base_delay.as_millis() as u64;
    let raw_ms = base_ms.saturating_mul(multiplier as u64);
    let capped_ms = raw_ms.min(policy.max_delay.as_millis() as u64);
    let jitter_ms = ((attempt as u64 + 1) * 37 + stable_kind_seed(kind) * 17) % 97;
    Duration::from_millis(capped_ms.saturating_add(jitter_ms))
}

pub(crate) fn parse_retry_after(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    let value = headers.get("Retry-After")?.to_str().ok()?.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let timestamp = jiff::fmt::rfc2822::DateTimeParser::new()
        .parse_timestamp(value)
        .ok()?;
    let retry_at: SystemTime = timestamp.into();
    retry_at.duration_since(now).ok()
}

fn stable_kind_seed(kind: RequestKind) -> u64 {
    match kind {
        RequestKind::Homepage => 1,
        RequestKind::UserProfile => 2,
        RequestKind::IllustPages => 3,
        RequestKind::IllustDetail => 4,
        RequestKind::UgoiraMeta => 5,
        RequestKind::KeywordSearch => 6,
        RequestKind::Ranking => 7,
        RequestKind::Bookmark => 8,
        RequestKind::ImageDownload => 9,
        RequestKind::UgoiraDownload => 10,
    }
}

pub(crate) fn host_timeout(kind: RequestKind) -> Duration {
    policy_for(kind).timeout
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use reqwest::{StatusCode, header::HeaderMap};

    use super::{
        compute_backoff_delay, is_cooldown_status, is_retryable_http_status, parse_retry_after,
        policy_for, retry_delay_for_status,
    };
    use crate::net::RequestKind;

    #[test]
    fn metadata_and_image_policies_are_split() {
        assert!(
            policy_for(RequestKind::ImageDownload).timeout
                > policy_for(RequestKind::IllustPages).timeout
        );
        assert!(
            policy_for(RequestKind::ImageDownload).base_delay
                > policy_for(RequestKind::IllustPages).base_delay
        );
    }

    #[test]
    fn retry_after_supports_delta_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert("Retry-After", "3".parse().unwrap());
        assert_eq!(
            parse_retry_after(&headers, std::time::SystemTime::UNIX_EPOCH),
            Some(Duration::from_secs(3))
        );
    }

    #[test]
    fn backoff_changes_with_attempt() {
        let first = compute_backoff_delay(RequestKind::ImageDownload, 0);
        let second = compute_backoff_delay(RequestKind::ImageDownload, 1);
        assert!(second > first);
    }

    #[test]
    fn retryable_http_statuses_follow_policy() {
        assert!(is_retryable_http_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_http_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!is_retryable_http_status(StatusCode::NOT_IMPLEMENTED));
    }

    #[test]
    fn only_429_triggers_cooldown() {
        assert!(is_cooldown_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(!is_cooldown_status(StatusCode::SERVICE_UNAVAILABLE));
    }

    #[test]
    fn non_retryable_http_status_has_no_retry_delay() {
        let headers = HeaderMap::new();
        assert_eq!(
            retry_delay_for_status(
                RequestKind::IllustPages,
                0,
                3,
                StatusCode::NOT_IMPLEMENTED,
                &headers,
                std::time::SystemTime::UNIX_EPOCH,
            ),
            None
        );
    }
}
