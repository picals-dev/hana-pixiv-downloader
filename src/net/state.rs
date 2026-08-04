//! 共享会话状态：429 冷却与升级。

use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use tokio::sync::Mutex as AsyncMutex;

use super::HostKind;

/// 连续 429 风暴下的冷却升级上限：冷却时长最多升到基数的 `1 << MAX_429_STRIKES` 倍。
const MAX_429_STRIKES: u32 = 4;
/// 距上次 429 超过该时长才视为一次全新事件，冷却级别归零。
const STRIKE_RESET_AFTER: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy)]
struct CooldownEntry {
    deadline: SystemTime,
    strikes: u32,
    last_429_at: SystemTime,
}

#[derive(Debug, Default)]
pub(crate) struct SharedState {
    cooldowns: AsyncMutex<HashMap<HostKind, CooldownEntry>>,
}

impl SharedState {
    pub(crate) async fn cooldown_remaining(
        &self,
        host: HostKind,
        now: SystemTime,
    ) -> Option<Duration> {
        let cooldowns = self.cooldowns.lock().await;
        let entry = cooldowns.get(&host).copied()?;
        if entry.deadline <= now {
            // 到期不删除条目：保留 strikes / last_429_at，供后续 429 判断是否升级为持久风暴。
            return None;
        }
        entry.deadline.duration_since(now).ok()
    }

    /// 一次 429 的冷却记账，返回本次实际应等待的时长。
    ///
    /// - 若当前仍在冷却期内（并发任务刚触发过 429），不重复升级，
    ///   直接返回剩余时长，让并发任务对齐同一个 deadline，避免惊群；
    /// - 冷却期已过仍收到 429（风暴持续），冷却级别 +1，时长翻倍升级；
    /// - `explicit`（`Retry-After`）存在时优先采用，不参与升级。
    pub(crate) async fn enter_429_cooldown(
        &self,
        host: HostKind,
        now: SystemTime,
        explicit: Option<Duration>,
        base: Duration,
    ) -> Duration {
        let mut cooldowns = self.cooldowns.lock().await;

        if let Some(entry) = cooldowns.get(&host).copied()
            && entry.deadline > now
        {
            let remaining = entry.deadline.duration_since(now).unwrap_or(Duration::ZERO);
            return remaining;
        }

        let strikes = match cooldowns.get(&host).copied() {
            Some(entry) if now <= entry.last_429_at + STRIKE_RESET_AFTER => {
                (entry.strikes + 1).min(MAX_429_STRIKES)
            }
            _ => 0,
        };

        let adaptive = base.saturating_mul(1u32 << strikes);
        let delay = explicit.unwrap_or(adaptive);
        let deadline = now + delay;

        cooldowns.insert(
            host,
            CooldownEntry {
                deadline,
                strikes,
                last_429_at: now,
            },
        );
        delay
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::SharedState;
    use crate::net::HostKind;

    #[tokio::test]
    async fn cooldowns_are_independent_per_host() {
        let state = SharedState::default();
        let now = SystemTime::UNIX_EPOCH;
        let first_delay = state
            .enter_429_cooldown(HostKind::Metadata, now, None, Duration::from_secs(3))
            .await;
        assert_eq!(first_delay, Duration::from_secs(3));

        assert_eq!(
            state.cooldown_remaining(HostKind::Metadata, now).await,
            Some(Duration::from_secs(3))
        );
        assert_eq!(state.cooldown_remaining(HostKind::Image, now).await, None);
    }

    #[tokio::test]
    async fn concurrent_429_in_window_aligns_on_remaining() {
        let state = SharedState::default();
        let now = SystemTime::UNIX_EPOCH;
        let base = Duration::from_secs(30);
        state
            .enter_429_cooldown(HostKind::Metadata, now, None, base)
            .await;

        // 同一窗口内的并发 429 不升级，直接对齐剩余时长。
        let remaining = state
            .enter_429_cooldown(HostKind::Metadata, now + Duration::from_secs(1), None, base)
            .await;
        assert_eq!(remaining, Duration::from_secs(29));
    }

    #[tokio::test]
    async fn cooldown_escalates_on_persistent_429() {
        let state = SharedState::default();
        let now = SystemTime::UNIX_EPOCH;
        let base = Duration::from_secs(30);

        let first = state
            .enter_429_cooldown(HostKind::Metadata, now, None, base)
            .await;
        assert_eq!(first, Duration::from_secs(30));

        // 冷却期过后仍收到 429 → 升级为 60s。
        let second = state
            .enter_429_cooldown(
                HostKind::Metadata,
                now + Duration::from_secs(31),
                None,
                base,
            )
            .await;
        assert_eq!(second, Duration::from_secs(60));

        // 无关事件时间差较远 → 级别归零，回到基数。
        let fresh = state
            .enter_429_cooldown(
                HostKind::Metadata,
                now + Duration::from_secs(31 + 301),
                None,
                base,
            )
            .await;
        assert_eq!(fresh, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn explicit_retry_after_is_preferred() {
        let state = SharedState::default();
        let now = SystemTime::UNIX_EPOCH;

        let delay = state
            .enter_429_cooldown(
                HostKind::Metadata,
                now,
                Some(Duration::from_secs(2)),
                Duration::from_secs(30),
            )
            .await;
        assert_eq!(delay, Duration::from_secs(2));
    }
}
