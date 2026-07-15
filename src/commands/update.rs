//! `hpd update` 命令。

use std::{
    fs,
    io::{self, Cursor},
    path::Path,
    process::Command as ProcessCommand,
};

#[cfg(not(windows))]
use std::process::Stdio;

#[cfg(windows)]
use std::path::PathBuf;

use eyre::{Context, bail, eyre};
use flate2::read::GzDecoder;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tempfile::Builder;
use zip::ZipArchive;

use crate::error::AppResult;

const RELEASE_API_URL: &str =
    "https://api.github.com/repos/picals-dev/hana-pixiv-downloader/releases/latest";
const REPOSITORY: &str = "picals-dev/hana-pixiv-downloader";
const MARKER_FILE_NAME: &str = ".hpd-install.json";
const INSTALL_MARKER: &str = r#"{"schema_version":1,"method":"github-release","repository":"picals-dev/hana-pixiv-downloader"}"#;

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    digest: Option<String>,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct InstallMarker {
    schema_version: u8,
    method: String,
    repository: String,
}

pub(crate) async fn run() -> AppResult<()> {
    let executable = std::env::current_exe().wrap_err("无法定位当前 hpd 可执行文件")?;
    let install_dir = executable
        .parent()
        .ok_or_else(|| eyre!("无法定位 hpd 的安装目录"))?;
    ensure_managed_install(&executable, install_dir)?;

    let current_version =
        Version::parse(env!("CARGO_PKG_VERSION")).wrap_err("当前 hpd 版本号不符合 SemVer 格式")?;
    let client = build_update_client()?;
    let release = fetch_latest_release(&client).await?;
    let latest_version = parse_release_version(&release.tag_name)?;

    if latest_version <= current_version {
        if latest_version == current_version {
            println!("当前已是最新正式版 v{current_version}。");
        } else {
            println!(
                "当前版本 v{current_version} 高于最新正式版 v{latest_version}，不会自动降级。"
            );
        }
        return Ok(());
    }

    let asset_name = current_asset_name()?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .ok_or_else(|| eyre!("Release v{latest_version} 缺少当前平台资产：{asset_name}"))?;
    let expected_digest = expected_digest(&client, &release, asset).await?;

    println!("发现新版本 v{latest_version}，正在下载更新...");
    let archive = download_bytes(&client, &asset.browser_download_url, &asset.name).await?;
    verify_digest(&archive, &expected_digest)?;

    match stage_and_replace(&executable, asset_name, &archive)? {
        #[cfg(not(windows))]
        UpdateResult::Applied => {
            println!("已更新到 v{latest_version}。可运行 hpd --version 确认。")
        }
        #[cfg(windows)]
        UpdateResult::Scheduled => println!(
            "已下载 v{latest_version}。hpd 退出后将完成替换，请重新运行 hpd --version 确认。"
        ),
    }

    Ok(())
}

fn build_update_client() -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(format!("hpd/{} self-update", env!("CARGO_PKG_VERSION")));

    if let Some(proxy_url) = std::env::var("HPD_PROXY_URL")
        .ok()
        .or_else(|| std::env::var("HTTPS_PROXY").ok())
        .filter(|value| !value.trim().is_empty())
    {
        builder = builder.proxy(
            reqwest::Proxy::all(&proxy_url)
                .wrap_err_with(|| format!("更新代理配置无效：{proxy_url}"))?,
        );
    }

    builder.build().wrap_err("无法创建更新请求客户端")
}

