# Changelog

所有重要变更都会记录在这个文件里。

## [0.1.0] - 2026-06-27

### Added

- 首次公开发布 `hana-pixiv-downloader`，安装包名为 `hana-pixiv-downloader`，CLI 命令为 `hpd`
- 提供 `setup` 向导，支持通过 `PHPSESSID` 初始化认证、下载目录、代理与默认下载参数
- 支持以下下载入口：
  - `download user`
  - `download keyword`
  - `download ranking`
  - `download illust`
  - `download bookmark`
  - 直接粘贴 Pixiv URL 的 `download <pixiv_url>`
- 支持 ugoira 自动导出 GIF
- 支持 `--dry-run`、`--with-tags`、`--proxy`、`--verbose`、下载并发/超时/重试等常用控制项
- 支持失败清单 manifest 与 `retry` 回放补救
- 支持 `config show` / `config set` 管理本地配置

### Distribution

- 提供 Cargo 安装路径：`cargo install hana-pixiv-downloader --locked`
- 提供 GitHub Releases 多平台预编译二进制：
  - macOS `aarch64`
  - macOS `x86_64`
  - Linux `x86_64`
  - Windows `x86_64`

### Notes

- `setup` 与 `config show` 会明文显示凭据，使用时应避免录屏或共享屏幕
