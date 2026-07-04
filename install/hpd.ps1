param(
    [string]$Version,
    [string]$InstallDir,
    [switch]$NoModifyPath,
    [string]$BaseUrl
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoSlug = "picals-dev/hana-pixiv-downloader"
$DefaultBaseUrl = "https://github.com/$RepoSlug/releases"
$DefaultInstallDir = Join-Path $env:LOCALAPPDATA "Programs\hana-pixiv-downloader\bin"

function Write-Info {
    param([string]$Message)
    Write-Host $Message
}

function Fail {
    param([string]$Message)
    throw $Message
}

function Test-Truthy {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $false
    }

    switch ($Value.Trim().ToLowerInvariant()) {
        "1" { return $true }
        "true" { return $true }
        "yes" { return $true }
        "on" { return $true }
        default { return $false }
    }
}

function Resolve-Tag {
    param([string]$RawVersion)

    if ([string]::IsNullOrWhiteSpace($RawVersion) -or $RawVersion -eq "latest") {
        return "latest"
    }

    if ($RawVersion.StartsWith("v")) {
        return $RawVersion
    }

    return "v$RawVersion"
}

function Resolve-AssetName {
    if ($env:OS -ne "Windows_NT") {
        Fail "当前脚本仅支持 Windows。"
    }

    $architecture = $null
    $runtimeInfoType = [System.Type]::GetType("System.Runtime.InteropServices.RuntimeInformation")
    if ($runtimeInfoType) {
        $osArchitectureProperty = $runtimeInfoType.GetProperty("OSArchitecture")
        if ($osArchitectureProperty) {
            $architecture = $osArchitectureProperty.GetValue($null, @()).ToString()
        }
    }

    if ([string]::IsNullOrWhiteSpace($architecture)) {
        $architecture = $env:PROCESSOR_ARCHITEW6432
    }

    if ([string]::IsNullOrWhiteSpace($architecture)) {
        $architecture = $env:PROCESSOR_ARCHITECTURE
    }

    switch ($architecture.ToUpperInvariant()) {
        "AMD64" { $architecture = "X64" }
        "X86_64" { $architecture = "X64" }
        "ARM64" { $architecture = "Arm64" }
        "X86" { $architecture = "X86" }
    }

    if ($architecture -ne "X64") {
        Fail "当前平台暂不支持自动安装：Windows $architecture。当前仅支持 Windows x64。"
    }

    return "hana-pixiv-downloader-x86_64-pc-windows-msvc.zip"
}

function Resolve-ExpectedHash {
    param(
        [string]$SumsPath,
        [string]$AssetName
    )

    $line = Get-Content -Path $SumsPath | Where-Object { $_ -match [regex]::Escape($AssetName) } | Select-Object -First 1
    if (-not $line) {
        Fail "未在 SHA256SUMS.txt 中找到 $AssetName 的校验值"
    }

    return (($line -split "\s+")[0]).ToLowerInvariant()
}

