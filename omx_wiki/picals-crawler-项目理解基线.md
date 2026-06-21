---
title: "Picals Crawler 项目理解基线"
tags: ["project", "baseline", "architecture", "product", "rust"]
created: 2026-06-17T03:17:25.240Z
updated: 2026-06-21T07:20:39.000Z
sources: []
links: ["typescript-原项目实现观察.md"]
category: architecture
confidence: medium
schemaVersion: 1
---

# Picals Crawler 项目理解基线

## 产品定位

- 面向最终用户的 Pixiv 图片下载 CLI，而不是库、浏览器扩展或 Web 应用。
- 核心承诺是开箱即用：首次通过 setup 完成 `PHPSESSID + userId` 认证元数据初始化、五类下载根目录与通用下载参数配置，之后用户用一行命令执行下载。

## 设计目标

- 产品设计把 `setup` 与 `download user` 定义为 P0。`keyword`、`ranking`、`illust`、`bookmark`、`config` 属于后续扩展。
- 用户体验重点是中文化、低认知成本、进度可视化、断点续传、部分失败不拖垮整体。

## Rust 技术方向

- 技术设计已经明确 Rust 2024 + tokio + reqwest(rustls) + clap derive + inquire + indicatif + serde/toml + eyre/thiserror。
- 目标架构是命令层、认证层、crawler 层、collector 层、downloader 层、config/error/utils 分层，不做过度抽象。

## 与 TS 原项目的关键差异

- TS 原项目是可嵌入的下载库，入口是 `downloadUser` / `downloadKeyword` / `downloadBookmark` 三个函数，依赖全局单例配置对象。
- 新项目要做的是面向终端用户的单二进制 CLI，配置来源与交互模型需要重做，而不是直接一比一翻译 TypeScript API。

## 当前仓库状态

- 早期主链路和 Phase 2 能力已经完成：
  - `download user / illust / keyword / ranking / bookmark` 均已进入主链路
  - `config show/set`、`tags.json`、`.part` 恢复语义、速度与 ETA 统计 seam 已实现
- 本轮 UX 优化已经完成：
  - `setup` 改为多步向导，完整展示凭据、五类 mode roots、通用下载参数与代理配置
  - `config show` 统一展示凭据与普通配置；`config set` 支持 `auth.phpsessid / auth.user_id / download.roots.*`
  - 下载目录模型升级为五类模式 root，并统一通过共享布局解析器输出 `context/illustId/illustId_pn.ext`
  - 请求层已统一到 `PixivRequestRuntime`，覆盖 `429 cooldown / Retry-After / backoff / jitter / fresh-on-retry`
  - 失败项已升级为结构化 manifest，并支持 `picals-crawler retry <manifest-path>` 回放
  - 测试隔离已统一收口到 `src/test_support.rs`
- 当前仓库的主要工作重心已从“实现 UX 优化”切换到“整理文档、准备后续发布与体验打磨”。

## 当前阶段判断

- **Phase 1**：已完成。
- **Phase 2**：已完成。
- **本轮 UX 优化**：已完成，并通过 `cargo fmt --check / cargo check / cargo clippy --all-targets -- -D warnings / cargo test --all-targets`。
- **后续起点**：
  - 可以开始 README、安装分发、发布准备与后续体验打磨。

## 当前主要偏差

- 需继续保持产品/技术文档中的“明文凭据可见性属于冻结产品合同”这一事实一致，避免后续按默认安全偏好回滚。
- 当前 replay / manifest 结构已经稳定，但若后续要继续抽边界，应在不破坏现有 CLI 合同的前提下进行。

## 关联页面

- 参考 [[typescript-原项目实现观察]]
