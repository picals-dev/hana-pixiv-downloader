//! config 子命令定义。

use clap::{ArgAction, Args, Subcommand};

#[derive(Debug, Clone, Args)]
pub struct ConfigCommand {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigAction {
    /// 查看当前配置
    Show,
    /// 设置单个配置项；不带参数时显示完整配置字段表
    #[command(disable_help_flag = true)]
    Set(SetConfigArgs),
}

#[derive(Debug, Clone, Args)]
pub struct SetConfigArgs {
    #[arg(
        short = 'h',
        long = "help",
        action = ArgAction::SetTrue,
        help = "显示配置字段与当前值"
    )]
    pub help: bool,

    #[arg(
        value_name = "KEY",
        help = "配置键，例如 auth.phpsessid、download.batch_layout 或 download.roots.user"
    )]
    pub key: Option<String>,

    #[arg(value_name = "VALUE", help = "配置值")]
    pub value: Option<String>,
}
