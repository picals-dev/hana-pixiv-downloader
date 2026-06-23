//! 仅供测试注入的网络观察 hook。

use std::sync::{LazyLock, Mutex as StdMutex};

use super::{PixivNetSessionBuilder, SessionObserver};

static SESSION_OBSERVER: LazyLock<StdMutex<Option<SessionObserver>>> =
    LazyLock::new(|| StdMutex::new(None));

pub struct SessionObserverGuard {
    previous: Option<SessionObserver>,
}

impl Drop for SessionObserverGuard {
    fn drop(&mut self) {
        *SESSION_OBSERVER.lock().unwrap() = self.previous.take();
    }
}

pub fn install_session_observer(observer: SessionObserver) -> SessionObserverGuard {
    let mut slot = SESSION_OBSERVER.lock().unwrap();
    let previous = slot.replace(observer);
    SessionObserverGuard { previous }
}

pub(crate) fn attach_session_observer(builder: PixivNetSessionBuilder) -> PixivNetSessionBuilder {
    if let Some(observer) = current_session_observer() {
        builder.with_observer(observer)
    } else {
        builder
    }
}

pub(crate) fn current_session_observer() -> Option<SessionObserver> {
    SESSION_OBSERVER.lock().unwrap().clone()
}
