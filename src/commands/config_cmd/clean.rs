use std::fs;

use eyre::Context;

use crate::{config::config_dir, error::AppResult};

pub(crate) async fn clean() -> AppResult<()> {
    let dir = config_dir()?;
    if !dir.exists() {
        println!("当前已是首次使用状态，无需清理：{}", dir.display());
        return Ok(());
    }

    fs::remove_dir_all(&dir).with_context(|| format!("清理配置目录失败: {}", dir.display()))?;
    println!("✅ 已清除配置目录：{}", dir.display());
    println!("下次使用前如需重新配置，请运行：");
    println!("  hpd setup");
    Ok(())
}
