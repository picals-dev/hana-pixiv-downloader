//! `picals-crawler setup` 命令。

use std::fmt;

use eyre::{Report, eyre};
use inquire::{Confirm, InquireError, Password, PasswordDisplayMode, Select, Text};
use url::Url;

use crate::{
    auth::Credential,
    collector::{PixivCollector, selector::select_current_user_id},
    config::{
        Config, DownloadMode, DownloadOverrides, DownloadRootsConfig, EnvOverrides,
        ResolvedDownloadOptions, SortOrder,
    },
    error::AppResult,
};

pub async fn run() -> AppResult<()> {
    print_setup_intro();

    let mut config = Config::load()?;
    let phpsessid = prompt_phpsessid()?;
    let auto_user_id = fetch_current_user_id_from_pixiv(&Credential::new(&phpsessid)?)
        .await
        .map_err(|error| {
            println!();
            println!("自动识别当前账号 userId 失败：{error}");
            println!("将继续由你手动确认 userId。");
            error
        })
        .ok();
    let user_id = prompt_user_id(auto_user_id.as_deref())?;

    config.download.roots = prompt_download_roots(&config.download.roots)?;
    config.download.count = prompt_usize(
        "默认下载数量",
        "0 表示下载当前模式可获取的全部内容",
        config.download.count,
    )?;
    config.download.sort = prompt_sort_order(config.download.sort)?;
    config.download.r18 = prompt_bool(
        "默认开启 R-18 过滤",
        "仅影响支持该开关的命令",
        config.download.r18,
    )?;
    config.download.ai = prompt_bool(
        "默认包含 AI 作品",
        "关闭后会尽量排除 AI 作品",
        config.download.ai,
    )?;
    config.download.concurrent = prompt_usize(
        "默认并发下载数",
        "建议从 4 到 8 开始，避免过高并发触发限流",
        config.download.concurrent,
    )?;
    config.download.timeout = prompt_u64(
        "默认单次请求超时（秒）",
        "用于单次 HTTP 请求的超时上限",
        config.download.timeout,
    )?;
    config.download.retry = prompt_usize(
        "默认网络重试次数",
        "当前版本的统一请求层会在此基础上执行收敛策略",
        config.download.retry,
    )?;
    config.download.with_tags = prompt_bool(
        "默认导出 tags.json",
        "开启后会在下载目录中写入当前批次 tags.json",
        config.download.with_tags,
    )?;
    config.proxy.url = prompt_optional_text(
        "默认代理地址",
        "留空表示不使用代理，也支持后续通过 config set proxy.url 修改",
        &config.proxy.url,
    )?;

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

    println!();
    println!("✅ 配置完成！你现在可以通过以下命令继续查看或修改：");
    println!("  picals-crawler config show");
    println!("  picals-crawler config set auth.phpsessid <PHPSESSID>");
    println!("  picals-crawler config set auth.user_id <USER_ID>");
    println!("  picals-crawler download user <画师ID>");
    println!("  picals-crawler download bookmark");
    println!("查看完整帮助: picals-crawler --help");

    Ok(())
}

fn print_setup_intro() {
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
}

fn prompt_phpsessid() -> AppResult<String> {
    Password::new("PHPSESSID")
        .without_confirmation()
        .with_display_mode(PasswordDisplayMode::Full)
        .with_help_message("当前使用明文显示，便于你核对粘贴是否正确")
        .prompt()
        .map_err(map_inquire_error)
        .and_then(Credential::new)
        .map(|credential| credential.phpsessid)
}

fn prompt_user_id(default_user_id: Option<&str>) -> AppResult<String> {
    let message = match default_user_id {
        Some(user_id) => {
            format!("当前账号 userId（已自动识别为 {user_id}，可直接回车确认或手动修改）")
        }
        None => "当前账号 userId（自动识别失败，请手动填写）".to_string(),
    };

    let prompt = Text::new(&message)
        .with_help_message("请输入纯数字 userId；可从 Pixiv 个人主页 URL 中提取");
    let prompt = if let Some(default_user_id) = default_user_id {
        prompt.with_default(default_user_id)
    } else {
        prompt
    };

    let user_id = prompt.prompt().map_err(map_inquire_error)?;
    Credential::parse_user_id(&user_id)
}