function Test-PathEntry {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    $parts = $PathValue -split ";" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    foreach ($part in $parts) {
        if ($part.TrimEnd("\") -ieq $Entry.TrimEnd("\")) {
            return $true
        }
    }

    return $false
}

function Add-UserPathEntry {
    param([string]$Entry)

    $current = [Environment]::GetEnvironmentVariable("Path", "User")
    if (Test-PathEntry -PathValue $current -Entry $Entry) {
        return "用户级 PATH 已包含安装目录，无需修改。"
    }

    if ([string]::IsNullOrWhiteSpace($current)) {
        $newValue = $Entry
    } else {
        $newValue = "$current;$Entry"
    }

    [Environment]::SetEnvironmentVariable("Path", $newValue, "User")
    if (-not (Test-PathEntry -PathValue $env:Path -Entry $Entry)) {
        $env:Path = "$Entry;$env:Path"
    }

    return "已写入用户级 PATH，重新打开 PowerShell 或 CMD 后生效。"
}

if (-not $PSBoundParameters.ContainsKey("Version") -and $env:HPD_VERSION) {
    $Version = $env:HPD_VERSION
}

if (-not $PSBoundParameters.ContainsKey("InstallDir") -and $env:HPD_INSTALL_DIR) {
    $InstallDir = $env:HPD_INSTALL_DIR
}

if (-not $PSBoundParameters.ContainsKey("BaseUrl") -and $env:HPD_DIST_BASE_URL) {
    $BaseUrl = $env:HPD_DIST_BASE_URL
}

if (-not $PSBoundParameters.ContainsKey("NoModifyPath") -and (Test-Truthy $env:HPD_NO_MODIFY_PATH)) {
    $NoModifyPath = $true
}

$resolvedTag = Resolve-Tag $Version
$assetName = Resolve-AssetName
$resolvedInstallDir = if ($InstallDir) { $InstallDir } else { $DefaultInstallDir }
$resolvedBaseUrl = if ($BaseUrl) { $BaseUrl } else { $DefaultBaseUrl }

if ($resolvedTag -eq "latest") {
    $assetUrl = "$resolvedBaseUrl/latest/download/$assetName"
    $sumsUrl = "$resolvedBaseUrl/latest/download/SHA256SUMS.txt"
    Write-Info "准备安装最新正式版。"
} else {
    $assetUrl = "$resolvedBaseUrl/download/$resolvedTag/$assetName"
    $sumsUrl = "$resolvedBaseUrl/download/$resolvedTag/SHA256SUMS.txt"
    Write-Info "准备安装指定版本：$resolvedTag"
}

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
$null = New-Item -ItemType Directory -Path $tempRoot

try {
    $assetPath = Join-Path $tempRoot $assetName
    $sumsPath = Join-Path $tempRoot "SHA256SUMS.txt"
    $extractDir = Join-Path $tempRoot "extract"

    Write-Info "下载发行资产中..."
    Invoke-WebRequest -Uri $assetUrl -OutFile $assetPath
    Invoke-WebRequest -Uri $sumsUrl -OutFile $sumsPath

    $expectedHash = Resolve-ExpectedHash -SumsPath $sumsPath -AssetName $assetName
    $actualHash = (Get-FileHash -Path $assetPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($expectedHash -ne $actualHash) {
        Fail "SHA256 校验失败：期望 $expectedHash，实际 $actualHash"
    }

    $null = New-Item -ItemType Directory -Path $extractDir
    Expand-Archive -Path $assetPath -DestinationPath $extractDir -Force

    $binaryPath = Join-Path $extractDir "hpd.exe"
    if (-not (Test-Path $binaryPath)) {
        Fail "压缩包中未找到 hpd.exe 可执行文件"
    }

    $null = New-Item -ItemType Directory -Force -Path $resolvedInstallDir
    Copy-Item -Path $binaryPath -Destination (Join-Path $resolvedInstallDir "hpd.exe") -Force

    if (Test-PathEntry -PathValue $env:Path -Entry $resolvedInstallDir) {
        $pathResult = "当前会话 PATH 已包含安装目录，无需修改。"
    } elseif ($NoModifyPath.IsPresent) {
        $pathResult = "已按要求跳过 PATH 修改。"
    } else {
        $pathResult = Add-UserPathEntry -Entry $resolvedInstallDir
    }

    Write-Info ""
    Write-Info "安装完成。"
    Write-Info "安装路径：$(Join-Path $resolvedInstallDir 'hpd.exe')"
    Write-Info $pathResult
    if (-not (Test-PathEntry -PathValue $env:Path -Entry $resolvedInstallDir)) {
        Write-Info "如需手动加入 PATH，可执行："
        Write-Info "  `$env:Path = '$resolvedInstallDir;' + `$env:Path"
    }
    Write-Info "可先验证命令是否可用："
    Write-Info "  $(Join-Path $resolvedInstallDir 'hpd.exe') --help"
    Write-Info "若 PATH 已生效，也可直接运行："
    Write-Info "  hpd --help"
    Write-Info "安装后请继续执行："
    Write-Info "  hpd setup"
} finally {
    if (Test-Path $tempRoot) {
        Remove-Item -Path $tempRoot -Recurse -Force
    }
}
