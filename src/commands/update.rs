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
use reqwest::{
    StatusCode, Url,
    header::{HeaderMap, LOCATION, RETRY_AFTER},
    redirect::Policy,
};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tempfile::Builder;
use zip::ZipArchive;

use crate::error::AppResult;

const DEFAULT_RELEASES_BASE_URL: &str =
    "https://github.com/picals-dev/hana-pixiv-downloader/releases";
const REPOSITORY: &str = "picals-dev/hana-pixiv-downloader";
const CHECKSUM_MANIFEST_NAME: &str = "SHA256SUMS.txt";
const MARKER_FILE_NAME: &str = ".hpd-install.json";
const INSTALL_MARKER: &str = r#"{"schema_version":1,"method":"github-release","repository":"picals-dev/hana-pixiv-downloader"}"#;

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
    let base_url = releases_base_url();
    let latest_check_client = build_latest_check_client()?;
    let latest_tag = fetch_latest_tag(&latest_check_client, &base_url).await?;
    let latest_version = parse_release_version(&latest_tag)?;

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
    let download_client = build_download_client()?;
    let expected_digest = fetch_expected_digest(
        &download_client,
        &base_url,
        &latest_tag,
        &latest_version,
        asset_name,
    )
    .await?;

    println!("发现新版本 v{latest_version}，正在下载更新...");
    let archive = download_release_archive(
        &download_client,
        &base_url,
        &latest_tag,
        &latest_version,
        asset_name,
    )
    .await?;
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