fn prompt_download_roots(current: &DownloadRootsConfig) -> AppResult<DownloadRootsConfig> {
    println!();
    println!("下面开始配置五种下载模式对应的根目录。");

    Ok(DownloadRootsConfig {
        illust: prompt_text(
            "作品下载根目录（illust）",
            "用于 download illust 的根目录；最终会继续追加作品目录",
            &current.illust,
        )?,
        user: prompt_text(
            "画师下载根目录（user）",
            "用于 download user 的根目录；最终会继续追加 userId/作品目录",
            &current.user,
        )?,
        bookmark: prompt_text(
            "收藏下载根目录（bookmark）",
            "用于 download bookmark 的根目录；最终会继续追加 userId/作品目录",
            &current.bookmark,
        )?,
        keyword: prompt_text(
            "关键词下载根目录（keyword）",
            "用于 download keyword 的根目录；最终会继续追加关键词目录/作品目录",
            &current.keyword,
        )?,
        ranking: prompt_text(
            "排行榜下载根目录（ranking）",
            "用于 download ranking 的根目录；最终会继续追加 mode/作品目录",
            &current.ranking,
        )?,
    })
}

fn prompt_text(message: &str, help: &str, default: &str) -> AppResult<String> {
    let value = Text::new(message)
        .with_help_message(help)
        .with_default(default)
        .prompt()
        .map_err(map_inquire_error)?;

    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{message} 不能为空"));
    }

    Ok(trimmed.to_string())
}

fn prompt_optional_text(message: &str, help: &str, default: &str) -> AppResult<String> {
    let prompt = Text::new(message).with_help_message(help);
    let prompt = if default.trim().is_empty() {
        prompt
    } else {
        prompt.with_default(default)
    };

    Ok(prompt
        .prompt()
        .map_err(map_inquire_error)?
        .trim()
        .to_string())
}

fn prompt_usize(message: &str, help: &str, default: usize) -> AppResult<usize> {
    let value = Text::new(message)
        .with_help_message(help)
        .with_default(&default.to_string())
        .prompt()
        .map_err(map_inquire_error)?;

    value
        .trim()
        .parse::<usize>()
        .map_err(|_| eyre!("{message} 需要无符号整数"))
}

fn prompt_u64(message: &str, help: &str, default: u64) -> AppResult<u64> {
    let value = Text::new(message)
        .with_help_message(help)
        .with_default(&default.to_string())
        .prompt()
        .map_err(map_inquire_error)?;

    value
        .trim()
        .parse::<u64>()
        .map_err(|_| eyre!("{message} 需要无符号整数"))
}

fn prompt_bool(message: &str, help: &str, default: bool) -> AppResult<bool> {
    let default_choice = BoolChoice::from(default);
    let options = vec![BoolChoice::Yes, BoolChoice::No];
    let selected = Select::new(message, options)
        .with_help_message(help)
        .with_starting_cursor(if default_choice == BoolChoice::Yes {
            0
        } else {
            1
        })
        .prompt()
        .map_err(map_inquire_error)?;

    Ok(bool::from(selected))
}

fn prompt_sort_order(default: SortOrder) -> AppResult<SortOrder> {
    let options = vec![SortChoice::DateDesc, SortChoice::DateAsc];
    let cursor = if default == SortOrder::DateAsc { 1 } else { 0 };
    let selected = Select::new("默认排序方式", options)
        .with_help_message("popular_desc 已冻结，不再作为可选项")
        .with_starting_cursor(cursor)
        .prompt()
        .map_err(map_inquire_error)?;

    Ok(SortOrder::from(selected))
}

