//! 网络事件定义。

use std::{sync::Arc, time::Duration};

use super::{HostKind, RequestKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetEvent {
    Attempt {
        session_id: u64,
        host: HostKind,
        kind: RequestKind,
        attempt: usize,
        url: String,
    },
    Retry {
        session_id: u64,
        host: HostKind,
        kind: RequestKind,
        attempt: usize,
        delay: Duration,
        reason: String,
    },
    Failure {
        session_id: u64,
        host: HostKind,
        kind: RequestKind,
        attempts: usize,
        reason: String,
    },
    Cooldown {
        session_id: u64,
        host: HostKind,
        delay: Duration,
    },
    TransferCompleted {
        session_id: u64,
        bytes: u64,
        target_path: String,
    },
    TransferProgress {
        session_id: u64,
        bytes: u64,
        target_path: String,
    },
}

pub type SessionObserver = Arc<dyn Fn(NetEvent) + Send + Sync>;
