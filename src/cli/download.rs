//! download 子命令定义。

use std::path::PathBuf;

use clap::{ArgAction, Args, Subcommand, ValueEnum};

use crate::config::{DownloadOverrides, SortOrder};

#[derive(Debug, Clone, Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub struct DownloadCommand {
    #[command(subcommand)]
    pub target: Option<DownloadSubcommand>,

    #[command(flatten)]
    pub direct: DirectDownloadArgs,
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
pub struct DirectDownloadArgs {
    #[arg(
        value_name = "PIXIV_URL",
        help = "直接粘贴 Pixiv 用户、作品或标签页面 URL",
        required = true
    )]
    pub pixiv_url: Option<String>,

    #[command(flatten)]
    pub common: CommonDownloadArgs,
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
    pub sort: Option<SortArg>,

    #[arg(long = "no-ai", action = ArgAction::SetTrue, help = "排除 AI 作品")]
    pub no_ai: bool,

    #[arg(long, value_name = "N", help = "并发下载数")]
    pub concurrent: Option<usize>,

    #[arg(long, value_name = "SECONDS", help = "单次请求超时时间（秒）")]
    pub timeout: Option<u64>,

    #[arg(long, value_name = "N", help = "网络错误重试次数")]
    pub retry: Option<usize>,

    #[arg(long = "with-tags", action = ArgAction::SetTrue, conflicts_with = "no_tags", help = "导出当前批次的 tags.json")]
    pub with_tags: bool,

    #[arg(long = "no-tags", action = ArgAction::SetTrue, conflicts_with = "with_tags", help = "显式关闭 tags.json 导出")]
    pub no_tags: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "只列出将要下载的内容，不实际下载")]
    pub dry_run: bool,
}

impl CommonDownloadArgs {
    pub(crate) fn to_overrides(&self) -> DownloadOverrides {
        DownloadOverrides {
            directory: self.directory.clone(),
            count: self.count,
            sort: self.sort.map(Into::into),
            r18: None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum SortArg {
    DateDesc,
    DateAsc,
}

impl From<SortArg> for SortOrder {
    fn from(value: SortArg) -> Self {
        match value {
            SortArg::DateDesc => SortOrder::DateDesc,
            SortArg::DateAsc => SortOrder::DateAsc,
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

    #[arg(long, action = ArgAction::SetTrue, help = "切换为 R-18 搜索")]
    pub r18: bool,

    #[command(flatten)]
    pub common: CommonDownloadArgs,
}

impl KeywordArgs {
    pub(crate) fn to_overrides(&self) -> DownloadOverrides {
        let mut overrides = self.common.to_overrides();
        overrides.r18 = self.r18.then_some(true);
        overrides
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum RankingMode {
    Daily,
    Weekly,
    Monthly,
    Male,
    Female,
    DailyR18,
    WeeklyR18,
}

impl RankingMode {
    pub fn as_api_mode(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Male => "male",
            Self::Female => "female",
            Self::DailyR18 => "daily_r18",
            Self::WeeklyR18 => "weekly_r18",
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct RankingArgs {
    #[arg(value_name = "OPTION", value_enum, help = "排行榜模式")]
    pub mode: Option<RankingMode>,

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

    #[arg(long, value_name = "N", help = "并发下载数")]
    pub concurrent: Option<usize>,

    #[arg(long, value_name = "SECONDS", help = "单次请求超时时间（秒）")]
    pub timeout: Option<u64>,

    #[arg(long, value_name = "N", help = "网络错误重试次数")]
    pub retry: Option<usize>,

    #[arg(long = "with-tags", action = ArgAction::SetTrue, conflicts_with = "no_tags", help = "导出当前批次的 tags.json")]
    pub with_tags: bool,

    #[arg(long = "no-tags", action = ArgAction::SetTrue, conflicts_with = "with_tags", help = "显式关闭 tags.json 导出")]
    pub no_tags: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "只列出将要下载的内容，不实际下载")]
    pub dry_run: bool,
}

impl RankingArgs {
    pub(crate) fn to_overrides(&self) -> DownloadOverrides {
        DownloadOverrides {
            directory: self.directory.clone(),
            count: self.count,
            sort: None,
            r18: None,
            ai: None,
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
pub struct IllustArgs {
    #[arg(value_name = "ID_OR_URL", help = "作品 ID 或作品页面 URL")]
    pub target: String,

    #[command(flatten)]
    pub common: CommonDownloadArgs,
}

#[derive(Debug, Clone, Args)]
pub struct BookmarkArgs {
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

    #[arg(long, value_name = "N", help = "并发下载数")]
    pub concurrent: Option<usize>,

    #[arg(long, value_name = "SECONDS", help = "单次请求超时时间（秒）")]
    pub timeout: Option<u64>,

    #[arg(long, value_name = "N", help = "网络错误重试次数")]
    pub retry: Option<usize>,

    #[arg(long = "with-tags", action = ArgAction::SetTrue, conflicts_with = "no_tags", help = "导出当前批次的 tags.json")]
    pub with_tags: bool,

    #[arg(long = "no-tags", action = ArgAction::SetTrue, conflicts_with = "with_tags", help = "显式关闭 tags.json 导出")]
    pub no_tags: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "只列出将要下载的内容，不实际下载")]
    pub dry_run: bool,
}

impl BookmarkArgs {
    pub(crate) fn to_overrides(&self) -> DownloadOverrides {
        DownloadOverrides {
            directory: self.directory.clone(),
            count: self.count,
            sort: None,
            r18: None,
            ai: None,
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
