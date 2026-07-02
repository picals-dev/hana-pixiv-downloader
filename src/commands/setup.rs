//! `hpd setup` 命令。

use eyre::eyre;
use inquire::{Confirm, Password, PasswordDisplayMode, Text};
use url::Url;

use super::prompt_support::{
    map_inquire_error, prompt_batch_layout, prompt_bool, prompt_optional_text, prompt_sort_order,
    prompt_text, prompt_u64, prompt_usize,
};
use crate::{
    auth::Credential,
    config::{
        Config, DownloadMode, DownloadOverrides, DownloadRootsConfig, EnvOverrides,
        ResolvedDownloadOptions, SortOrder,
    },
    error::AppResult,
    net::PixivNetSession,
    organize::OrganizeExecutionReport,
    pixiv::selector::select_current_user_id,
};

pub(crate) async fn run() -> AppResult<()> {
    print_setup_intro();

    let mut config = Config::load()?;
    let previous_config = config.clone();
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
    config.download.batch_layout = prompt_batch_layout(
        "批量下载目录布局",
        "只影响多作品下载；mixed 会在单输出作品时直接平铺",
        config.download.batch_layout,
    )?;
    config.download.count = prompt_usize(
        "默认下载数量",
        "0 表示下载当前模式可获取的全部内容",
        config.download.count,
    )?;
    config.download.sort = prompt_sort_order(
        "默认排序方式",
        "支持按发布时间排序（新→旧 / 旧→新）",
        config.download.sort,
    )?;
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
    maybe_run_post_setup_organize(&previous_config, &config)?;

    println!();
    println!("✅ 配置完成！你现在可以通过以下命令继续查看或修改：");
    println!("  hpd config show");
    println!("  hpd config set auth.phpsessid <PHPSESSID>");
    println!("  hpd config set auth.user_id <USER_ID>");
    println!("  hpd download user <画师ID>");
    println!("  hpd download <Pixiv URL>");
    println!("  hpd download bookmark");
    println!("查看完整帮助: hpd --help");

    Ok(())
}

fn maybe_run_post_setup_organize(previous: &Config, current: &Config) -> AppResult<()> {
    match evaluate_layout_change(previous, current) {
        LayoutChangeAction::None => Ok(()),
        LayoutChangeAction::OfferOrganizeNow => {
            println!();
            let confirmed =
                Confirm::new("批量目录布局已变化，是否立即整理当前 batch roots 下的已有目录？")
                    .with_default(false)
                    .prompt()
                    .map_err(map_inquire_error)?;
            if !confirmed {
                println!("你可以稍后手动运行 `hpd organize --dry-run` 或 `hpd organize --yes`。");
                return Ok(());
            }

            match crate::commands::organize::run_with_config(current, false, true) {
                Ok(report) => print_post_organize_result(&report),
                Err(error) => {
                    println!("立即整理失败：{error}");
                    println!("配置已经写入，你可以稍后运行 `hpd organize --yes` 继续整理。");
                }
            }
            Ok(())
        }
        LayoutChangeAction::ExplainCrossRootOnly => {
            println!();
            println!("本次修改了 batch roots。");
            println!("当前版本的 hpd organize 只支持当前 root 内原地整理，不负责跨 root 迁移。");
            println!("如需整理，请先确认最终 roots 后，再运行：");
            println!("  hpd organize --dry-run");
            Ok(())
        }
    }
}

fn print_setup_intro() {
    println!("🌸 欢迎使用 hana-pixiv-downloader！");
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
    println!("先配置统一下载根目录。");
    let current_root = infer_download_root_seed(current);
    let root_input = prompt_optional_text(
        "统一下载根目录",
        if current_root.is_some() {
            "直接回车保持当前统一 root；若修改，会自动更新下面五个目录的默认值"
        } else {
            "留空表示保留当前五个目录不变；填写后会自动派生下面五个目录的默认值"
        },
        current_root.as_deref().unwrap_or_default(),
    )?;
    let defaults = derive_setup_download_roots(current, &root_input);

    println!("下面开始确认五种下载模式对应的根目录。");

    Ok(DownloadRootsConfig {
        illust: prompt_text(
            "作品下载根目录（illust）",
            "用于 download illust 的根目录；最终会继续追加作品目录",
            &defaults.illust,
        )?,
        user: prompt_text(
            "画师下载根目录（user）",
            "用于 download user 的根目录；最终会追加 userId，再按 batch_layout 决定作品是否建目录",
            &defaults.user,
        )?,
        bookmark: prompt_text(
            "收藏下载根目录（bookmark）",
            "用于 download bookmark 的根目录；最终会追加 userId，再按 batch_layout 决定作品是否建目录",
            &defaults.bookmark,
        )?,
        keyword: prompt_text(
            "关键词下载根目录（keyword）",
            "用于 download keyword 的根目录；最终会追加关键词目录，再按 batch_layout 决定作品是否建目录",
            &defaults.keyword,
        )?,
        ranking: prompt_text(
            "排行榜下载根目录（ranking）",
            "用于 download ranking 的根目录；最终会追加 mode，再按 batch_layout 决定作品是否建目录",
            &defaults.ranking,
        )?,
    })
}

