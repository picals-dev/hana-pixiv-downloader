//! `picals-crawler config` 命令。

use serde::Serialize;

use crate::{
    auth::Credential,
    cli::config::SetConfigArgs,
    config::{Config, config_dir, parse_sort_value},
    error::{AppResult, CrawlerError},
};

#[derive(Debug, Serialize)]
struct ConfigShowOutput {
    auth: AuthConfigView,
    download: crate::config::DownloadConfig,
    proxy: crate::config::ProxyConfig,
}

#[derive(Debug, Serialize)]
struct AuthConfigView {
    phpsessid: String,
    user_id: Option<String>,
}

pub(crate) async fn show() -> AppResult<()> {
    let config = Config::load()?;
    let credential = Credential::load()?;

    println!("配置目录: {}", config_dir()?.display());
    println!();
    let rendered = render_show_output(config, credential)?;
    println!("{rendered}");

    Ok(())
}

pub(crate) async fn set(args: SetConfigArgs) -> AppResult<()> {
    let updated_key = args.key.clone();

    match args.key.as_str() {
        "auth.phpsessid" => set_auth_phpsessid(&args.value)?,
        "auth.user_id" => set_auth_user_id(&args.value)?,
        _ => set_regular_config(args)?,
    }

    println!("✅ 已更新配置：{updated_key}");
    Ok(())
}

fn set_auth_phpsessid(value: &str) -> AppResult<()> {
    let mut credential = Credential::load()?.unwrap_or_else(|| Credential {
        phpsessid: String::new(),
        user_id: None,
    });
    credential.set_phpsessid(value.to_string())?;
    credential.save()?;
    Ok(())
}

fn set_auth_user_id(value: &str) -> AppResult<()> {
    let mut credential = Credential::load()?.ok_or_else(|| {
        CrawlerError::InvalidInput(
            "尚未配置 PHPSESSID，请先运行 setup 或先设置 auth.phpsessid".to_string(),
        )
    })?;
    credential.set_user_id(Some(value.to_string()))?;
    credential.save()?;
    Ok(())
}

fn set_regular_config(args: SetConfigArgs) -> AppResult<()> {
    let mut config = Config::load()?;

    match args.key.as_str() {
        "download.count" => config.download.count = parse_usize(&args.key, &args.value)?,
        "download.sort" => config.download.sort = parse_sort_value(&args.value)?,
        "download.r18" => config.download.r18 = parse_bool(&args.key, &args.value)?,
        "download.ai" => config.download.ai = parse_bool(&args.key, &args.value)?,
        "download.concurrent" => config.download.concurrent = parse_usize(&args.key, &args.value)?,
        "download.timeout" => config.download.timeout = parse_u64(&args.key, &args.value)?,
        "download.retry" => config.download.retry = parse_usize(&args.key, &args.value)?,
        "download.with_tags" => config.download.with_tags = parse_bool(&args.key, &args.value)?,
        "download.roots.illust" => config.download.roots.illust = parse_string(&args.value)?,
        "download.roots.user" => config.download.roots.user = parse_string(&args.value)?,
        "download.roots.bookmark" => config.download.roots.bookmark = parse_string(&args.value)?,
        "download.roots.keyword" => config.download.roots.keyword = parse_string(&args.value)?,
        "download.roots.ranking" => config.download.roots.ranking = parse_string(&args.value)?,
        "proxy.url" => config.proxy.url = args.value,
        _ => {
            return Err(CrawlerError::InvalidInput(format!("不支持的配置键: {}", args.key)).into());
        }
    }

    config.save()?;
    Ok(())
}

fn render_show_output(config: Config, credential: Option<Credential>) -> AppResult<String> {
    toml::to_string_pretty(&ConfigShowOutput {
        auth: AuthConfigView {
            phpsessid: credential
                .as_ref()
                .map(|value| value.phpsessid.clone())
                .unwrap_or_default(),
            user_id: credential.and_then(|value| value.user_id),
        },
        download: config.download,
        proxy: config.proxy,
    })
    .map_err(Into::into)
}

fn parse_string(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CrawlerError::InvalidInput("配置值不能为空".to_string()).into());
    }

    Ok(trimmed.to_string())
}

fn parse_bool(key: &str, value: &str) -> AppResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(CrawlerError::InvalidInput(format!("{key} 需要布尔值（true/false）")).into()),
    }
}

fn parse_usize(key: &str, value: &str) -> AppResult<usize> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| CrawlerError::InvalidInput(format!("{key} 需要无符号整数")).into())
}

fn parse_u64(key: &str, value: &str) -> AppResult<u64> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| CrawlerError::InvalidInput(format!("{key} 需要无符号整数")).into())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{
        auth::Credential,
        cli::config::SetConfigArgs,
        config::{Config, DownloadRootsConfig},
        test_support::{EnvVarGuard, lock_env},
    };

    use super::{render_show_output, set};

    #[tokio::test]
    async fn config_set_can_update_auth_keys() {
        let _lock = lock_env().await;
        let temp = tempdir().unwrap();
        let xdg_home = temp.path().join(".config");
        let _xdg = EnvVarGuard::set("XDG_CONFIG_HOME", &xdg_home);

        set(SetConfigArgs {
            key: "auth.phpsessid".to_string(),
            value: "cookie-value".to_string(),
        })
        .await
        .unwrap();
        set(SetConfigArgs {
            key: "auth.user_id".to_string(),
            value: "12345678".to_string(),
        })
        .await
        .unwrap();

        let credential = Credential::load().unwrap().unwrap();
        assert_eq!(credential.phpsessid, "cookie-value");
        assert_eq!(credential.user_id(), Some("12345678"));
    }

    #[test]
    fn config_show_output_contains_auth_and_mode_roots() {
        let mut config = Config::default();
        config.download.roots = DownloadRootsConfig {
            illust: "/tmp/illust".to_string(),
            user: "/tmp/user".to_string(),
            bookmark: "/tmp/bookmark".to_string(),
            keyword: "/tmp/keyword".to_string(),
            ranking: "/tmp/ranking".to_string(),
        };

        let rendered = render_show_output(
            config,
            Some(Credential::new_with_user_id("cookie-value", Some("12345678")).unwrap()),
        )
        .unwrap();

        assert!(rendered.contains("[auth]"));
        assert!(rendered.contains("phpsessid = \"cookie-value\""));
        assert!(rendered.contains("user_id = \"12345678\""));
        assert!(rendered.contains("[download.roots]"));
        assert!(rendered.contains("illust = \"/tmp/illust\""));
        assert!(rendered.contains("ranking = \"/tmp/ranking\""));
    }
}
