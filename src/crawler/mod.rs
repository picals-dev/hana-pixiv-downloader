//! 爬虫模块骨架。

pub mod bookmark;
pub mod illust;
pub mod keyword;
pub mod ranking;
pub(crate) mod shared;
pub mod user;

use std::sync::Arc;

use crate::{config::ResolvedDownloadOptions, net::PixivNetSession};

#[derive(Debug, Clone)]
pub(crate) struct CrawlContext {
    pub options: ResolvedDownloadOptions,
    pub session: Arc<PixivNetSession>,
}

impl CrawlContext {
    pub(crate) fn new(options: ResolvedDownloadOptions, session: Arc<PixivNetSession>) -> Self {
        Self { options, session }
    }
}
