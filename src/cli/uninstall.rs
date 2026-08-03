//! uninstall 子命令定义。

use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct UninstallCommand {
    #[arg(long, help = "确认删除 hpd 可执行文件与本地配置")]
    pub yes: bool,

    #[arg(long, requires = "yes", help = "同时删除配置中记录的下载目录")]
    pub purge_downloads: bool,
}
