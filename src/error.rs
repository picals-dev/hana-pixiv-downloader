//! 统一错误定义。

use std::path::PathBuf;

use thiserror::Error;

pub type AppResult<T> = eyre::Result<T>;

#[derive(Debug, Error)]
pub enum CrawlerError {
    #[error("认证失败: {0}")]
    Auth(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("网络请求失败: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API 响应解析失败: {0}")]
    Parse(String),

    #[error("序列化失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML 解析失败: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("TOML 序列化失败: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL 解析失败: {0}")]
    Url(#[from] url::ParseError),

    #[error("正则表达式错误: {0}")]
    Regex(#[from] regex::Error),

    #[error("未找到用户: {0}")]
    UserNotFound(String),

    #[error("未找到作品: {0}")]
    IllustNotFound(String),

    #[error("下载中断: {0}")]
    DownloadInterrupted(String),

    #[error("HTTP 请求失败: 状态码 {status}，{context}")]
    HttpStatus { status: u16, context: String },

    #[error("未找到认证信息，请先运行 picals-crawler setup")]
    MissingCredential,

    #[error("当前认证信息缺少 userId，请重新运行 picals-crawler setup")]
    MissingUserId,

    #[error("未找到配置目录: {0}")]
    MissingConfigDir(PathBuf),

    #[error("输入无效: {0}")]
    InvalidInput(String),

    #[error("{0} 尚未实现")]
    NotImplemented(&'static str),
}

impl CrawlerError {
    pub fn not_implemented(feature: &'static str) -> Self {
        Self::NotImplemented(feature)
    }
}
