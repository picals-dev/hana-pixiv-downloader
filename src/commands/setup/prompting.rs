use inquire::Text;

use super::super::prompt_support::{
    map_inquire_error, prompt_batch_layout, prompt_bool, prompt_optional_text, prompt_sort_order,
    prompt_text, prompt_u64, prompt_usize,
};
use crate::{
    auth::Credential,
    config::{Config, DownloadRootsConfig},
    error::AppResult,
};

pub(crate) fn print_setup_intro(has_saved_credential: bool) {
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
    println!("提示：下面会明文显示凭据，避免录屏或共享屏幕。");
    if has_saved_credential {
        println!("检测到已有认证信息，直接回车可保留当前值；输入新值会覆盖现有配置。");
    }
    println!();
}

pub(crate) fn prompt_phpsessid(current: Option<&str>) -> AppResult<String> {
    let prompt_config = build_phpsessid_prompt(current);
    let prompt = Text::new("PHPSESSID").with_help_message(prompt_config.help_message);
    let prompt = if let Some(default_value) = prompt_config.default_value {
        prompt.with_default(default_value)
    } else {
        prompt
    };

    prompt
        .prompt()
        .map_err(map_inquire_error)
        .and_then(Credential::new)
        .map(|credential| credential.phpsessid)
}

pub(crate) fn prompt_user_id(
    auto_user_id: Option<&str>,
    current_user_id: Option<&str>,
    can_reuse_saved_user_id: bool,
) -> AppResult<String> {
    let prompt_config =
        build_user_id_prompt(auto_user_id, current_user_id, can_reuse_saved_user_id);

    let prompt = Text::new(&prompt_config.message)
        .with_help_message("请输入纯数字 userId；可从 Pixiv 个人主页 URL 中提取");
    let prompt = if let Some(default_user_id) = prompt_config.default_value {
        prompt.with_default(default_user_id)
    } else {
        prompt
    };

    let user_id = prompt.prompt().map_err(map_inquire_error)?;
    Credential::parse_user_id(&user_id)
}

pub(crate) fn prompt_download_config(config: &mut Config) -> AppResult<()> {
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
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PhpsessidPrompt<'a> {
    default_value: Option<&'a str>,
    help_message: &'static str,
}

fn build_phpsessid_prompt(current: Option<&str>) -> PhpsessidPrompt<'_> {
    match current {
        Some(current) => PhpsessidPrompt {
            default_value: Some(current),
            help_message: "当前会明文显示；直接回车保持已保存的 PHPSESSID，输入新值会覆盖当前配置",
        },
        None => PhpsessidPrompt {
            default_value: None,
            help_message: "当前会明文显示，便于你核对粘贴是否正确",
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UserIdPrompt<'a> {
    message: String,
    default_value: Option<&'a str>,
}

fn build_user_id_prompt<'a>(
    auto_user_id: Option<&'a str>,
    current_user_id: Option<&'a str>,
    can_reuse_saved_user_id: bool,
) -> UserIdPrompt<'a> {
    match auto_user_id {
        Some(user_id) => {
            let message = match current_user_id {
                Some(current_user_id) if current_user_id != user_id => format!(
                    "当前账号 userId（已自动识别为 {user_id}；当前保存值为 {current_user_id}，可直接回车使用自动识别结果或手动修改）"
                ),
                _ => {
                    format!("当前账号 userId（已自动识别为 {user_id}，可直接回车确认或手动修改）")
                }
            };

            UserIdPrompt {
                message,
                default_value: Some(user_id),
            }
        }
        None if can_reuse_saved_user_id => match current_user_id {
            Some(current_user_id) => UserIdPrompt {
                message: format!(
                    "当前账号 userId（自动识别失败，将使用已保存的 {current_user_id} 作为默认值，可直接回车确认或手动修改）"
                ),
                default_value: Some(current_user_id),
            },
            None => UserIdPrompt {
                message: "当前账号 userId（自动识别失败，请手动填写）".to_string(),
                default_value: None,
            },
        },
        None if current_user_id.is_some() => UserIdPrompt {
            message:
                "当前账号 userId（新 PHPSESSID 的自动识别失败；为避免复用旧账号的 userId，请手动填写）"
                    .to_string(),
            default_value: None,
        },
        None => UserIdPrompt {
            message: "当前账号 userId（自动识别失败，请手动填写）".to_string(),
            default_value: None,
        },
    }
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

#[cfg(test)]
mod tests {
    use crate::auth::Credential;

    use super::{
        build_phpsessid_prompt, build_user_id_prompt, derive_setup_download_roots,
        infer_download_root_seed, prompt_user_id,
    };

    #[test]
    fn prompt_user_id_accepts_auto_detected_value_contract() {
        let user_id = Credential::parse_user_id("12345678").unwrap();
        assert_eq!(user_id, "12345678");
    }

    #[test]
    fn saved_phpsessid_prompt_allows_enter_to_keep_current_value() {
        let prompt = build_phpsessid_prompt(Some("cookie-value"));

        assert_eq!(prompt.default_value, Some("cookie-value"));
        assert!(
            prompt
                .help_message
                .contains("直接回车保持已保存的 PHPSESSID")
        );
    }

    #[test]
    fn build_user_id_prompt_prefers_auto_detected_value() {
        let prompt = build_user_id_prompt(Some("12345678"), Some("87654321"), true);

        assert_eq!(prompt.default_value, Some("12345678"));
        assert!(prompt.message.contains("已自动识别为 12345678"));
        assert!(prompt.message.contains("当前保存值为 87654321"));
    }

    #[test]
    fn build_user_id_prompt_reuses_saved_value_when_cookie_is_unchanged() {
        let prompt = build_user_id_prompt(None, Some("12345678"), true);

        assert_eq!(prompt.default_value, Some("12345678"));
        assert!(
            prompt
                .message
                .contains("将使用已保存的 12345678 作为默认值")
        );
    }

    #[test]
    fn build_user_id_prompt_requires_manual_input_when_cookie_changes() {
        let prompt = build_user_id_prompt(None, Some("12345678"), false);

        assert_eq!(prompt.default_value, None);
        assert!(
            prompt
                .message
                .contains("为避免复用旧账号的 userId，请手动填写")
        );
    }

    #[test]
    fn prompt_user_id_rejects_invalid_manual_value() {
        let error = Credential::parse_user_id("bad-user").unwrap_err();
        assert!(format!("{error:#}").contains("userId 必须是纯数字"));
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
        let _ = prompt_user_id
            as fn(Option<&str>, Option<&str>, bool) -> crate::error::AppResult<String>;
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
}
