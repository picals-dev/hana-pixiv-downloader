//! 爬虫模块骨架。

pub mod bookmark;
pub mod illust;
pub mod keyword;
pub mod ranking;
pub mod shared;
pub mod user;

use crate::{auth::Credential, config::ResolvedDownloadOptions};

#[derive(Debug, Clone)]
pub struct CrawlContext {
    pub credential: Credential,
    pub options: ResolvedDownloadOptions,
}

impl CrawlContext {
    pub fn new(credential: Credential, options: ResolvedDownloadOptions) -> Self {
        Self {
            credential,
            options,
        }
    }
}
