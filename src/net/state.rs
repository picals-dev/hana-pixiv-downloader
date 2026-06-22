//! 共享会话状态。

use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use tokio::sync::Mutex as AsyncMutex;

use super::HostKind;

#[derive(Debug, Default)]
pub struct SharedState {
    cooldowns: AsyncMutex<HashMap<HostKind, SystemTime>>,
}

impl SharedState {
    pub async fn cooldown_remaining(&self, host: HostKind, now: SystemTime) -> Option<Duration> {
        let mut cooldowns = self.cooldowns.lock().await;
        let deadline = cooldowns.get(&host).copied()?;
        if deadline <= now {
            cooldowns.remove(&host);
            return None;
        }
        deadline.duration_since(now).ok()
    }

    pub async fn extend_cooldown(&self, host: HostKind, now: SystemTime, delay: Duration) {
        let mut cooldowns = self.cooldowns.lock().await;
        let next_deadline = now + delay;
        match cooldowns.get(&host).copied() {
            Some(current) if current > next_deadline => {}
            _ => {
                cooldowns.insert(host, next_deadline);
            }
        }
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
        state
            .extend_cooldown(HostKind::Metadata, now, Duration::from_secs(3))
            .await;

        assert_eq!(
            state.cooldown_remaining(HostKind::Metadata, now).await,
            Some(Duration::from_secs(3))
        );
        assert_eq!(state.cooldown_remaining(HostKind::Image, now).await, None);
    }

    #[tokio::test]
    async fn longer_cooldown_wins() {
        let state = SharedState::default();
        let now = SystemTime::UNIX_EPOCH;
        state
            .extend_cooldown(HostKind::Metadata, now, Duration::from_secs(2))
            .await;
        state
            .extend_cooldown(HostKind::Metadata, now, Duration::from_secs(5))
            .await;

        assert_eq!(
            state.cooldown_remaining(HostKind::Metadata, now).await,
            Some(Duration::from_secs(5))
        );
    }
}
