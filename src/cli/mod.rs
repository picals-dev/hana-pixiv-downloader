//! CLI 命令定义。

pub mod config;
pub mod download;

use clap::{Args, Parser, Subcommand};

use self::{config::ConfigCommand, download::DownloadCommand};

#[derive(Debug, Parser)]
#[command(
    name = "picals-crawler",
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
    Download(DownloadCommand),
    /// 查看或修改配置
    Config(ConfigCommand),
}
