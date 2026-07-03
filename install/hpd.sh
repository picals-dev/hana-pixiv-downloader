#!/usr/bin/env bash

set -euo pipefail

readonly REPO_SLUG="picals-dev/hana-pixiv-downloader"
readonly DEFAULT_BASE_URL="https://github.com/${REPO_SLUG}/releases"
readonly DEFAULT_INSTALL_DIR="${HOME}/.local/bin"

usage() {
  cat <<'EOF'
用法：
  bash install/hpd.sh [--version <VERSION>] [--install-dir <PATH>] [--no-modify-path]

参数：
  --version <VERSION>     安装指定版本，例如 v0.1.1 或 0.1.1
  --install-dir <PATH>    覆盖默认安装目录，默认为 ~/.local/bin
  --no-modify-path        只安装二进制，不自动修改 PATH
  --help                  显示本帮助

环境变量：
  HPD_VERSION
  HPD_INSTALL_DIR
  HPD_NO_MODIFY_PATH
  HPD_DIST_BASE_URL
EOF
}

say() {
  printf '%s\n' "$*"
}

die() {
  printf '错误：%s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "缺少依赖命令：$1"
}

expand_home() {
  local path="$1"
  if [[ "$path" == "~" ]]; then
    printf '%s\n' "$HOME"
    return
  fi

  if [[ "$path" == "~/"* ]]; then
    printf '%s/%s\n' "$HOME" "${path#~/}"
    return
  fi

  printf '%s\n' "$path"
}

normalize_tag() {
  local raw="$1"
  if [[ -z "$raw" || "$raw" == "latest" ]]; then
    printf 'latest\n'
    return
  fi

  if [[ "$raw" == v* ]]; then
    printf '%s\n' "$raw"
    return
  fi

  printf 'v%s\n' "$raw"
}

is_truthy() {
  local normalized
  normalized="$(printf '%s' "${1:-}" | tr '[:upper:]' '[:lower:]')"
  case "$normalized" in
    1|true|yes|on) return 0 ;;
    *) return 1 ;;
  esac
}

to_lower() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]'
}

resolve_asset_name() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}:${arch}" in
    Darwin:arm64|Darwin:aarch64)
      printf 'hana-pixiv-downloader-aarch64-apple-darwin.tar.gz\n'
      ;;
    Linux:x86_64)
      printf 'hana-pixiv-downloader-x86_64-unknown-linux-gnu.tar.gz\n'
      ;;
    *)
      die "当前平台暂不支持自动安装：${os} ${arch}。当前仅支持 macOS Apple Silicon 与 Linux x86_64。"
      ;;
  esac
}

sha256_file() {
  local path="$1"
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
    return
  fi

  die "未找到 SHA256 校验工具（需要 shasum 或 sha256sum）"
}

extract_expected_hash() {
  local sums_path="$1"
  local asset_name="$2"
  local hash
  hash="$(awk -v asset="$asset_name" 'index($0, asset) {print $1; exit}' "$sums_path")"
  [[ -n "$hash" ]] || die "未在 SHA256SUMS.txt 中找到 ${asset_name} 的校验值"
  printf '%s\n' "$(to_lower "$hash")"
}

path_contains_dir() {
  local dir="$1"
  case ":$PATH:" in
    *":$dir:"*) return 0 ;;
    *) return 1 ;;
  esac
}

choose_profile_file() {
  local shell_name profile
  shell_name="$(basename "${SHELL:-}")"

  case "$shell_name" in
    zsh)
      profile="${HOME}/.zprofile"
      ;;
    bash)
      if [[ -f "${HOME}/.bash_profile" ]]; then
        profile="${HOME}/.bash_profile"
      else
        profile="${HOME}/.profile"
      fi
      ;;
    *)
      profile="${HOME}/.profile"
      ;;
  esac

  printf '%s\n' "$profile"
}

