//! CLI 命令定义。

pub mod config;
pub mod download;
pub mod organize;

use clap::{Args, Parser, Subcommand};

use self::{config::ConfigCommand, download::DownloadCommand, organize::OrganizeCommand};

#[derive(Debug, Parser)]
#[command(
    name = "hpd",
    version,
    about = "开箱即用的 Pixiv 图片下载 CLI 工具",
    propagate_version = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Default, Args)]
pub struct GlobalArgs {
    #[arg(long, global = true, help = "显示详细日志")]
    pub verbose: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 交互式认证引导与配置初始化
    Setup,
    /// 下载 Pixiv 图片
    Download(Box<DownloadCommand>),
    /// 重试失败清单中的可重试项
    Retry(RetryCommand),
    /// 查看或修改配置
    Config(ConfigCommand),
    /// 按当前批量布局整理已有下载目录
    Organize(OrganizeCommand),
    /// 更新到最新正式版
    #[command(visible_alias = "upgrade")]
    Update,
}

#[derive(Debug, Clone, Args)]
pub struct RetryCommand {
    #[arg(value_name = "MANIFEST_PATH", help = "失败清单 manifest 路径")]
    pub manifest_path: std::path::PathBuf,
}
