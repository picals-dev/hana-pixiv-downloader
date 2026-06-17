---
title: "Picals Crawler 项目理解基线"
tags: ["project", "baseline", "architecture", "product", "rust"]
created: 2026-06-17T03:17:25.240Z
updated: 2026-06-17T03:17:25.240Z
sources: []
links: ["typescript-原项目实现观察.md"]
category: architecture
confidence: medium
schemaVersion: 1
---

# Picals Crawler 项目理解基线

## 产品定位
- 面向最终用户的 Pixiv 图片下载 CLI，而不是库、浏览器扩展或 Web 应用。
- 核心承诺是开箱即用：首次通过 setup 完成 PHPSESSID 认证与默认目录配置，之后用户用一行命令执行下载。

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
- Rust 仓库目前仍处于初始化状态：`src/main.rs` 只有 Hello World，`Cargo.toml` 仅有包元信息，还未接入设计文档里列出的依赖与模块。
- 因此现阶段最重要的是先把“设计文档 -> 可运行 CLI 骨架 -> 核心下载链路”的路径压实，而不是直接追求功能齐全。

## 关联页面
- 参考 [[typescript-原项目实现观察]]

