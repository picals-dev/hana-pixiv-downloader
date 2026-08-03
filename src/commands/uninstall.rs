//! hpd 卸载命令。

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

use eyre::{Context, eyre};

use crate::{
    cli::uninstall::UninstallCommand,
    config::{Config, config_dir, expand_home_dir},
    error::{AppResult, CrawlerError},
};

pub(crate) async fn run(args: UninstallCommand) -> AppResult<()> {
    if !args.yes {
        return Err(CrawlerError::InvalidInput(
            "卸载会删除 hpd 可执行文件和本地配置。请确认后运行 `hpd uninstall --yes`；如需同时删除下载内容，追加 `--purge-downloads`".to_string(),
        )
        .into());
    }

    let download_dirs = if args.purge_downloads {
        configured_download_dirs()?
    } else {
        Vec::new()
    };
    let config_dir = config_dir()?;
    let executable_paths = executable_paths()?;

    for directory in download_dirs {
        remove_dir_if_exists(&directory, "下载目录")?;
    }
    remove_dir_if_exists(&config_dir, "配置目录")?;

    for executable in executable_paths {
        remove_executable(&executable)?;
    }

    println!("✅ hpd 已卸载");
    if args.purge_downloads {
        println!("已删除配置中记录的下载目录。");
    } else {
        println!("已保留下载内容；如需删除，请运行：hpd uninstall --yes --purge-downloads");
    }
    Ok(())
}

fn configured_download_dirs() -> AppResult<Vec<PathBuf>> {
    let roots = Config::load()?.download.roots;
    let mut directories = BTreeSet::new();

    for root in [
        roots.illust,
        roots.user,
        roots.bookmark,
        roots.keyword,
        roots.ranking,
    ] {
        directories.insert(normalize_download_dir(Path::new(&root))?);
    }

    Ok(directories.into_iter().collect())
}

fn normalize_download_dir(path: &Path) -> AppResult<PathBuf> {
    let path = expand_home_dir(path)?;
    let path = if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .wrap_err("定位当前工作目录失败")?
            .join(path)
    };

    if path.parent().is_none() {
        return Err(eyre!(CrawlerError::InvalidInput(format!(
            "拒绝删除文件系统根目录：{}",
            path.display()
        ))));
    }

    Ok(path)
}

fn executable_paths() -> AppResult<Vec<PathBuf>> {
    let current = env::current_exe().wrap_err("定位当前 hpd 可执行文件失败")?;
    let current_canonical = fs::canonicalize(&current)
        .with_context(|| format!("解析 hpd 可执行文件路径失败: {}", current.display()))?;
    let mut paths = BTreeSet::from([current]);

    let invoked = PathBuf::from(env::args_os().next().unwrap_or_default());
    if invoked.components().count() > 1 && invoked.exists() {
        paths.insert(invoked);
    }

    if let Some(path) = env::var_os("PATH") {
        for directory in env::split_paths(&path) {
            let candidate = directory.join("hpd");
            if fs::canonicalize(&candidate)
                .map(|resolved| resolved == current_canonical)
                .unwrap_or(false)
            {
                paths.insert(candidate);
            }
        }
    }

    Ok(paths.into_iter().collect())
}

#[cfg(unix)]
fn remove_executable(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_file(path)
        .with_context(|| format!("删除 hpd 可执行文件失败: {}", path.display()))?;
    println!("已删除可执行文件：{}", path.display());
    Ok(())
}

#[cfg(windows)]
fn remove_executable(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Ok(());
    }

    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Seconds 1; Remove-Item -LiteralPath $env:HPD_UNINSTALL_EXECUTABLE -Force",
        ])
        .env("HPD_UNINSTALL_EXECUTABLE", path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("安排删除 hpd 可执行文件失败: {}", path.display()))?;
    println!("将在当前进程退出后删除可执行文件：{}", path.display());
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn remove_executable(path: &Path) -> AppResult<()> {
    Err(eyre!(CrawlerError::InvalidInput(format!(
        "当前系统不支持自动删除 hpd 可执行文件：{}",
        path.display()
    ))))
}

fn remove_dir_if_exists(path: &Path, description: &str) -> AppResult<()> {
    if !path.exists() {
        return Ok(());
    }

    fs::remove_dir_all(path)
        .with_context(|| format!("删除{description}失败: {}", path.display()))?;
    println!("已删除{description}：{}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::normalize_download_dir;

    #[test]
    fn refuses_to_purge_filesystem_root() {
        let error = normalize_download_dir(Path::new("/")).unwrap_err();

        assert!(format!("{error:#}").contains("拒绝删除文件系统根目录"));
    }
}
