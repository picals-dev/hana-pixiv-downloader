use std::path::{Path, PathBuf};

use assert_cmd::Command;
use picals_crawler::auth::Credential;
use tempfile::{TempDir, tempdir};
use tokio::sync::MutexGuard;
use wiremock::MockServer;

use crate::support::env::{self, ConfigHomeGuard, DownloadEnvGuards, EnvVarGuard};

pub struct CliTestContext {
    temp: TempDir,
    base_url: String,
    _lock: MutexGuard<'static, ()>,
    _config_home: ConfigHomeGuard,
    _base_url: EnvVarGuard,
    _download_env: DownloadEnvGuards,
}

impl CliTestContext {
    pub async fn new(server: &MockServer) -> Self {
        let lock = env::lock_env().await;
        let temp = tempdir().unwrap();
        let config_home = env::set_config_home(temp.path());
        let base_url = server.uri();
        let base_url_guard = env::set_base_url(&base_url);
        let download_env = env::unset_download_env();
        Self {
            temp,
            base_url,
            _lock: lock,
            _config_home: config_home,
            _base_url: base_url_guard,
            _download_env: download_env,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::cargo_bin("picals-crawler").unwrap();
        command
            .env("HOME", self.home_dir())
            .env("XDG_CONFIG_HOME", self.xdg_config_home())
            .env("PICALS_PIXIV_BASE_URL", &self.base_url)
            .env_remove("PICALS_DOWNLOAD_SORT")
            .env_remove("PICALS_DOWNLOAD_AI")
            .env_remove("PICALS_DOWNLOAD_R18");
        command
    }

    pub fn home_dir(&self) -> &Path {
        self.temp.path()
    }

    pub fn xdg_config_home(&self) -> PathBuf {
        self.temp.path().join(".config")
    }

    pub fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.temp.path().join(relative)
    }

    pub fn write_credential(&self, credential: Credential) {
        credential.save().unwrap();
    }

    pub fn write_cookie_only_credential(&self) {
        self.write_credential(Credential::new("cookie").unwrap());
    }

    pub fn write_credential_with_user_id(&self, user_id: &str) {
        self.write_credential(Credential::new_with_user_id("cookie", Some(user_id)).unwrap());
    }
}
