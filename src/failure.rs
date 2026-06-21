//! 失败清单与回放模型。

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::{
    config::{DownloadMode, ResolvedDownloadOptions, ensure_config_dir},
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureStage {
    Collect,
    Download,
    Tags,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureRecord {
    pub mode: DownloadMode,
    pub stage: FailureStage,
    pub illust_id: Option<String>,
    pub image_url: Option<String>,
    pub target_path: Option<String>,
    pub error_kind: String,
    pub error_message: String,
    pub retryable: bool,
}

impl FailureRecord {
    pub fn from_report(
        mode: DownloadMode,
        stage: FailureStage,
        illust_id: Option<String>,
        image_url: Option<String>,
        target_path: Option<String>,
        error: &eyre::Report,
    ) -> Self {
        let classification = classify_error(error);
        Self {
            mode,
            stage,
            illust_id,
            image_url,
            target_path,
            error_kind: classification.error_kind,
            error_message: format!("{error:#}"),
            retryable: classification.retryable,
        }
    }

    pub fn from_crawler_error(
        mode: DownloadMode,
        stage: FailureStage,
        illust_id: Option<String>,
        image_url: Option<String>,
        target_path: Option<String>,
        error: &CrawlerError,
    ) -> Self {
        let classification = classify_crawler_error(error);

        Self {
            mode,
            stage,
            illust_id,
            image_url,
            target_path,
            error_kind: classification.error_kind,
            error_message: error.to_string(),
            retryable: classification.retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReplayCommand {
    User {
        user_id: String,
        options: ReplayOptions,
    },
    Illust {
        illust_id: String,
        options: ReplayOptions,
    },
    Bookmark {
        user_id: String,
        options: ReplayOptions,
    },
    Keyword {
        query: String,
        r18: bool,
        options: ReplayOptions,
    },
    Ranking {
        mode: String,
        options: ReplayOptions,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayOptions {
    pub directory: String,
    pub count: usize,
    pub sort: crate::config::SortOrder,
    pub r18: bool,
    pub ai: bool,
    pub concurrent: usize,
    pub timeout: u64,
    pub retry: usize,
    pub with_tags: bool,
    pub proxy_url: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureManifest {
    pub created_at: String,
    pub command: ReplayCommand,
    pub records: Vec<FailureRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorClassification {
    pub error_kind: String,
    pub retryable: bool,
}

impl From<&ResolvedDownloadOptions> for ReplayOptions {
    fn from(value: &ResolvedDownloadOptions) -> Self {
        Self {
            directory: value.directory.to_string_lossy().into_owned(),
            count: value.count,
            sort: value.sort,
            r18: value.r18,
            ai: value.ai,
            concurrent: value.concurrent,
            timeout: value.timeout,
            retry: value.retry,
            with_tags: value.with_tags,
            proxy_url: value.proxy_url.clone(),
            dry_run: value.dry_run,
        }
    }
}

impl ReplayOptions {
    pub fn to_resolved(&self, mode: DownloadMode) -> ResolvedDownloadOptions {
        ResolvedDownloadOptions {
            mode,
            directory: PathBuf::from(&self.directory),
            count: self.count,
            sort: self.sort,
            r18: self.r18,
            ai: self.ai,
            concurrent: self.concurrent,
            timeout: self.timeout,
            retry: self.retry,
            with_tags: self.with_tags,
            proxy_url: self.proxy_url.clone(),
            dry_run: self.dry_run,
        }
    }

    pub fn with_retry_profile(&self) -> Self {
        let mut next = self.clone();
        next.concurrent = 1;
        next.retry = next.retry.max(3);
        next
    }
}

impl ReplayCommand {
    pub fn file_stem(&self) -> &'static str {
        match self {
            Self::User { .. } => "download-user",
            Self::Illust { .. } => "download-illust",
            Self::Bookmark { .. } => "download-bookmark",
            Self::Keyword { .. } => "download-keyword",
            Self::Ranking { .. } => "download-ranking",
        }
    }

    pub fn with_retry_profile(&self) -> Self {
        match self {
            Self::User { user_id, options } => Self::User {
                user_id: user_id.clone(),
                options: options.with_retry_profile(),
            },
            Self::Illust { illust_id, options } => Self::Illust {
                illust_id: illust_id.clone(),
                options: options.with_retry_profile(),
            },
            Self::Bookmark { user_id, options } => Self::Bookmark {
                user_id: user_id.clone(),
                options: options.with_retry_profile(),
            },
            Self::Keyword {
                query,
                r18,
                options,
            } => Self::Keyword {
                query: query.clone(),
                r18: *r18,
                options: options.with_retry_profile(),
            },
            Self::Ranking { mode, options } => Self::Ranking {
                mode: mode.clone(),
                options: options.with_retry_profile(),
            },
        }
    }

    pub fn mode(&self) -> DownloadMode {
        match self {
            Self::User { .. } => DownloadMode::User,
            Self::Illust { .. } => DownloadMode::Illust,
            Self::Bookmark { .. } => DownloadMode::Bookmark,
            Self::Keyword { .. } => DownloadMode::Keyword,
            Self::Ranking { .. } => DownloadMode::Ranking,
        }
    }

    pub fn subject(&self) -> &str {
        match self {
            Self::User { user_id, .. } => user_id,
            Self::Illust { illust_id, .. } => illust_id,
            Self::Bookmark { user_id, .. } => user_id,
            Self::Keyword { query, .. } => query,
            Self::Ranking { mode, .. } => mode,
        }
    }

    pub fn options(&self) -> &ReplayOptions {
        match self {
            Self::User { options, .. }
            | Self::Illust { options, .. }
            | Self::Bookmark { options, .. }
            | Self::Keyword { options, .. }
            | Self::Ranking { options, .. } => options,
        }
    }
}

impl FailureManifest {
    pub fn new(command: ReplayCommand, records: Vec<FailureRecord>) -> AppResult<Self> {
        Ok(Self {
            created_at: manifest_timestamp()?,
            command,
            records,
        })
    }

    pub fn save(&self) -> AppResult<PathBuf> {
        let dir = ensure_config_dir()?.join("failures");
        fs::create_dir_all(&dir)?;
        let mut timestamp = self.created_at.clone();
        let path = loop {
            let candidate = dir.join(format!("{}-{}.json", timestamp, self.command.file_stem()));
            if !candidate.exists() {
                break candidate;
            }

            let base = parse_manifest_system_time(&timestamp)?;
            let next = Timestamp::try_from(base + Duration::from_secs(1))?;
            timestamp = next.strftime("%Y%m%dT%H%M%SZ").to_string();
        };
        fs::write(&path, serde_json::to_vec_pretty(self)?)?;
        Ok(path)
    }

    pub fn load_from(path: &Path) -> AppResult<Self> {
        let content = fs::read(path)?;
        Ok(serde_json::from_slice(&content)?)
    }
}

pub fn manifest_timestamp() -> AppResult<String> {
    let now = Timestamp::try_from(SystemTime::now())?;
    Ok(now.strftime("%Y%m%dT%H%M%SZ").to_string())
}

fn parse_manifest_system_time(value: &str) -> AppResult<SystemTime> {
    let timestamp: Timestamp = format!(
        "{}-{}-{}T{}:{}:{}Z",
        &value[0..4],
        &value[4..6],
        &value[6..8],
        &value[9..11],
        &value[11..13],
        &value[13..15]
    )
    .parse()?;
    Ok(timestamp.into())
}

pub fn classify_error(error: &eyre::Report) -> ErrorClassification {
    if let Some(reqwest_error) = error.downcast_ref::<reqwest::Error>() {
        if reqwest_error.is_timeout() {
            return ErrorClassification {
                error_kind: "timeout".to_string(),
                retryable: true,
            };
        }
        if reqwest_error.is_connect() {
            return ErrorClassification {
                error_kind: "connect".to_string(),
                retryable: true,
            };
        }
        if let Some(status) = reqwest_error.status() {
            let status_code = status.as_u16();
            return ErrorClassification {
                error_kind: format!("http_{status_code}"),
                retryable: matches!(status_code, 408 | 425 | 429 | 500 | 502 | 503 | 504),
            };
        }
        return ErrorClassification {
            error_kind: "request".to_string(),
            retryable: reqwest_error.is_request(),
        };
    }

    if let Some(crawler_error) = error.downcast_ref::<CrawlerError>() {
        return classify_crawler_error(crawler_error);
    }

    ErrorClassification {
        error_kind: "unknown".to_string(),
        retryable: false,
    }
}

fn classify_crawler_error(error: &CrawlerError) -> ErrorClassification {
    match error {
        CrawlerError::DownloadInterrupted(_) => ErrorClassification {
            error_kind: "download_interrupted".to_string(),
            retryable: true,
        },
        CrawlerError::HttpStatus { status, .. } => ErrorClassification {
            error_kind: format!("http_{status}"),
            retryable: matches!(*status, 408 | 425 | 429 | 500 | 502 | 503 | 504),
        },
        CrawlerError::Network(_) => ErrorClassification {
            error_kind: "network".to_string(),
            retryable: true,
        },
        CrawlerError::Io(_) => ErrorClassification {
            error_kind: "io".to_string(),
            retryable: true,
        },
        CrawlerError::Parse(_) => ErrorClassification {
            error_kind: "parse".to_string(),
            retryable: false,
        },
        CrawlerError::Auth(_) | CrawlerError::MissingCredential | CrawlerError::MissingUserId => {
            ErrorClassification {
                error_kind: "auth".to_string(),
                retryable: false,
            }
        }
        CrawlerError::Config(_) | CrawlerError::InvalidInput(_) => ErrorClassification {
            error_kind: "config".to_string(),
            retryable: false,
        },
        CrawlerError::Json(_) => ErrorClassification {
            error_kind: "json".to_string(),
            retryable: false,
        },
        CrawlerError::TomlDeserialize(_) | CrawlerError::TomlSerialize(_) => ErrorClassification {
            error_kind: "toml".to_string(),
            retryable: false,
        },
        CrawlerError::Url(_) => ErrorClassification {
            error_kind: "url".to_string(),
            retryable: false,
        },
        CrawlerError::Regex(_) => ErrorClassification {
            error_kind: "regex".to_string(),
            retryable: false,
        },
        CrawlerError::UserNotFound(_) | CrawlerError::IllustNotFound(_) => ErrorClassification {
            error_kind: "not_found".to_string(),
            retryable: false,
        },
        CrawlerError::MissingConfigDir(_) => ErrorClassification {
            error_kind: "missing_config_dir".to_string(),
            retryable: false,
        },
        CrawlerError::NotImplemented(_) => ErrorClassification {
            error_kind: "not_implemented".to_string(),
            retryable: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{
        config::{DownloadMode, ResolvedDownloadOptions, SortOrder},
        error::CrawlerError,
        test_support::{EnvVarGuard, lock_env},
    };

    use super::{
        FailureManifest, FailureRecord, FailureStage, ReplayCommand, ReplayOptions, classify_error,
    };

    fn options() -> ResolvedDownloadOptions {
        ResolvedDownloadOptions {
            mode: DownloadMode::Illust,
            directory: "/tmp/picals".into(),
            count: 1,
            sort: SortOrder::DateDesc,
            r18: false,
            ai: true,
            concurrent: 4,
            timeout: 30,
            retry: 2,
            with_tags: false,
            proxy_url: None,
            dry_run: false,
        }
    }

    #[test]
    fn replay_options_can_roundtrip_from_resolved_options() {
        let resolved = options();
        let replay = ReplayOptions::from(&resolved);

        assert_eq!(replay.to_resolved(DownloadMode::Illust), resolved);
    }

    #[tokio::test]
    async fn failure_manifest_can_roundtrip() {
        let _lock = lock_env().await;
        let temp = tempdir().unwrap();
        let _xdg = EnvVarGuard::set("XDG_CONFIG_HOME", temp.path());

        let manifest = FailureManifest::new(
            ReplayCommand::Illust {
                illust_id: "123456".to_string(),
                options: ReplayOptions::from(&options()),
            },
            vec![FailureRecord {
                mode: DownloadMode::Illust,
                stage: FailureStage::Download,
                illust_id: Some("123456".to_string()),
                image_url: Some("https://example.com/123456_p0.png".to_string()),
                target_path: Some("/tmp/picals/123456/123456_p0.png".to_string()),
                error_kind: "timeout".to_string(),
                error_message: "timeout".to_string(),
                retryable: true,
            }],
        )
        .unwrap();

        let path = manifest.save().unwrap();
        let loaded = FailureManifest::load_from(&path).unwrap();

        assert_eq!(loaded.command.file_stem(), "download-illust");
        assert_eq!(loaded.records.len(), 1);
    }

    #[test]
    fn classify_error_marks_download_interrupted_as_retryable() {
        let error = eyre::Report::new(CrawlerError::DownloadInterrupted("broken".to_string()));
        let classification = classify_error(&error);

        assert_eq!(classification.error_kind, "download_interrupted");
        assert!(classification.retryable);
    }

    #[test]
    fn classify_error_marks_retryable_http_status_as_retryable() {
        let error = eyre::Report::new(CrawlerError::HttpStatus {
            status: 429,
            context: "too many requests".to_string(),
        });
        let classification = classify_error(&error);

        assert_eq!(classification.error_kind, "http_429");
        assert!(classification.retryable);
    }
}
