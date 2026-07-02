//! organize 子命令定义。

use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct OrganizeCommand {
    #[arg(long, help = "只预览整理计划，不实际移动文件")]
    pub dry_run: bool,

    #[arg(long, short = 'y', help = "跳过执行前确认，直接按当前配置整理")]
    pub yes: bool,
}
