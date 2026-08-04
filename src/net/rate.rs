//! 请求速率限制（令牌桶）。
//!
//! 只约束请求的**发起节奏**，不约束并发的在途请求数量：
//! Pixiv 的 429 是按账号/窗口配额计数的，`concurrent` 只决定"同一时刻最多几个在途"，
//! 决定不了"每秒发起几个"。批量抓取时若多个请求几乎同时发出，就会瞬间击穿配额。
//! 令牌桶让请求按固定速率逐个放行，从源头避免 429。

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

use super::HostKind;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RateLimitParams {
    /// 桶容量（允许的突发请求数）。
    pub capacity: f64,
    /// 每秒补充的令牌数（持续请求速率）。
    pub per_second: f64,
}

/// 各 host 的默认限速参数。
pub(crate) fn rate_limit_for(host: HostKind) -> RateLimitParams {
    match host {
        // 元数据接口按账号配额计数，必须更保守，避免批量抓取瞬间击穿配额。
        HostKind::Metadata => RateLimitParams {
            capacity: 4.0,
            per_second: 1.5,
        },
        // 图片 CDN 配额更宽松，主要防止瞬时连接数打满。
        HostKind::Image => RateLimitParams {
            capacity: 12.0,
            per_second: 8.0,
        },
    }
}

/// 两个 host 各持一个令牌桶，共享在 session 上供所有并发请求复用。
#[derive(Debug)]
pub(crate) struct RateBuckets {
    metadata: TokenBucket,
    image: TokenBucket,
}

impl RateBuckets {
    pub(crate) fn new(overrides: &HashMap<HostKind, RateLimitParams>) -> Self {
        let param = |host: HostKind| {
            overrides
                .get(&host)
                .copied()
                .unwrap_or_else(|| rate_limit_for(host))
        };
        Self {
            metadata: TokenBucket::new(param(HostKind::Metadata)),
            image: TokenBucket::new(param(HostKind::Image)),
        }
    }

    /// 阻塞直到该 host 允许发起一个新请求。
    pub(crate) async fn acquire(&self, host: HostKind) {
        match host {
            HostKind::Metadata => self.metadata.acquire().await,
            HostKind::Image => self.image.acquire().await,
        }
    }
}

#[derive(Debug)]
struct TokenBucket {
    params: RateLimitParams,
    state: Mutex<BucketState>,
}

#[derive(Debug)]
struct BucketState {
    tokens: f64,
    refilled_at: Instant,
}

impl TokenBucket {
    fn new(params: RateLimitParams) -> Self {
        Self {
            params,
            state: Mutex::new(BucketState {
                tokens: params.capacity,
                refilled_at: Instant::now(),
            }),
        }
    }

    /// 阻塞直到拿到一个令牌。令牌代表"可以发起一个请求"。
    async fn acquire(&self) {
        loop {
            let mut state = self.state.lock().await;
            let now = Instant::now();
            let elapsed = now.duration_since(state.refilled_at).as_secs_f64();
            state.tokens =
                (state.tokens + elapsed * self.params.per_second).min(self.params.capacity);
            state.refilled_at = now;

            if state.tokens >= 1.0 {
                state.tokens -= 1.0;
                return;
            }

            let wait = Duration::from_secs_f64((1.0 - state.tokens) / self.params.per_second);
            drop(state);
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{RateLimitParams, TokenBucket};

    #[tokio::test]
    async fn burst_capacity_is_honored() {
        let bucket = TokenBucket::new(RateLimitParams {
            capacity: 4.0,
            per_second: 1000.0,
        });
        let start = Instant::now();
        for _ in 0..4 {
            bucket.acquire().await;
        }
        assert!(
            start.elapsed() < Duration::from_millis(20),
            "突发配额内不应等待，实际耗时 {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn exhausted_bucket_wait_for_refill() {
        let bucket = TokenBucket::new(RateLimitParams {
            capacity: 1.0,
            per_second: 100.0,
        });
        let start = Instant::now();
        bucket.acquire().await; // 立即
        bucket.acquire().await; // 需等 ~10ms 补一个令牌
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(5),
            "令牌耗尽后应等待，实际耗时 {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn rate_limits_sustained_throughput() {
        let bucket = TokenBucket::new(RateLimitParams {
            capacity: 2.0,
            per_second: 200.0,
        });
        let start = Instant::now();
        for _ in 0..6 {
            bucket.acquire().await;
        }
        // 前 2 个走突发配额，后 4 个至少各等 ~5ms。
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(15),
            "持续请求应被限速，实际耗时 {elapsed:?}"
        );
    }
}
