//! `picals-crawler setup` 命令。

use eyre::{Report, eyre};
use inquire::{InquireError, Password, PasswordDisplayMode, Text};

use crate::{auth::Credential, config::Config, error::AppResult};

pub async fn run() -> AppResult<()> {
    println!("🌸 欢迎使用 Picals Crawler！");
    println!();
    println!("在开始下载之前，需要先完成 Pixiv 认证。请按以下步骤操作：");
    println!();
    println!("Step 1: 在浏览器中打开 https://www.pixiv.net 并登录你的 Pixiv 账号");
    println!("Step 2: 登录后，按 F12 打开开发者工具");
    println!("        → 点击顶部的 \"Application\" 标签");
    println!("        → 左侧找到 Cookies → https://www.pixiv.net");
    println!("        → 找到 PHPSESSID 这一项");
    println!("Step 3: 复制 PHPSESSID 的值，粘贴到下面。");
    println!();

    let phpsessid = Password::new("PHPSESSID")
        .without_confirmation()
        .with_display_mode(PasswordDisplayMode::Masked)
        .prompt()
        .map_err(map_inquire_error)?;

    let mut config = Config::load()?;
    let directory = Text::new("下载目录")
        .with_default(config.download.directory.as_str())
        .prompt()
        .map_err(map_inquire_error)?;

    let credential = Credential::new(phpsessid)?;
    credential.save()?;

    config.download.directory = directory.trim().to_string();
    config.save()?;

    println!();
    println!("✅ 配置完成！认证信息已保存。");
    println!("现在可以开始下载了：");
    println!("  picals-crawler download user <画师ID>");
    println!("查看完整帮助: picals-crawler --help");

    Ok(())
}

fn map_inquire_error(error: InquireError) -> Report {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            eyre!("操作已取消")
        }
        other => Report::new(other).wrap_err("交互式输入失败"),
    }
}