fn infer_download_root_seed(current: &DownloadRootsConfig) -> Option<String> {
    let illust = strip_mode_suffix(&current.illust, "illust")?;
    let user = strip_mode_suffix(&current.user, "user")?;
    let bookmark = strip_mode_suffix(&current.bookmark, "bookmark")?;
    let keyword = strip_mode_suffix(&current.keyword, "keyword")?;
    let ranking = strip_mode_suffix(&current.ranking, "ranking")?;

    if illust == user && user == bookmark && bookmark == keyword && keyword == ranking {
        Some(illust)
    } else {
        None
    }
}

fn derive_setup_download_roots(
    current: &DownloadRootsConfig,
    root_input: &str,
) -> DownloadRootsConfig {
    let trimmed = root_input.trim();
    if trimmed.is_empty() {
        current.clone()
    } else {
        DownloadRootsConfig::from_seed(trimmed)
    }
}

fn strip_mode_suffix(path: &str, mode: &str) -> Option<String> {
    let trimmed = path.trim_end_matches(['/', '\\']);
    let slash_suffix = format!("/{mode}");
    let backslash_suffix = format!("\\{mode}");

    trimmed
        .strip_suffix(&slash_suffix)
        .or_else(|| trimmed.strip_suffix(&backslash_suffix))
        .map(ToOwned::to_owned)
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
    println!(
        "  download.batch_layout = {}",
        config.download.batch_layout.display_name()
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
    }
}

fn render_optional(value: &str) -> &str {
    if value.trim().is_empty() {
        "<未设置>"
    } else {
        value
    }
}

