//! config 子命令定义。

use clap::{Args, Subcommand};

#[derive(Debug, Clone, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    /// 查看当前配置
    Show,
    /// 设置单个配置项
    Set(SetConfigArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SetConfigArgs {
    #[arg(value_name = "KEY", help = "配置键，例如 download.directory")]
    pub key: String,

    #[arg(value_name = "VALUE", help = "配置值")]
    pub value: String,
}
