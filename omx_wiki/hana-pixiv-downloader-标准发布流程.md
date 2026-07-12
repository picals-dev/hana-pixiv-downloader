---
title: "Hana Pixiv Downloader 标准发布流程"
tags: ["release", "git", "tag", "changelog", "github-actions", "ssot", "guide"]
created: 2026-07-13T00:00:00.000Z
updated: 2026-07-13T00:00:00.000Z
sources: ["Cargo.toml", "Cargo.lock", "CHANGELOG.md", ".github/workflows/release.yml"]
links: ["picals-crawler-测试指南.md", "hana-pixiv-downloader-重命名收口实现记录-2026-06-26.md"]
category: convention
confidence: high
schemaVersion: 1
---

# Hana Pixiv Downloader 标准发布流程

> 本文档是 `hana-pixiv-downloader` 版本发布、提交、tag、Changelog、推送与 GitHub Release 验证的 **SSOT**。  
> 凡是执行版本升级、正式发布、补发 tag、更新发布说明或排查 release workflow，必须先阅读本文。

## 1. 发布目标与完成条件

一次标准发布只有在以下条件全部满足后才算完成：

- 版本号、Changelog 和 release notes 位于同一个发布提交中
- 发布提交通过本地完整质量门禁
- `master` 与版本 tag 指向同一个提交并已推送到远端
- GitHub Actions 的 `verify`、三平台 `build` 和最终 `release` job 全部成功
- GitHub Release 已生成 macOS、Linux、Windows 产物与 `SHA256SUMS.txt`
- 本地工作树干净，远端 `master` / tag 与本地提交一致

只完成 commit、只推送 tag，或 workflow 仍在运行，都不能宣称发布完成。

## 2. 发布前检查

发布前先确认仓库状态与版本边界：

```bash
git status --short --branch
git log --oneline --decorate -10
git tag --sort=-version:refname
git remote -v
git ls-remote origin refs/heads/master refs/tags/vX.Y.Z
```

硬约束：

- 从与 `origin/master` 同步的 `master` 发布
- 工作树中的变更必须全部属于本次发布
- 目标 tag 在本地和远端都不能已存在
- 版本遵循 SemVer；纯缺陷修复默认递增 patch 版本
- 推送前确认远端仓库地址有效；若 GitHub 返回仓库迁移提示，应更新 `origin` 到规范地址

## 3. 发布元数据

发布提交必须同时包含以下文件：

### `Cargo.toml`

- 更新 `[package].version`

### `Cargo.lock`

- 只更新 `[[package]] name = "hana-pixiv-downloader"` 对应的版本
- 不应因发布顺手升级无关依赖

### `CHANGELOG.md`

- 新增 `## [X.Y.Z] - YYYY-MM-DD`
- 按实际内容使用 `Added`、`Fixed`、`Improved`、`Distribution`、`Notes`
- 只描述用户可感知行为、兼容性与分发变化，不记录内部执行过程

### `.github/release-notes/vX.Y.Z.md`

- 文件名必须与 tag 完全一致
- `.github/workflows/release.yml` 使用该文件作为 GitHub Release 正文
- 缺少该文件会导致最后的 `release` job 失败
- 至少包含版本标题、主要变更、安装方式和必要的兼容性说明

## 4. 本地发布门禁

按以下顺序执行，任一步失败都应停止发布并修复：

```bash
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --release --locked
cargo publish --dry-run --locked --allow-dirty
git diff --check
```

说明：

- 提交前 `cargo publish --dry-run` 会因工作树有未提交变更而拒绝运行，因此此阶段允许使用 `--allow-dirty`
- `--allow-dirty` 只用于验证待提交发布快照，不允许掩盖无关工作树变更
- 测试分层与测试修改规则遵循 [[Picals Crawler 测试指南]]

## 5. 提交规范

先只暂存本次发布文件，再检查 staged diff：

```bash
git add -- <本次发布文件>
git status --short
git diff --cached --check
git diff --cached --stat
git diff --cached
```

提交信息必须为单行中文 Conventional Commit：

```text
<type>: <中文详情>
```

常用类型：

- `fix:`：缺陷修复发布
- `feat:`：新增用户能力
- `docs:`：纯文档发布准备
- `chore:`：不改变用户功能的维护发布

