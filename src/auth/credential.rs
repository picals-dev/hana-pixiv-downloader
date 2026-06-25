//! 凭据读取与写入。

use std::{fs, path::Path};

use eyre::{Context, eyre};
use serde::{Deserialize, Serialize};

use crate::{
    config::{credential_file_path, ensure_config_dir},
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Credential {
    pub phpsessid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct StoredCredential {
    phpsessid: String,
    #[serde(default)]
    user_id: Option<String>,
}

impl Credential {
    pub fn new(phpsessid: impl Into<String>) -> AppResult<Self> {
        Self::new_with_user_id(phpsessid, Option::<String>::None)
    }

    pub fn new_with_user_id<S, T>(phpsessid: S, user_id: Option<T>) -> AppResult<Self>
    where
        S: Into<String>,
        T: Into<String>,
    {
        Self::from_parts(phpsessid.into(), user_id.map(Into::into))
    }

    pub(crate) fn load() -> AppResult<Option<Self>> {
        let path = credential_file_path()?;
        Self::load_from(&path)
    }

    pub(crate) fn load_from(path: &Path) -> AppResult<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("读取凭据文件失败: {}", path.display()))?;

        let stored = toml::from_str::<StoredCredential>(&content)?;
        let credential = Self::from_parts(stored.phpsessid, stored.user_id)?;
        Ok(Some(credential))
    }

    pub fn save(&self) -> AppResult<()> {
        let dir = ensure_config_dir()?;
        let path = dir.join("credentials");
        self.save_to(&path)
    }

    pub(crate) fn save_to(&self, path: &Path) -> AppResult<()> {
        self.validate()?;

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

    pub(crate) fn user_id(&self) -> Option<&str> {
        self.user_id.as_deref()
    }

    pub(crate) fn require_user_id(&self) -> AppResult<&str> {
        self.user_id().ok_or(CrawlerError::MissingUserId.into())
    }

    pub(crate) fn set_phpsessid(&mut self, phpsessid: impl Into<String>) -> AppResult<()> {
        self.phpsessid = normalize_phpsessid(phpsessid.into())?;
        self.validate()
    }

    pub(crate) fn set_user_id<T>(&mut self, user_id: Option<T>) -> AppResult<()>
    where
        T: Into<String>,
    {
        self.user_id = normalize_user_id(user_id.map(Into::into))?;
        self.validate()
    }

    pub(crate) fn parse_user_id(input: &str) -> AppResult<String> {
        normalize_user_id(Some(input.to_string()))?
            .ok_or_else(|| eyre!(CrawlerError::Auth("userId 不能为空".to_string())))
    }

    fn from_parts(phpsessid: String, user_id: Option<String>) -> AppResult<Self> {
        let credential = Self {
            phpsessid: normalize_phpsessid(phpsessid)?,
            user_id: normalize_user_id(user_id)?,
        };
        credential.validate()?;
        Ok(credential)
    }

    fn validate(&self) -> AppResult<()> {
        normalize_phpsessid(self.phpsessid.clone())?;

        if let Some(user_id) = self.user_id.as_deref() {
            Self::parse_user_id(user_id)?;
        }

        Ok(())
    }
}

fn normalize_phpsessid(phpsessid: String) -> AppResult<String> {
    let phpsessid = phpsessid.trim().to_string();

    if phpsessid.is_empty() {
        return Err(eyre!(CrawlerError::Auth("PHPSESSID 不能为空".to_string())));
    }

    Ok(phpsessid)
}

fn normalize_user_id(user_id: Option<String>) -> AppResult<Option<String>> {
    match user_id {
        Some(user_id) => {
            let trimmed = user_id.trim();

            if trimmed.is_empty() {
                return Err(eyre!(CrawlerError::Auth("userId 不能为空".to_string())));
            }

            if !trimmed.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(eyre!(CrawlerError::Auth("userId 必须是纯数字".to_string())));
            }

            Ok(Some(trimmed.to_string()))
        }
        None => Ok(None),
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
        let credential = Credential::new_with_user_id("test-cookie", Some("12345678")).unwrap();

        credential.save_to(&path).unwrap();
        let loaded = Credential::load_from(&path).unwrap().unwrap();

        assert_eq!(loaded, credential);
        assert_eq!(loaded.phpsessid, "test-cookie");
        assert_eq!(loaded.user_id(), Some("12345678"));
    }

    #[test]
    fn old_credential_format_can_still_be_loaded() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("credentials");
        std::fs::write(&path, "phpsessid = \"legacy-cookie\"\n").unwrap();

        let loaded = Credential::load_from(&path).unwrap().unwrap();

        assert_eq!(loaded.phpsessid, "legacy-cookie");
        assert_eq!(loaded.user_id(), None);
    }

    #[test]
    fn invalid_user_id_is_rejected() {
        let error = Credential::new_with_user_id("cookie", Some("not-a-number")).unwrap_err();
        assert!(format!("{error:#}").contains("userId 必须是纯数字"));
    }

    #[test]
    fn invalid_user_id_in_file_is_rejected() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("credentials");
        std::fs::write(&path, "phpsessid = \"cookie\"\nuser_id = \"bad-user-id\"\n").unwrap();

        let error = Credential::load_from(&path).unwrap_err();
        assert!(format!("{error:#}").contains("userId 必须是纯数字"));
    }
}