async fn fetch_latest_release(client: &reqwest::Client) -> AppResult<Release> {
    let response = client
        .get(RELEASE_API_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .wrap_err("查询最新 Release 失败，请检查网络后重试")?
        .error_for_status()
        .wrap_err("查询最新 Release 失败，请稍后重试")?;
    let release = response
        .json::<Release>()
        .await
        .wrap_err("最新 Release 响应格式无效")?;

    if release.draft || release.prerelease {
        bail!("最新 Release 不是可更新的正式版");
    }

    Ok(release)
}

async fn expected_digest(
    client: &reqwest::Client,
    release: &Release,
    asset: &ReleaseAsset,
) -> AppResult<String> {
    if let Some(digest) = asset.digest.as_deref() {
        return normalize_digest(digest);
    }

    let sums = release
        .assets
        .iter()
        .find(|candidate| candidate.name == "SHA256SUMS.txt")
        .ok_or_else(|| eyre!("Release 未提供资产校验值或 SHA256SUMS.txt"))?;
    let sums = download_bytes(client, &sums.browser_download_url, &sums.name).await?;
    let sums = std::str::from_utf8(&sums).wrap_err("SHA256SUMS.txt 不是 UTF-8 文本")?;
    checksum_from_manifest(sums, &asset.name)
}

async fn download_bytes(client: &reqwest::Client, url: &str, name: &str) -> AppResult<Vec<u8>> {
    client
        .get(url)
        .send()
        .await
        .wrap_err_with(|| format!("下载 {name} 失败，请检查网络后重试"))?
        .error_for_status()
        .wrap_err_with(|| format!("下载 {name} 失败，请稍后重试"))?
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .wrap_err_with(|| format!("读取 {name} 响应内容失败"))
}

fn parse_release_version(tag: &str) -> AppResult<Version> {
    Version::parse(tag.trim_start_matches('v'))
        .wrap_err_with(|| format!("Release 标签不是有效 SemVer 版本：{tag}"))
}

fn normalize_digest(digest: &str) -> AppResult<String> {
    let digest = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| eyre!("Release 资产使用了不支持的校验算法：{digest}"))?
        .to_ascii_lowercase();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("Release 资产的 SHA-256 校验值格式无效");
    }
    Ok(digest)
}

fn checksum_from_manifest(manifest: &str, asset_name: &str) -> AppResult<String> {
    let checksum = manifest
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let checksum = fields.next()?;
            let path = fields.next()?;
            path.rsplit('/').next().filter(|name| *name == asset_name)?;
            Some(checksum)
        })
        .next()
        .ok_or_else(|| eyre!("SHA256SUMS.txt 未包含 {asset_name} 的校验值"))?;
    normalize_digest(&format!("sha256:{checksum}"))
}

fn verify_digest(bytes: &[u8], expected: &str) -> AppResult<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected {
        bail!("SHA-256 校验失败：下载文件可能损坏，未修改当前安装。");
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn current_asset_name() -> AppResult<&'static str> {
    Ok("hana-pixiv-downloader-aarch64-apple-darwin.tar.gz")
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn current_asset_name() -> AppResult<&'static str> {
    Ok("hana-pixiv-downloader-x86_64-unknown-linux-gnu.tar.gz")
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn current_asset_name() -> AppResult<&'static str> {
    Ok("hana-pixiv-downloader-x86_64-pc-windows-msvc.zip")
}

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
fn current_asset_name() -> AppResult<&'static str> {
    bail!("当前平台暂不支持自动更新，请从 GitHub Releases 手动安装对应版本。")
}

fn ensure_managed_install(executable: &Path, install_dir: &Path) -> AppResult<()> {
    let marker_path = install_dir.join(MARKER_FILE_NAME);
    if marker_path.exists() {
        let marker = fs::read_to_string(&marker_path)
            .wrap_err_with(|| format!("无法读取安装标记：{}", marker_path.display()))?;
        let marker: InstallMarker =
            serde_json::from_str(&marker).wrap_err("安装标记格式无效，请重新运行官方安装脚本")?;
        if marker.schema_version == 1
            && marker.method == "github-release"
            && marker.repository == REPOSITORY
        {
            return Ok(());
        }
        bail!("安装标记不属于 hpd 官方 Release 安装，请使用原安装方式更新。");
    }

    if is_legacy_official_install(executable) {
        return Ok(());
    }

    bail!(
        "当前 hpd 未由官方安装脚本管理，无法安全原地更新。请使用原安装方式更新（Cargo、Homebrew 或开发构建）。"
    )
}

#[cfg(not(windows))]
fn is_legacy_official_install(executable: &Path) -> bool {
    dirs_next::home_dir().is_some_and(|home| executable == home.join(".local/bin/hpd"))
}

