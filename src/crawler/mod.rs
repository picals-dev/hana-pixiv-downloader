//! 爬虫模块骨架。

pub mod bookmark;
pub mod illust;
pub mod keyword;
pub mod ranking;
pub mod shared;
pub mod user;

use std::sync::Arc;

use crate::{config::ResolvedDownloadOptions, net::PixivNetSession};

#[derive(Debug, Clone)]
pub struct CrawlContext {
    pub options: ResolvedDownloadOptions,
    pub session: Arc<PixivNetSession>,
}

impl CrawlContext {
    pub fn new(options: ResolvedDownloadOptions, session: Arc<PixivNetSession>) -> Self {
        Self { options, session }
    }
}
