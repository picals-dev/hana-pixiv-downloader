//! download 子命令定义。

use std::path::PathBuf;

use clap::{ArgAction, Args, Subcommand};

use crate::config::{DownloadOverrides, SortOrder};

#[derive(Debug, Clone, Args)]
pub struct DownloadCommand {
    #[command(subcommand)]
    pub target: DownloadSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DownloadSubcommand {
    /// 下载指定画师的全部作品
    User(UserArgs),
    /// 下载关键词搜索结果
    Keyword(KeywordArgs),
    /// 下载排行榜作品
    Ranking(RankingArgs),
    /// 下载单张作品的所有图片
    Illust(IllustArgs),
    /// 下载当前账号的收藏
    Bookmark(BookmarkArgs),
}

#[derive(Debug, Clone, Default, Args)]
pub struct CommonDownloadArgs {
    #[arg(long = "to", value_name = "PATH", help = "覆盖下载目录")]
    pub directory: Option<PathBuf>,

    #[arg(
        long,
        value_name = "URL",
        help = "代理地址，也支持 HTTPS_PROXY 环境变量"
    )]
    pub proxy: Option<String>,

    #[arg(long, value_name = "COUNT", help = "下载数量，0 表示全部")]
    pub count: Option<usize>,

    #[arg(long, value_enum, help = "排序方式")]
    pub sort: Option<SortOrder>,

    #[arg(long, action = ArgAction::SetTrue, help = "包含 R-18 作品")]
    pub r18: bool,

    #[arg(long = "no-ai", action = ArgAction::SetTrue, help = "排除 AI 作品")]
    pub no_ai: bool,

    #[arg(long, value_name = "N", help = "并发下载数")]
    pub concurrent: Option<usize>,

    #[arg(long, value_name = "SECONDS", help = "单次请求超时时间（秒）")]
    pub timeout: Option<u64>,

    #[arg(long, value_name = "N", help = "网络错误重试次数")]
    pub retry: Option<usize>,

    #[arg(long = "with-tags", action = ArgAction::SetTrue, conflicts_with = "no_tags", help = "保存 tags.json")]
    pub with_tags: bool,

    #[arg(long = "no-tags", action = ArgAction::SetTrue, conflicts_with = "with_tags", help = "不保存 tags.json")]
    pub no_tags: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "只列出将要下载的内容，不实际下载")]
    pub dry_run: bool,
}

impl CommonDownloadArgs {
    pub fn to_overrides(&self) -> DownloadOverrides {
        DownloadOverrides {
            directory: self.directory.clone(),
            count: self.count,
            sort: self.sort,
            r18: self.r18.then_some(true),
            ai: self.no_ai.then_some(false),
            concurrent: self.concurrent,
            timeout: self.timeout,
            retry: self.retry,
            with_tags: if self.with_tags {
                Some(true)
            } else if self.no_tags {
                Some(false)
            } else {
                None
            },
            proxy_url: self.proxy.clone(),
            dry_run: self.dry_run,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct UserArgs {
    #[arg(value_name = "ID_OR_URL", help = "画师 ID 或 Pixiv 用户页面 URL")]
    pub target: String,

    #[command(flatten)]
    pub common: CommonDownloadArgs,
}

#[derive(Debug, Clone, Args)]
pub struct KeywordArgs {
    #[arg(value_name = "QUERY", help = "关键词")]
    pub query: String,

    #[command(flatten)]
    pub common: CommonDownloadArgs,
}

#[derive(Debug, Clone, Args)]
pub struct RankingArgs {
    #[arg(long, default_value = "daily", help = "排行榜模式")]
    pub mode: String,

    #[command(flatten)]
    pub common: CommonDownloadArgs,
}

#[derive(Debug, Clone, Args)]
pub struct IllustArgs {
    #[arg(value_name = "ID_OR_URL", help = "作品 ID 或作品页面 URL")]
    pub target: String,

    #[command(flatten)]
    pub common: CommonDownloadArgs,
}

#[derive(Debug, Clone, Args)]
pub struct BookmarkArgs {
    #[command(flatten)]
    pub common: CommonDownloadArgs,
}