fn releases_base_url() -> String {
    std::env::var("HPD_DIST_BASE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_RELEASES_BASE_URL.to_string())
}

fn update_client_builder() -> AppResult<reqwest::ClientBuilder> {
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

    Ok(builder)
}

fn build_latest_check_client() -> AppResult<reqwest::Client> {
    update_client_builder()?
        .redirect(latest_redirect_policy())
        .build()
        .wrap_err("无法创建更新检查客户端")
}

fn build_download_client() -> AppResult<reqwest::Client> {
    update_client_builder()?
        .build()
        .wrap_err("无法创建更新下载客户端")
}

fn latest_redirect_policy() -> Policy {
    Policy::custom(|attempt| {
        let path = attempt.url().path().trim_end_matches('/');
        if path.ends_with("/latest") && attempt.previous().len() < 5 {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

async fn fetch_latest_tag(client: &reqwest::Client, releases_base_url: &str) -> AppResult<String> {
    let latest_url = format!("{releases_base_url}/latest");
    let response = client
        .get(&latest_url)
        .send()
        .await
        .wrap_err("查询最新 Release 失败，请检查网络后重试")?;

    let status = response.status();
    if status.is_redirection() {
        let location = response
            .headers()
            .get(LOCATION)
            .ok_or_else(|| eyre!("最新 Release 响应缺少 Location 重定向头"))?
            .to_str()
            .wrap_err("最新 Release 重定向地址不是有效 UTF-8")?;
        let target = response
            .url()
            .join(location)
            .wrap_err_with(|| format!("最新 Release 重定向地址无效：{location}"))?;
        return extract_tag_from_release_url(&target).ok_or_else(|| {
            eyre!("未找到已发布的正式版 Release（重定向目标不包含版本标签：{target}）")
        });
    }

    if status.is_success() {
        if let Some(tag) = extract_tag_from_release_url(response.url()) {
            return Ok(tag);
        }
        bail!(
            "未找到已发布的正式版 Release（意外响应 HTTP {status}：{}）",
            response.url()
        );
    }

    Err(classify_http_status_error(
        "查询最新 Release",
        status,
        response.headers(),
    ))
}

fn extract_tag_from_release_url(url: &Url) -> Option<String> {
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() >= 2 && segments[segments.len() - 2] == "tag" {
        let tag = segments[segments.len() - 1];
        if !tag.is_empty() {
            return Some(tag.to_string());
        }
    }
    None
}

async fn fetch_expected_digest(
    client: &reqwest::Client,
    releases_base_url: &str,
    tag: &str,
    version: &Version,
    asset_name: &str,
) -> AppResult<String> {
    let sums_url = release_asset_url(releases_base_url, tag, CHECKSUM_MANIFEST_NAME);
    let sums = download_bytes(client, &sums_url, CHECKSUM_MANIFEST_NAME, |status| {
        if status == StatusCode::NOT_FOUND {
            Some(eyre!(
                "Release v{version} 缺少校验文件 {CHECKSUM_MANIFEST_NAME}，可能仍在发布中，请稍后重试"
            ))
        } else {
            None
        }
    })
    .await?;
    let sums = std::str::from_utf8(&sums).wrap_err("SHA256SUMS.txt 不是 UTF-8 文本")?;
    checksum_from_manifest(sums, asset_name)
        .with_context(|| format!("Release v{version} 缺少当前平台资产校验值：{asset_name}"))
}

async fn download_release_archive(
    client: &reqwest::Client,
    releases_base_url: &str,
    tag: &str,
    version: &Version,
    asset_name: &str,
) -> AppResult<Vec<u8>> {
    let archive_url = release_asset_url(releases_base_url, tag, asset_name);
    download_bytes(client, &archive_url, asset_name, |status| {
        if status == StatusCode::NOT_FOUND {
            Some(eyre!("Release v{version} 缺少当前平台资产：{asset_name}"))
        } else {
            None
        }
    })
    .await
}

fn release_asset_url(releases_base_url: &str, tag: &str, file_name: &str) -> String {
    format!("{releases_base_url}/download/{tag}/{file_name}")
}

async fn download_bytes(
    client: &reqwest::Client,
    url: &str,
    name: &str,
    map_status: impl FnOnce(StatusCode) -> Option<eyre::Report>,
) -> AppResult<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .await
        .wrap_err_with(|| format!("下载 {name} 失败，请检查网络后重试"))?;
    let status = response.status();
    if !status.is_success() {
        if let Some(error) = map_status(status) {
            return Err(error);
        }
        return Err(classify_http_status_error(
            &format!("下载 {name}"),
            status,
            response.headers(),
        ));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .wrap_err_with(|| format!("读取 {name} 响应内容失败"))
}

fn classify_http_status_error(
    action: &str,
    status: StatusCode,
    headers: &HeaderMap,
) -> eyre::Report {
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        let retry_hint = headers
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .map(|value| format!("（建议 {value} 秒后重试）"))
            .unwrap_or_default();
        return eyre!(
            "{action}被远端拒绝（HTTP {status}），可能是当前出口 IP 触发了限流{retry_hint}；请稍后重试，或切换代理节点 / 设置 HPD_PROXY_URL 后重试。"
        );
    }

    eyre!("{action}失败（HTTP {status}），请稍后重试")
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

#[cfg(all(target_os = "android", target_arch = "aarch64"))]
fn current_asset_name() -> AppResult<&'static str> {
    Ok("hana-pixiv-downloader-aarch64-linux-android.tar.gz")
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn current_asset_name() -> AppResult<&'static str> {
    Ok("hana-pixiv-downloader-x86_64-pc-windows-msvc.zip")
}

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "android", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64"),
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
    let script_with_bom = format!("\u{FEFF}{}", WINDOWS_REPLACEMENT_SCRIPT);
    fs::write(&script, script_with_bom).wrap_err("无法创建 Windows 更新助手")?;

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
    use semver::Version;
    use sha2::{Digest, Sha256};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::{
        Url, checksum_from_manifest, download_release_archive, extract_tag_from_release_url,
        fetch_expected_digest, fetch_latest_tag, latest_redirect_policy, normalize_digest,
        parse_release_version, verify_digest,
    };

    fn test_latest_client() -> reqwest::Client {
        reqwest::Client::builder()
            .redirect(latest_redirect_policy())
            .build()
            .unwrap()
    }

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

    #[test]
    fn release_url_tag_extraction_handles_standard_and_invalid_paths() {
        let standard =
            Url::parse("https://github.com/picals-dev/hana-pixiv-downloader/releases/tag/v0.3.2")
                .unwrap();
        assert_eq!(
            extract_tag_from_release_url(&standard).as_deref(),
            Some("v0.3.2")
        );

        let with_slash_and_query = Url::parse(
            "https://github.com/picals-dev/hana-pixiv-downloader/releases/tag/v0.3.2/?foo=1#top",
        )
        .unwrap();
        assert_eq!(
            extract_tag_from_release_url(&with_slash_and_query).as_deref(),
            Some("v0.3.2")
        );

        let mirror = Url::parse("https://mirror.example.com/hpd/tag/v0.3.2").unwrap();
        assert_eq!(
            extract_tag_from_release_url(&mirror).as_deref(),
            Some("v0.3.2")
        );

        let no_release =
            Url::parse("https://github.com/picals-dev/hana-pixiv-downloader/releases").unwrap();
        assert_eq!(extract_tag_from_release_url(&no_release), None);

        let empty_tag =
            Url::parse("https://github.com/picals-dev/hana-pixiv-downloader/releases/tag/")
                .unwrap();
        assert_eq!(extract_tag_from_release_url(&empty_tag), None);
    }

    #[tokio::test]
    async fn fetch_latest_tag_reads_redirect_location_without_fetching_tag_page() {
        let server = MockServer::start().await;
        let tag_url = format!("{}/releases/tag/v0.3.2", server.uri());

        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", tag_url.as_str()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/releases/tag/v0.3.2"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let tag = fetch_latest_tag(&test_latest_client(), &format!("{}/releases", server.uri()))
            .await
            .unwrap();
        assert_eq!(tag, "v0.3.2");
    }

    #[tokio::test]
    async fn fetch_latest_tag_follows_intermediate_latest_redirects() {
        let server = MockServer::start().await;
        let next_latest = format!("{}/new-repo/releases/latest", server.uri());
        let tag_url = format!("{}/new-repo/releases/tag/v0.3.2", server.uri());

        Mock::given(method("GET"))
            .and(path("/old-repo/releases/latest"))
            .respond_with(
                ResponseTemplate::new(301).insert_header("location", next_latest.as_str()),
            )
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/new-repo/releases/latest"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", tag_url.as_str()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/new-repo/releases/tag/v0.3.2"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;

        let tag = fetch_latest_tag(
            &test_latest_client(),
            &format!("{}/old-repo/releases", server.uri()),
        )
        .await
        .unwrap();
        assert_eq!(tag, "v0.3.2");
    }

    #[tokio::test]
    async fn fetch_latest_tag_supports_relative_location_and_rejects_missing_tag() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/ok/releases/latest"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("location", "/ok/releases/tag/v0.3.2"),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/empty/releases/latest"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/empty/releases"))
            .mount(&server)
            .await;

        let client = test_latest_client();
        let tag = fetch_latest_tag(&client, &format!("{}/ok/releases", server.uri()))
            .await
            .unwrap();
        assert_eq!(tag, "v0.3.2");

        let err = fetch_latest_tag(&client, &format!("{}/empty/releases", server.uri()))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("未找到已发布的正式版 Release"));
    }

    #[tokio::test]
    async fn fetch_latest_tag_reports_actionable_rate_limit_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/releases/latest"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "60"))
            .mount(&server)
            .await;

        let err = fetch_latest_tag(&test_latest_client(), &format!("{}/releases", server.uri()))
            .await
            .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("429"));
        assert!(message.contains("限流"));
        assert!(message.contains("60 秒"));
    }

    #[tokio::test]
    async fn download_and_verify_pins_checksum_and_archive_to_resolved_tag() {
        let server = MockServer::start().await;
        let payload = b"fake-hpd-archive-bytes";
        let expected_hash = format!("{:x}", Sha256::digest(payload));
        let manifest = format!("{expected_hash}  artifacts/macos/hpd-test.tar.gz\n");
        let cdn_url = format!("{}/cdn/hpd-test.tar.gz", server.uri());

        Mock::given(method("GET"))
            .and(path("/releases/download/v0.3.2/SHA256SUMS.txt"))
            .respond_with(ResponseTemplate::new(200).set_body_string(manifest))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/releases/download/v0.3.2/hpd-test.tar.gz"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", cdn_url.as_str()))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/cdn/hpd-test.tar.gz"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.as_slice()))
            .expect(1)
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let base_url = format!("{}/releases", server.uri());
        let version = Version::parse("0.3.2").unwrap();

        let digest =
            fetch_expected_digest(&client, &base_url, "v0.3.2", &version, "hpd-test.tar.gz")
                .await
                .unwrap();
        let archive =
            download_release_archive(&client, &base_url, "v0.3.2", &version, "hpd-test.tar.gz")
                .await
                .unwrap();
        verify_digest(&archive, &digest).unwrap();
    }

    #[tokio::test]
    async fn missing_checksum_or_archive_reports_clear_not_found_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/releases/download/v0.3.2/SHA256SUMS.txt"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/releases/download/v0.3.2/hpd-test.tar.gz"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let base_url = format!("{}/releases", server.uri());
        let version = Version::parse("0.3.2").unwrap();

        let sums_err =
            fetch_expected_digest(&client, &base_url, "v0.3.2", &version, "hpd-test.tar.gz")
                .await
                .unwrap_err();
        assert!(sums_err.to_string().contains("SHA256SUMS.txt"));

        let archive_err =
            download_release_archive(&client, &base_url, "v0.3.2", &version, "hpd-test.tar.gz")
                .await
                .unwrap_err();
        assert!(archive_err.to_string().contains("缺少当前平台资产"));
    }
}
