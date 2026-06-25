---
title: "Hana Pixiv Downloader 重命名收口实现记录 2026-06-26"
tags: ["session-log", "rename", "cleanup", "cli", "release"]
created: 2026-06-26T00:00:00.000Z
updated: 2026-06-26T00:00:00.000Z
sources:
  - "Cargo.toml"
  - ".github/workflows/release.yml"
  - "src/cli/mod.rs"
  - "src/commands/setup.rs"
  - "src/config.rs"
  - "src/downloader/ugoira.rs"
  - "src/failure.rs"
  - "src/main.rs"
  - "src/net/session.rs"
  - "src/utils/progress.rs"
  - "tests/app/command_dispatch.rs"
  - "tests/app/crawler.rs"
  - "tests/app/ugoira.rs"
  - "tests/cli/help.rs"
  - "tests/cli/parse.rs"
  - "tests/cli/ugoira.rs"
  - "tests/contracts/manifest.rs"
  - "tests/contracts/ugoira.rs"
  - "tests/support/cli.rs"
  - "tests/support/env.rs"
links:
  - "picals-crawler-返工收尾实现记录-2026-06-25.md"
  - "picals-crawler-测试指南.md"
  - "picals-crawler-技术设计文档.md"
category: session-log
confidence: high
schemaVersion: 1
---

# Hana Pixiv Downloader 重命名收口实现记录 2026-06-26

## 本轮完成项

- 完成代码层彻底重命名：
  - package 名改为 `hana-pixiv-downloader`
  - 真实 CLI 二进制名改为 `hpd`
  - 用户可见命令示例统一改为 `hpd ...`
- 清理所有外部可见旧前缀：
  - `PICALS_*` 环境变量改为 `HPD_*`
  - `.picals-workspace` 改为 `.hpd-workspace`
  - `picals-progress` 改为 `hpd-progress`
  - `/tmp/picals` 测试样本改为 `/tmp/hpd`
- 发布打包流程同步改名：
  - Unix / Windows 产物改为打包 `hpd` / `hpd.exe`
  - release archive 仍使用完整项目名 `hana-pixiv-downloader-*`
- 代码注释与启动文案同步收口：
  - `//!` 模块注释统一为 `hpd ...`
  - `setup` 欢迎语与补拉提示统一为新项目名 / `hpd`

## 保留范围

- 历史 wiki 页面保留原始命名，不做内容回填式改写。
- `.gitmodules` 与子模块历史不动。

## 验证结果

- `cargo build` 通过
- `cargo check` 通过
- `cargo test` 通过
- `cargo test --test cli` 通过
- `cargo test --test app` 通过
- `cargo test --test contracts` 通过

## 结论

- 代码层面已无 `Picals Crawler` / `picals-crawler` / `PICALS_*` / `.picals-workspace` 残留。
- 用户日常 CLI 入口现在是 `hpd`，完整项目名保留在 package、发布包名与仓库说明中。