不要使用只有版本号的提交信息。发布实现、测试、版本号、Changelog 和 release notes 应处于同一个提交。

提交后，在干净工作树上重新执行：

```bash
cargo publish --dry-run --locked
```

只有不带 `--allow-dirty` 的验证也成功，才能创建 tag。

## 6. Tag 与推送顺序

仓库历史使用轻量 tag，继续保持一致：

```bash
git tag vX.Y.Z <release-commit>
git show --no-patch --oneline --decorate vX.Y.Z
```

推送必须先分支、后 tag：

```bash
git push origin master
git push origin vX.Y.Z
```

原因：tag 推送会立即触发 release workflow；先推 `master` 可以保证远端主分支已经包含 tag 指向的发布提交。

推送后核对：

```bash
git ls-remote origin refs/heads/master refs/tags/vX.Y.Z
git status --short --branch
```

`master` 与 tag 必须解析到同一个 commit SHA。

## 7. GitHub Release 验证

tag push 会触发 `.github/workflows/release.yml`。标准 job 顺序是：

1. `verify`
   - `cargo test --locked --all-targets`
   - `cargo publish --dry-run --locked`
2. 三平台 `build`
   - `aarch64-apple-darwin`
   - `x86_64-unknown-linux-gnu`
   - `x86_64-pc-windows-msvc`
3. `release`
   - 下载三平台产物
   - 生成 `SHA256SUMS.txt`
   - 使用对应 release notes 创建 GitHub Release

使用 GitHub CLI 跟踪到终态：

```bash
gh run list --repo picals-dev/hana-pixiv-downloader --limit 5
gh run watch <run-id> --repo picals-dev/hana-pixiv-downloader --exit-status --interval 10
gh release view vX.Y.Z --repo picals-dev/hana-pixiv-downloader
```

最终 Release 应包含：

- `hana-pixiv-downloader-aarch64-apple-darwin.tar.gz`
- `hana-pixiv-downloader-x86_64-unknown-linux-gnu.tar.gz`
- `hana-pixiv-downloader-x86_64-pc-windows-msvc.zip`
- `SHA256SUMS.txt`

## 8. crates.io 边界

当前 release workflow 只执行：

```bash
cargo publish --dry-run --locked
```

它不会把 crate 实际上传到 crates.io。除非用户明确要求并确认凭据、版本与外部发布影响，否则标准 GitHub Release 流程不得擅自执行真实 `cargo publish`。

## 9. 失败处理

- 本地门禁失败：修复后从对应门禁重新开始，不创建 tag
- commit 后干净包验证失败：追加或重做发布修复，再重新验证；不要创建 tag
- `master` 已推送但 tag 未推送：先修复并推送新的发布提交，再创建 tag
- tag 已推送且 workflow 失败：不要静默移动或复用公开 tag；修复后发布新的 patch 版本
- release notes 缺失：补齐后使用新的 patch 版本发布，避免重写已公开 tag
- 三平台任一构建失败：发布未完成，必须明确报告失败 job 和日志链接

## 10. 发布检查清单

- [ ] 目标版本与 tag 不存在
- [ ] `Cargo.toml` / `Cargo.lock` 版本一致
- [ ] `CHANGELOG.md` 已更新
- [ ] `.github/release-notes/vX.Y.Z.md` 已创建
- [ ] fmt / check / clippy / test 全部使用 locked 依赖并通过
- [ ] release build 与 publish dry-run 通过
- [ ] staged diff 只包含本次发布内容
- [ ] 提交信息为单行 `<type>: <中文详情>`
- [ ] 干净提交上的 publish dry-run 通过
- [ ] 轻量 tag 指向发布提交
- [ ] 先推 `master`，再推 tag
- [ ] 远端 `master` 与 tag SHA 一致
- [ ] GitHub Actions 全部成功
- [ ] GitHub Release 与四个资产可见
- [ ] 本地工作树干净

## 11. 已验证基线

`v0.1.4` 发布完整走通了本文流程：本地门禁、干净包验证、轻量 tag、分支与 tag 顺序推送、三平台构建、SHA256 生成和 GitHub Release 创建均成功。后续流程变更必须同步更新本文与 `.github/workflows/release.yml`，避免文档与自动化分叉。

