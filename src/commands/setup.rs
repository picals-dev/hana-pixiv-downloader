//! `picals-crawler setup` 命令。

use eyre::{Report, eyre};
use inquire::{InquireError, Password, PasswordDisplayMode, Text};
use url::Url;

use crate::{
    auth::Credential,
    collector::{PixivCollector, selector::select_current_user_id},
    config::{Config, DownloadOverrides, EnvOverrides, ResolvedDownloadOptions},
    error::AppResult,
};

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

    let user_id = resolve_setup_user_id(
        &phpsessid,
        fetch_current_user_id_from_pixiv(&Credential::new(&phpsessid)?).await,
        prompt_manual_user_id,
    )?;
    let credential = Credential::new_with_user_id(phpsessid, Some(user_id))?;
    credential.save()?;

    config.download.directory = directory.trim().to_string();
    config.save()?;

    println!();
    println!("✅ 配置完成！认证信息与当前账号身份已保存。");
    println!("现在可以开始下载了：");
    println!("  picals-crawler download user <画师ID>");
    println!("  picals-crawler download bookmark");
    println!("查看完整帮助: picals-crawler --help");

    Ok(())
}

fn resolve_setup_user_id<ManualPrompt>(
    phpsessid: &str,
    auto_resolve: AppResult<String>,
    mut manual_prompt: ManualPrompt,
) -> AppResult<String>
where
    ManualPrompt: FnMut() -> AppResult<String>,
{
    let _credential = Credential::new(phpsessid)?;

    match auto_resolve {
        Ok(user_id) => {
            println!();
            println!("已自动识别当前账号 userId: {user_id}");
            Ok(user_id)
        }
        Err(error) => {
            println!();
            println!("自动识别当前账号 userId 失败：{error}");
            println!("将切换为手动输入 userId。");
            manual_prompt()
        }
    }
}

async fn fetch_current_user_id_from_pixiv(credential: &Credential) -> AppResult<String> {
    let config = Config::load()?;
    let env = EnvOverrides::from_process_env()?;
    let options = config.resolve_download_options(&env, &DownloadOverrides::default())?;
    fetch_current_user_id_with_options(credential, &options).await
}

async fn fetch_current_user_id_with_options(
    credential: &Credential,
    options: &ResolvedDownloadOptions,
) -> AppResult<String> {
    let base_url = crate::collector::resolve_base_url(None)?;
    fetch_current_user_id_with_base_url(credential, options, base_url).await
}

async fn fetch_current_user_id_with_base_url(
    credential: &Credential,
    options: &ResolvedDownloadOptions,
    base_url: Url,
) -> AppResult<String> {
    let collector = PixivCollector::new_with_base_url(options, credential, base_url)?;
    let page = collector.fetch_current_user_homepage().await?;
    Ok(select_current_user_id(
        page.header_user_id.as_deref(),
        &page.html,
    )?)
}

fn prompt_manual_user_id() -> AppResult<String> {
    let user_id = Text::new("当前账号 userId")
        .with_help_message("请输入纯数字 userId；可从 Pixiv 个人主页 URL 中提取")
        .prompt()
        .map_err(map_inquire_error)?;

    Credential::parse_user_id(&user_id)
}

fn map_inquire_error(error: InquireError) -> Report {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            eyre!("操作已取消")
        }
        other => Report::new(other).wrap_err("交互式输入失败"),
    }
}

#[cfg(test)]
mod tests {
    use eyre::eyre;
    use std::path::PathBuf;

    use tempfile::tempdir;
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::{
        auth::Credential,
        config::{DownloadConfig, ResolvedDownloadOptions, SortOrder},
    };

    use super::{fetch_current_user_id_with_base_url, resolve_setup_user_id};

    fn options(directory: PathBuf) -> ResolvedDownloadOptions {
        let defaults = DownloadConfig::default();
        ResolvedDownloadOptions {
            directory,
            count: defaults.count,
            sort: SortOrder::DateDesc,
            r18: defaults.r18,
            ai: defaults.ai,
            concurrent: 1,
            timeout: 5,
            retry: 0,
            with_tags: false,
            proxy_url: None,
            dry_run: false,
        }
    }

    #[tokio::test]
    async fn setup_user_id_resolution_prefers_auto_detected_value() {
        let user_id = resolve_setup_user_id("cookie", Ok("12345678".to_string()), || {
            Ok("87654321".to_string())
        })
        .unwrap();

        assert_eq!(user_id, "12345678");
    }

    #[tokio::test]
    async fn setup_user_id_resolution_falls_back_to_manual_input() {
        let user_id = resolve_setup_user_id("cookie", Err(eyre!("自动解析失败")), || {
            Ok("12345678".to_string())
        })
        .unwrap();

        assert_eq!(user_id, "12345678");
    }

    #[tokio::test]
    async fn setup_can_fetch_current_user_id_from_response_header() {
        let server = MockServer::start().await;
        let temp = tempdir().unwrap();
        let credential = Credential::new("cookie").unwrap();
        let options = options(temp.path().to_path_buf());
        let base_url = Url::parse(&server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-userid", "12345678")
                    .set_body_string("<html><body>ok</body></html>"),
            )
            .mount(&server)
            .await;

        let user_id = fetch_current_user_id_with_base_url(&credential, &options, base_url)
            .await
            .unwrap();

        assert_eq!(user_id, "12345678");
    }

    #[tokio::test]
    async fn setup_can_fetch_current_user_id_from_homepage_html() {
        let server = MockServer::start().await;
        let temp = tempdir().unwrap();
        let credential = Credential::new("cookie").unwrap();
        let options = options(temp.path().to_path_buf());
        let base_url = Url::parse(&server.uri()).unwrap();

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"<html><script>pixiv.user.id = "12345678";</script></html>"#,
                ),
            )
            .mount(&server)
            .await;

        let user_id = fetch_current_user_id_with_base_url(&credential, &options, base_url)
            .await
            .unwrap();

        assert_eq!(user_id, "12345678");
    }
}