fn print_post_organize_result(report: &OrganizeExecutionReport) {
    println!(
        "已完成当前 batch roots 原地整理：移动 {}，跳过 {}，冲突 {}，未识别 {}。",
        report.moved_files, report.skipped_files, report.conflicts, report.unknown_files
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutChangeAction {
    None,
    OfferOrganizeNow,
    ExplainCrossRootOnly,
}

fn evaluate_layout_change(previous: &Config, current: &Config) -> LayoutChangeAction {
    if !batch_roots_unchanged(previous, current) {
        return LayoutChangeAction::ExplainCrossRootOnly;
    }

    if previous.download.batch_layout == current.download.batch_layout {
        return LayoutChangeAction::None;
    }

    LayoutChangeAction::OfferOrganizeNow
}

fn batch_roots_unchanged(previous: &Config, current: &Config) -> bool {
    previous.download.roots.user == current.download.roots.user
        && previous.download.roots.bookmark == current.download.roots.bookmark
        && previous.download.roots.keyword == current.download.roots.keyword
        && previous.download.roots.ranking == current.download.roots.ranking
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
    let base_url = crate::net::resolve_base_url(None)?;
    fetch_current_user_id_with_base_url(credential, options, base_url).await
}

async fn fetch_current_user_id_with_base_url(
    credential: &Credential,
    options: &ResolvedDownloadOptions,
    base_url: Url,
) -> AppResult<String> {
    let session =
        PixivNetSession::new_with_base_url(options.clone(), credential.clone(), base_url)?;
    let page = session.fetch_current_user_homepage().await?;
    Ok(select_current_user_id(
        page.header_user_id.as_deref(),
        &page.html,
    )?)
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
        config::{
            BatchLayoutStrategy, Config, DownloadConfig, DownloadMode, DownloadRootsConfig,
            SortOrder,
        },
    };

    use super::{
        LayoutChangeAction, derive_setup_download_roots, evaluate_layout_change,
        fetch_current_user_id_with_base_url, infer_download_root_seed, prompt_user_id, render_sort,
    };

    fn options(directory: PathBuf) -> super::ResolvedDownloadOptions {
        let defaults = DownloadConfig::default();
        super::ResolvedDownloadOptions {
            mode: DownloadMode::Illust,
            directory,
            batch_layout: BatchLayoutStrategy::Mixed,
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

    #[test]
    fn infer_download_root_seed_returns_shared_root() {
        let roots = crate::config::DownloadRootsConfig::from_seed("~/Pictures/Pixiv");

        assert_eq!(
            infer_download_root_seed(&roots).as_deref(),
            Some("~/Pictures/Pixiv")
        );
    }

    #[test]
    fn infer_download_root_seed_returns_none_for_mixed_roots() {
        let roots = crate::config::DownloadRootsConfig {
            illust: "~/Pictures/Pixiv/illust".to_string(),
            user: "~/Downloads/Pixiv/user".to_string(),
            bookmark: "~/Pictures/Pixiv/bookmark".to_string(),
            keyword: "~/Pictures/Pixiv/keyword".to_string(),
            ranking: "~/Pictures/Pixiv/ranking".to_string(),
        };

        assert_eq!(infer_download_root_seed(&roots), None);
    }

    #[test]
    fn derive_setup_download_roots_uses_root_input_as_seed() {
        let current = crate::config::DownloadRootsConfig {
            illust: "/tmp/custom-illust".to_string(),
            user: "/tmp/custom-user".to_string(),
            bookmark: "/tmp/custom-bookmark".to_string(),
            keyword: "/tmp/custom-keyword".to_string(),
            ranking: "/tmp/custom-ranking".to_string(),
        };

        let derived = derive_setup_download_roots(&current, "/data/pixiv");

        assert_eq!(
            derived,
            crate::config::DownloadRootsConfig::from_seed("/data/pixiv")
        );
    }

    #[test]
    fn derive_setup_download_roots_keeps_current_roots_when_input_is_empty() {
        let current = crate::config::DownloadRootsConfig {
            illust: "/tmp/custom-illust".to_string(),
            user: "/tmp/custom-user".to_string(),
            bookmark: "/tmp/custom-bookmark".to_string(),
            keyword: "/tmp/custom-keyword".to_string(),
            ranking: "/tmp/custom-ranking".to_string(),
        };

        let derived = derive_setup_download_roots(&current, "   ");

        assert_eq!(derived, current);
    }

    #[test]
    fn setup_layout_matrix_offers_organize_when_only_layout_changes() {
        let previous = Config::default();
        let mut current = previous.clone();
        current.download.batch_layout = BatchLayoutStrategy::Flat;

        assert_eq!(
            evaluate_layout_change(&previous, &current),
            LayoutChangeAction::OfferOrganizeNow
        );
    }

    #[test]
    fn setup_layout_matrix_explains_cross_root_when_batch_roots_change_only() {
        let previous = Config::default();
        let mut current = previous.clone();
        current.download.roots = DownloadRootsConfig {
            illust: previous.download.roots.illust.clone(),
            user: "/tmp/other-user-root".to_string(),
            bookmark: previous.download.roots.bookmark.clone(),
            keyword: previous.download.roots.keyword.clone(),
            ranking: previous.download.roots.ranking.clone(),
        };

        assert_eq!(
            evaluate_layout_change(&previous, &current),
            LayoutChangeAction::ExplainCrossRootOnly
        );
    }

    #[test]
    fn setup_layout_matrix_ignores_illust_root_for_batch_organize_prompt() {
        let previous = Config::default();
        let mut current = previous.clone();
        current.download.batch_layout = BatchLayoutStrategy::PerIllust;
        current.download.roots = DownloadRootsConfig {
            illust: "/tmp/other-illust-root".to_string(),
            user: previous.download.roots.user.clone(),
            bookmark: previous.download.roots.bookmark.clone(),
            keyword: previous.download.roots.keyword.clone(),
            ranking: previous.download.roots.ranking.clone(),
        };

        assert_eq!(
            evaluate_layout_change(&previous, &current),
            LayoutChangeAction::OfferOrganizeNow
        );
    }
}