fn print_setup_summary(phpsessid: &str, user_id: &str, config: &Config) {
    println!();
    println!("请确认以下配置摘要：");
    println!("  auth.phpsessid = {phpsessid}");
    println!("  auth.user_id = {user_id}");
    println!("  download.roots.illust = {}", config.download.roots.illust);
    println!("  download.roots.user = {}", config.download.roots.user);
    println!(
        "  download.roots.bookmark = {}",
        config.download.roots.bookmark
    );
    println!(
        "  download.roots.keyword = {}",
        config.download.roots.keyword
    );
    println!(
        "  download.roots.ranking = {}",
        config.download.roots.ranking
    );
    println!("  download.count = {}", config.download.count);
    println!("  download.sort = {}", render_sort(config.download.sort));
    println!("  download.r18 = {}", config.download.r18);
    println!("  download.ai = {}", config.download.ai);
    println!("  download.concurrent = {}", config.download.concurrent);
    println!("  download.timeout = {}", config.download.timeout);
    println!("  download.retry = {}", config.download.retry);
    println!("  download.with_tags = {}", config.download.with_tags);
    println!("  proxy.url = {}", render_optional(&config.proxy.url));
}

fn render_sort(sort: SortOrder) -> &'static str {
    match sort {
        SortOrder::DateDesc => "date_desc",
        SortOrder::DateAsc => "date_asc",
        SortOrder::PopularDesc => "popular_desc",
    }
}

fn render_optional(value: &str) -> &str {
    if value.trim().is_empty() {
        "<未设置>"
    } else {
        value
    }
}

async fn fetch_current_user_id_from_pixiv(credential: &Credential) -> AppResult<String> {
    let config = Config::load()?;
    let env = EnvOverrides::from_process_env()?;
    let options = config.resolve_download_options(
        DownloadMode::Illust,
        &env,
        &DownloadOverrides::default(),
    )?;
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

fn map_inquire_error(error: InquireError) -> Report {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            eyre!("操作已取消")
        }
        other => Report::new(other).wrap_err("交互式输入失败"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoolChoice {
    Yes,
    No,
}

impl From<bool> for BoolChoice {
    fn from(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }
}

impl From<BoolChoice> for bool {
    fn from(value: BoolChoice) -> Self {
        matches!(value, BoolChoice::Yes)
    }
}

impl fmt::Display for BoolChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yes => write!(f, "是"),
            Self::No => write!(f, "否"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortChoice {
    DateDesc,
    DateAsc,
}

impl From<SortChoice> for SortOrder {
    fn from(value: SortChoice) -> Self {
        match value {
            SortChoice::DateDesc => SortOrder::DateDesc,
            SortChoice::DateAsc => SortOrder::DateAsc,
        }
    }
}

impl fmt::Display for SortChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DateDesc => write!(f, "date_desc（新的作品优先）"),
            Self::DateAsc => write!(f, "date_asc（旧的作品优先）"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use eyre::eyre;
    use tempfile::tempdir;
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::{
        auth::Credential,
        config::{DownloadConfig, DownloadMode, SortOrder},
    };

    use super::{fetch_current_user_id_with_base_url, prompt_user_id, render_sort};

    fn options(directory: PathBuf) -> super::ResolvedDownloadOptions {
        let defaults = DownloadConfig::default();
        super::ResolvedDownloadOptions {
            mode: DownloadMode::Illust,
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

    #[test]
    fn render_sort_uses_public_config_values() {
        assert_eq!(render_sort(SortOrder::DateDesc), "date_desc");
        assert_eq!(render_sort(SortOrder::DateAsc), "date_asc");
    }

    #[test]
    fn prompt_user_id_accepts_auto_detected_value_contract() {
        let user_id = Credential::parse_user_id("12345678").unwrap();
        assert_eq!(user_id, "12345678");
    }

    #[test]
    fn prompt_user_id_rejects_invalid_manual_value() {
        let error = Credential::parse_user_id("bad-user").unwrap_err();
        assert!(format!("{error:#}").contains("userId 必须是纯数字"));
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

    #[test]
    fn parse_user_id_still_rejects_invalid_values() {
        let error = Credential::parse_user_id("  ").unwrap_err();
        assert!(format!("{error:#}").contains("userId 不能为空"));
    }

    #[test]
    fn parse_user_id_still_accepts_valid_values() {
        let user_id = Credential::parse_user_id("87654321").unwrap();
        assert_eq!(user_id, "87654321");
    }

    #[test]
    fn placeholder_compiles_for_interactive_user_id_contract() {
        let _ = prompt_user_id as fn(Option<&str>) -> crate::error::AppResult<String>;
        let _ = eyre!("保持测试模块导入活跃");
    }
}
