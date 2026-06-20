//! `picals-crawler config` 命令。

use crate::{
    auth::Credential,
    cli::config::SetConfigArgs,
    config::{Config, config_dir, parse_sort_value},
    error::{AppResult, CrawlerError},
};

pub async fn show() -> AppResult<()> {
    let config = Config::load()?;
    let credential = Credential::load()?;

    println!("配置目录: {}", config_dir()?.display());
    println!(
        "认证状态: {}",
        if credential.is_some() {
            "已配置"
        } else {
            "未配置，请先运行 picals-crawler setup"
        }
    );
    println!();
    println!("{}", toml::to_string_pretty(&config)?);

    Ok(())
}

pub async fn set(args: SetConfigArgs) -> AppResult<()> {
    let mut config = Config::load()?;

    match args.key.as_str() {
        "download.directory" => config.download.directory = args.value,
        "download.count" => config.download.count = parse_usize(&args.key, &args.value)?,
        "download.sort" => config.download.sort = parse_sort_value(&args.value)?,
        "download.r18" => config.download.r18 = parse_bool(&args.key, &args.value)?,
        "download.ai" => config.download.ai = parse_bool(&args.key, &args.value)?,
        "download.concurrent" => config.download.concurrent = parse_usize(&args.key, &args.value)?,
        "download.timeout" => config.download.timeout = parse_u64(&args.key, &args.value)?,
        "download.retry" => config.download.retry = parse_usize(&args.key, &args.value)?,
        "download.with_tags" => config.download.with_tags = parse_bool(&args.key, &args.value)?,
        "proxy.url" => config.proxy.url = args.value,
        _ => {
            return Err(CrawlerError::InvalidInput(format!("不支持的配置键: {}", args.key)).into());
        }
    }

    config.save()?;
    println!("✅ 已更新配置：{}", args.key);
    Ok(())
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

    use crate::cli::config::SetConfigArgs;

    use super::set;

    #[tokio::test]
    async fn config_set_rejects_popular_sort() {
        let temp = tempdir().unwrap();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp.path());
        }

        let error = set(SetConfigArgs {
            key: "download.sort".to_string(),
            value: "popular_desc".to_string(),
        })
        .await
        .unwrap_err();

        assert!(format!("{error:#}").contains("popular_desc 已不再支持"));
    }
}