#[cfg(windows)]
fn is_legacy_official_install(executable: &Path) -> bool {
    std::env::var_os("LOCALAPPDATA").is_some_and(|local_app_data| {
        executable
            == PathBuf::from(local_app_data).join("Programs/hana-pixiv-downloader/bin/hpd.exe")
    })
}

fn stage_and_replace(
    executable: &Path,
    asset_name: &str,
    archive: &[u8],
) -> AppResult<UpdateResult> {
    let install_dir = executable
        .parent()
        .ok_or_else(|| eyre!("无法定位 hpd 的安装目录"))?;
    let staging = Builder::new()
        .prefix(".hpd-update-")
        .tempdir_in(install_dir)
        .wrap_err("无法创建更新暂存目录，请检查安装目录写权限")?;
    let candidate = staging.path().join(binary_file_name());
    extract_binary(archive, asset_name, &candidate)?;

    #[cfg(windows)]
    {
        let replacement = install_dir.join(format!(".hpd-update-{}.exe", std::process::id()));
        fs::copy(&candidate, &replacement).wrap_err("无法暂存 Windows 更新文件")?;
        write_install_marker(install_dir)?;
        schedule_windows_replacement(executable, &replacement)?;
        Ok(UpdateResult::Scheduled)
    }

    #[cfg(not(windows))]
    {
        replace_unix_binary(executable, &candidate)?;
        write_install_marker(install_dir)?;
        Ok(UpdateResult::Applied)
    }
}

#[cfg(windows)]
fn binary_file_name() -> &'static str {
    "hpd.exe"
}

#[cfg(not(windows))]
fn binary_file_name() -> &'static str {
    "hpd"
}

fn extract_binary(archive: &[u8], asset_name: &str, destination: &Path) -> AppResult<()> {
    if asset_name.ends_with(".tar.gz") {
        extract_tar_binary(archive, destination)?;
    } else if asset_name.ends_with(".zip") {
        extract_zip_binary(archive, destination)?;
    } else {
        bail!("不支持的 Release 资产格式：{asset_name}");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(destination, fs::Permissions::from_mode(0o755))
            .wrap_err("无法设置更新后二进制的执行权限")?;
    }

    Ok(())
}

fn extract_tar_binary(archive: &[u8], destination: &Path) -> AppResult<()> {
    let decoder = GzDecoder::new(Cursor::new(archive));
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries().wrap_err("无法读取 tar 更新包")? {
        let mut entry = entry.wrap_err("无法读取 tar 更新包条目")?;
        if entry.path().wrap_err("无法读取 tar 条目路径")? == Path::new(binary_file_name())
        {
            if !entry.header().entry_type().is_file() {
                bail!("更新包中的 hpd 不是普通文件");
            }
            let mut destination = fs::File::create(destination).wrap_err("无法写入更新二进制")?;
            io::copy(&mut entry, &mut destination).wrap_err("无法解压更新二进制")?;
            return Ok(());
        }
    }
    bail!("更新包中未找到 {}", binary_file_name())
}

fn extract_zip_binary(archive: &[u8], destination: &Path) -> AppResult<()> {
    let mut archive = ZipArchive::new(Cursor::new(archive)).wrap_err("无法读取 zip 更新包")?;
    let mut entry = archive
        .by_name(binary_file_name())
        .wrap_err_with(|| format!("更新包中未找到 {}", binary_file_name()))?;
    let mut destination = fs::File::create(destination).wrap_err("无法写入更新二进制")?;
    io::copy(&mut entry, &mut destination).wrap_err("无法解压更新二进制")?;
    Ok(())
}

