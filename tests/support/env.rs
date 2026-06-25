use std::path::Path;

use hana_pixiv_downloader::{net::SessionObserver, test_support::install_session_observer};
use tokio::sync::MutexGuard;

pub use hana_pixiv_downloader::test_support::{ConfigHomeGuard, EnvVarGuard, SessionObserverGuard};

pub struct DownloadEnvGuards {
    pub _sort: EnvVarGuard,
    pub _ai: EnvVarGuard,
    pub _r18: EnvVarGuard,
}

pub async fn lock_env() -> MutexGuard<'static, ()> {
    hana_pixiv_downloader::test_support::lock_env().await
}

pub fn set_config_home(home: &Path) -> ConfigHomeGuard {
    hana_pixiv_downloader::test_support::set_config_home(home)
}

pub fn unset_download_env() -> DownloadEnvGuards {
    DownloadEnvGuards {
        _sort: EnvVarGuard::unset("HPD_DOWNLOAD_SORT"),
        _ai: EnvVarGuard::unset("HPD_DOWNLOAD_AI"),
        _r18: EnvVarGuard::unset("HPD_DOWNLOAD_R18"),
    }
}

pub fn set_base_url(base_url: &str) -> EnvVarGuard {
    EnvVarGuard::set("HPD_PIXIV_BASE_URL", base_url)
}

pub fn install_observer(observer: SessionObserver) -> SessionObserverGuard {
    install_session_observer(observer)
}
