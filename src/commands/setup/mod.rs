//! `hpd setup` 命令。

mod auth_flow;
mod post_setup;
mod prompting;

use eyre::eyre;
use inquire::Confirm;

use super::prompt_support::map_inquire_error;
use crate::{auth::Credential, config::Config, error::AppResult};

use self::{
    auth_flow::fetch_current_user_id_from_pixiv,
    post_setup::{maybe_run_post_setup_organize, print_setup_success_hint, print_setup_summary},
    prompting::{print_setup_intro, prompt_download_config, prompt_phpsessid, prompt_user_id},
};

pub(crate) async fn run() -> AppResult<()> {
    let existing_credential = Credential::load()?;
    print_setup_intro(existing_credential.is_some());

    let mut config = Config::load()?;
    let previous_config = config.clone();
    let phpsessid = prompt_phpsessid(
        existing_credential
            .as_ref()
            .map(|credential| credential.phpsessid.as_str()),
    )?;
    let can_reuse_saved_user_id = existing_credential
        .as_ref()
        .map(|credential| credential.phpsessid.as_str() == phpsessid.as_str())
        .unwrap_or(false);
    let auto_user_id = fetch_current_user_id_from_pixiv(&Credential::new(&phpsessid)?)
        .await
        .map_err(|error| {
            println!();
            println!("自动识别当前账号 userId 失败：{error}");
            println!("将继续由你确认 userId。");
            error
        })
        .ok();
    let user_id = prompt_user_id(
        auto_user_id.as_deref(),
        existing_credential.as_ref().and_then(Credential::user_id),
        can_reuse_saved_user_id,
    )?;

    prompt_download_config(&mut config)?;

    print_setup_summary(&phpsessid, &user_id, &config);
    let confirmed = Confirm::new("确认写入以上配置？")
        .with_default(true)
        .prompt()
        .map_err(map_inquire_error)?;

    if !confirmed {
        return Err(eyre!("操作已取消"));
    }

    let credential = Credential::new_with_user_id(phpsessid, Some(user_id))?;
    credential.save()?;
    config.save()?;
    maybe_run_post_setup_organize(&previous_config, &config)?;
    print_setup_success_hint();

    Ok(())
}