#[cfg(not(windows))]
fn replace_unix_binary(executable: &Path, candidate: &Path) -> AppResult<()> {
    let backup = executable.with_file_name(format!(".hpd-backup-{}", std::process::id()));
    fs::rename(executable, &backup).wrap_err("无法备份当前 hpd，请检查安装目录写权限")?;

    if let Err(error) = fs::rename(candidate, executable) {
        let _ = fs::rename(&backup, executable);
        return Err(error).wrap_err("无法替换 hpd，已恢复原版本");
    }

    let verify_result = ProcessCommand::new(executable)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .wrap_err("无法启动更新后的 hpd 进行自检")
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(eyre!("更新后的 hpd 自检失败，退出码：{status}"))
            }
        });
    if let Err(error) = verify_result {
        let _ = fs::remove_file(executable);
        let _ = fs::rename(&backup, executable);
        return Err(error).wrap_err("更新失败，已恢复原版本");
    }

    fs::remove_file(&backup).wrap_err("更新完成但无法清理旧版本备份")
}

#[cfg(windows)]
fn schedule_windows_replacement(executable: &Path, replacement: &Path) -> AppResult<()> {
    let install_dir = executable
        .parent()
        .ok_or_else(|| eyre!("无法定位 hpd 的安装目录"))?;
    let script = install_dir.join(format!(".hpd-update-{}.ps1", std::process::id()));
    let backup = install_dir.join(format!(".hpd-backup-{}.exe", std::process::id()));
    fs::write(&script, WINDOWS_REPLACEMENT_SCRIPT).wrap_err("无法创建 Windows 更新助手")?;

    if let Err(error) = ProcessCommand::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .arg(executable)
        .arg(replacement)
        .arg(&backup)
        .spawn()
    {
        let _ = fs::remove_file(&script);
        let _ = fs::remove_file(replacement);
        return Err(error).wrap_err("无法启动 Windows 更新助手");
    }

    Ok(())
}

#[cfg(windows)]
const WINDOWS_REPLACEMENT_SCRIPT: &str = r#"
param(
    [string]$Target,
    [string]$Replacement,
    [string]$Backup
)

$ErrorActionPreference = "Stop"
$moved = $false
for ($attempt = 0; $attempt -lt 40; $attempt++) {
    try {
        Move-Item -LiteralPath $Target -Destination $Backup -Force
        $moved = $true
        break
    } catch {
        Start-Sleep -Milliseconds 250
    }
}

if (-not $moved) {
    throw "等待 hpd 退出超时，未修改当前版本。"
}

try {
    Move-Item -LiteralPath $Replacement -Destination $Target -Force
    & $Target --version | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "更新后的 hpd 自检失败。"
    }
    Remove-Item -LiteralPath $Backup -Force
} catch {
    if (Test-Path -LiteralPath $Target) {
        Remove-Item -LiteralPath $Target -Force
    }
    if (Test-Path -LiteralPath $Backup) {
        Move-Item -LiteralPath $Backup -Destination $Target -Force
    }
    throw
} finally {
    if (Test-Path -LiteralPath $Replacement) {
        Remove-Item -LiteralPath $Replacement -Force
    }
    Remove-Item -LiteralPath $PSCommandPath -Force -ErrorAction SilentlyContinue
}
"#;

fn write_install_marker(install_dir: &Path) -> AppResult<()> {
    fs::write(install_dir.join(MARKER_FILE_NAME), INSTALL_MARKER)
        .wrap_err("更新完成但无法写入安装管理标记")
}

enum UpdateResult {
    #[cfg(not(windows))]
    Applied,
    #[cfg(windows)]
    Scheduled,
}

#[cfg(test)]
mod tests {
    use super::{checksum_from_manifest, normalize_digest, parse_release_version, verify_digest};

    #[test]
    fn release_version_removes_only_the_leading_v() {
        assert_eq!(
            parse_release_version("v0.1.4").unwrap().to_string(),
            "0.1.4"
        );
        assert!(parse_release_version("release-0.1.4").is_err());
    }

    #[test]
    fn checksum_manifest_accepts_release_asset_paths() {
        let checksum = "a".repeat(64);
        let manifest = format!("{checksum}  artifacts/x86_64/hpd.tar.gz\n");
        assert_eq!(
            checksum_from_manifest(&manifest, "hpd.tar.gz").unwrap(),
            checksum
        );
    }

    #[test]
    fn digest_validation_rejects_malformed_and_mismatched_values() {
        assert!(normalize_digest("sha512:abc").is_err());
        assert!(
            verify_digest(
                b"hpd",
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
            .is_err()
        );
    }
}