append_path_entry() {
  local install_dir="$1"
  local profile
  profile="$(choose_profile_file)"

  local marker="# hana-pixiv-downloader"
  local line="export PATH=\"${install_dir}:\$PATH\""

  mkdir -p "$(dirname "$profile")"
  touch "$profile"

  if grep -F "$line" "$profile" >/dev/null 2>&1; then
    say "PATH 配置已存在：${profile}"
    return
  fi

  {
    printf '\n%s\n' "$marker"
    printf '%s\n' "$line"
  } >>"$profile"

  say "已写入 PATH 到 ${profile}"
}

version_arg=""
install_dir_arg=""
no_modify_path_arg=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      [[ $# -ge 2 ]] || die "--version 需要一个值"
      version_arg="$2"
      shift 2
      ;;
    --install-dir)
      [[ $# -ge 2 ]] || die "--install-dir 需要一个路径"
      install_dir_arg="$2"
      shift 2
      ;;
    --no-modify-path)
      no_modify_path_arg="1"
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      die "不支持的参数：$1。可运行 --help 查看帮助。"
      ;;
  esac
done

need_cmd curl
need_cmd tar
need_cmd mktemp
need_cmd awk
need_cmd grep

version_raw="${version_arg:-${HPD_VERSION:-latest}}"
install_dir_raw="${install_dir_arg:-${HPD_INSTALL_DIR:-$DEFAULT_INSTALL_DIR}}"
base_url="${HPD_DIST_BASE_URL:-$DEFAULT_BASE_URL}"

if [[ -n "$no_modify_path_arg" ]]; then
  no_modify_path="1"
elif is_truthy "${HPD_NO_MODIFY_PATH:-}"; then
  no_modify_path="1"
else
  no_modify_path="0"
fi

tag="$(normalize_tag "$version_raw")"
asset_name="$(resolve_asset_name)"
install_dir="$(expand_home "$install_dir_raw")"

if [[ "$tag" == "latest" ]]; then
  asset_url="${base_url}/latest/download/${asset_name}"
  sums_url="${base_url}/latest/download/SHA256SUMS.txt"
  say "准备安装最新正式版。"
else
  asset_url="${base_url}/download/${tag}/${asset_name}"
  sums_url="${base_url}/download/${tag}/SHA256SUMS.txt"
  say "准备安装指定版本：${tag}"
fi

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

asset_path="${tmpdir}/${asset_name}"
sums_path="${tmpdir}/SHA256SUMS.txt"

say "下载发行资产中..."
curl -fsSL --retry 3 "$asset_url" -o "$asset_path" || die "下载二进制失败：${asset_url}"
curl -fsSL --retry 3 "$sums_url" -o "$sums_path" || die "下载校验文件失败：${sums_url}"

expected_hash="$(extract_expected_hash "$sums_path" "$asset_name")"
actual_hash="$(sha256_file "$asset_path")"
actual_hash="$(to_lower "$actual_hash")"

if [[ "$expected_hash" != "$actual_hash" ]]; then
  die "SHA256 校验失败：期望 ${expected_hash}，实际 ${actual_hash}"
fi

mkdir -p "$install_dir"
tar -xzf "$asset_path" -C "$tmpdir"

binary_path="${tmpdir}/hpd"
[[ -f "$binary_path" ]] || die "压缩包中未找到 hpd 可执行文件"

cp "$binary_path" "${install_dir}/hpd"
chmod +x "${install_dir}/hpd"

path_result=""
if path_contains_dir "$install_dir"; then
  path_result="PATH 已包含安装目录，无需修改。"
elif [[ "$no_modify_path" == "1" ]]; then
  path_result="已按要求跳过 PATH 修改。"
else
  append_path_entry "$install_dir"
  path_result="已尝试写入 PATH，重新打开终端后生效。"
fi

say
say "安装完成。"
say "安装路径：${install_dir}/hpd"
say "$path_result"
if ! path_contains_dir "$install_dir"; then
  say "如需手动加入 PATH，可执行："
  say "  export PATH=\"${install_dir}:\$PATH\""
fi
say "可先验证命令是否可用："
say "  ${install_dir}/hpd --help"
say "若 PATH 已生效，也可直接运行："
say "  hpd --help"
say "安装后请继续执行："
say "  hpd setup"
