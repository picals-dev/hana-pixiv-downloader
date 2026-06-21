//! 测试期共享的环境变量隔离工具。

use std::path::Path;
use std::sync::LazyLock;

use tokio::sync::{Mutex, MutexGuard};

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    pub fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }

    pub fn unset(key: &'static str) -> Self {
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => unsafe {
                std::env::set_var(self.key, value);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

pub async fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().await
}

pub struct ConfigHomeGuard {
    _home: EnvVarGuard,
    _xdg: EnvVarGuard,
}

pub fn set_config_home(home: &Path) -> ConfigHomeGuard {
    let xdg = home.join(".config");
    ConfigHomeGuard {
        _home: EnvVarGuard::set("HOME", home),
        _xdg: EnvVarGuard::set("XDG_CONFIG_HOME", &xdg),
    }
}
