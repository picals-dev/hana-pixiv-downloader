//! 凭据读取与写入。

use std::{fs, path::Path};

use eyre::{Context, eyre};
use serde::{Deserialize, Serialize};

use crate::{
    config::{credential_file_path, ensure_config_dir},
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    pub phpsessid: String,
}

impl Credential {
    pub fn new(phpsessid: impl Into<String>) -> AppResult<Self> {
        let phpsessid = phpsessid.into().trim().to_string();

        if phpsessid.is_empty() {
            return Err(eyre!(CrawlerError::Auth("PHPSESSID 不能为空".to_string())));
        }

        Ok(Self { phpsessid })
    }

    pub fn load() -> AppResult<Option<Self>> {
        let path = credential_file_path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> AppResult<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("读取凭据文件失败: {}", path.display()))?;

        let credential = toml::from_str(&content)?;
        Ok(Some(credential))
    }

    pub fn save(&self) -> AppResult<()> {
        let dir = ensure_config_dir()?;
        let path = dir.join("credentials");
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建凭据目录失败: {}", parent.display()))?;
        }

        let content = toml::to_string(self)?;
        fs::write(path, content)
            .with_context(|| format!("写入凭据文件失败: {}", path.display()))?;

        set_secure_permissions(path)?;
        Ok(())
    }

    pub fn exists() -> bool {
        credential_file_path().is_ok_and(|path| path.exists())
    }

    pub fn cookie_header(&self) -> String {
        format!("PHPSESSID={}", self.phpsessid)
    }

    pub fn masked(&self) -> String {
        let prefix: String = self.phpsessid.chars().take(3).collect();
        format!("{prefix}***")
    }
}

#[cfg(unix)]
fn set_secure_permissions(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("设置凭据文件权限失败: {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_secure_permissions(_path: &Path) -> AppResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::Credential;

    #[test]
    fn credential_roundtrip_works() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("credentials");
        let credential = Credential::new("test-cookie").unwrap();

        credential.save_to(&path).unwrap();
        let loaded = Credential::load_from(&path).unwrap().unwrap();

        assert_eq!(loaded, credential);
        assert_eq!(loaded.cookie_header(), "PHPSESSID=test-cookie");
    }
}
