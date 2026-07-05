# Changelog

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

## [0.1.1] - 2026-06-28

### Notes

- 修改 `release.yml`，添加 github token

## [0.1.2] - 2026-07-03

### Added

- 根据平台（Windows、MacOS & Linux）调整 `hpd setup` 的默认配置目录
- 添加新配置项：`download.batch_layout`，用于配置作品图片的下载布局
  - `mixed` 单输出平铺，多输出作品分目录
  - `per_illust` 所有作品都分目录
  - `flat` 所有作品都直接平铺
- 新增 `hpd organize` 命令。如果调整了布局，但是原下载目录还保持原来的状态，可以运行此命令整理下载目录
- 优化 `hpd config` 体验
  - `hpd config show` 支持 table 展示
  - `hpd config set` 支持 table 展示原有配置项
  - `hpd config set <KEY>` 支持字段层面 CLI 交互配置

### Distribution

- 提供标准安装脚本（Windows、MacOS & Linux）
- 增加预安装验证 CI

## [0.1.3] - 2026-07-05

### Added

- 新增 `hpd config clean` 命令，可一键清空整个配置目录并恢复到首次使用状态

### Improved

- 优化已有配置时的 `hpd setup` 体验
  - 已保存的 `PHPSESSID` 支持直接回车复用
  - 自动识别 `userId` 失败时，会根据 `PHPSESSID` 是否变更决定是否复用已保存 `userId`

### Distribution

- 修复 Windows PowerShell 安装脚本
  - README 中的一行安装命令不再依赖 `ExecutionPolicy Bypass`
  - 修复脚本编码与平台架构识别问题
