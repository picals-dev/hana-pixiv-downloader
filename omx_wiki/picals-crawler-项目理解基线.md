---
title: "Picals Crawler 项目理解基线"
tags: ["project", "baseline", "architecture", "product", "rust"]
created: 2026-06-17T03:17:25.240Z
updated: 2026-06-19T00:00:00.000Z
sources: []
links: ["typescript-原项目实现观察.md"]
category: architecture
confidence: medium
schemaVersion: 1
---

# Picals Crawler 项目理解基线

## 产品定位

- 面向最终用户的 Pixiv 图片下载 CLI，而不是库、浏览器扩展或 Web 应用。
- 核心承诺是开箱即用：首次通过 setup 完成 `PHPSESSID + userId` 认证元数据初始化与默认目录配置，之后用户用一行命令执行下载。

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

- Rust CLI 骨架已经落地，不再是初始化仓库：
  - `src/main.rs` 已完成 CLI 解析与命令分发
  - `Cargo.toml` 已接入 `tokio / reqwest / clap / inquire / indicatif / serde / toml / eyre / thiserror` 等核心依赖
  - `auth / cli / collector / commands / config / crawler / downloader / error / utils` 模块目录已经成型
- Phase 1 的核心目标“跑通 `download user` 完整链路”已经完成：
  - `setup` 可保存 `PHPSESSID`、当前账号 `userId` 与默认下载目录
  - `download user` 已打通配置合并、凭据读取、作品 ID 采集、图片 URL 采集、并发下载、跳过已存在文件、失败汇总
  - `cargo test` 当前通过，且存在 selector 单测与 `wiremock` 集成测试
- Phase 2 的主体实现已经完成：
  - `download illust / keyword / ranking` 已落地并补齐主链路测试
  - `download bookmark` 已落地，并基于 setup 保存的 `userId` 完成收藏分页、去重、count 截断、`tags.json` 导出与下载闭环
  - `config show/set` 已完成 Phase 2 约束收敛：`download.sort` 只允许 `date_desc / date_asc`，`popular_desc` 作为迁移错误处理
  - `tags.json` 导出、`.part` 恢复语义、速度与 ETA 的统计 seam 已实现并有测试覆盖
- 当前的主要工作重心已从“推进 Phase 2”切换为“同步文档状态并为后续体验打磨做准备”。

## 当前阶段判断

- **Phase 1**：已完成。
- **Phase 2**：主体已完成。
  - 已完成：`download illust / keyword / ranking / bookmark`、`config show/set` 的 Phase 2 约束、`tags.json`、`.part` 恢复语义、速度与 ETA 统计 seam
- **Phase 3 起点**：
  - 可以开始体验打磨、发布准备和文档整理

## 当前主要偏差

- 产品文档仍把 `--with-tags` 默认值描述为 `true`，但当前实现保持显式 opt-in，默认值仍为 `false`。
- 需继续保持产品文档与技术设计文档中的 setup / bookmark 说明和当前代码事实一致，避免后续文档回漂。

## 关联页面

- 参考 [[typescript-原项目实现观察]]
