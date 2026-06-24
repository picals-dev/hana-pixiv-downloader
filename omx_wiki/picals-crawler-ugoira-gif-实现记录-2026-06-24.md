---
title: "Picals Crawler ugoira GIF 实现记录 2026-06-24"
tags: ["session-log", "ugoira", "gif", "pixiv", "replay", "test"]
created: 2026-06-24T08:56:27.000Z
updated: 2026-06-24T08:56:27.000Z
sources:
  - "Cargo.toml"
  - "src/crawler/shared.rs"
  - "src/downloader/mod.rs"
  - "src/downloader/ugoira.rs"
  - "src/failure.rs"
  - "src/net/catalog.rs"
  - "src/net/session.rs"
  - "src/pixiv/selector.rs"
  - "src/replay.rs"
  - "tests/app/ugoira.rs"
  - "tests/cli/ugoira.rs"
  - "tests/contracts/ugoira.rs"
links:
  - "picals-crawler-技术设计文档.md"
  - "picals-crawler-测试指南.md"
  - "picals-crawler-ugoira-gif-下载调研报告-2026-06-24.md"
category: session-log
confidence: high
schemaVersion: 1
---

# Picals Crawler ugoira GIF 实现记录 2026-06-24

## 本轮完成项

- 正式把下载规划从 `pages -> DownloadItem` 升级为 `detail -> ArtworkDownloadPlan`。
- 新增 ugoira 作品分流：
  - 作品 detail 识别 `illustType = 2`
  - 读取 `/ajax/illust/{id}/ugoira_meta`
  - 只使用 `ugoira_meta.originalSrc` 作为素材源
- 新增纯 Rust `zip -> GIF` 管线：
  - 运行时依赖为 `zip + image + gif`
  - 不引入 ffmpeg 或任何外部二进制
  - 最终只产出标准 `.gif`
- 新增 `FailureStage::Convert`，把 GIF 转换失败升级为一等失败语义。
- manifest / replay 已兼容新旧记录：
  - 新记录持久化 `source_url`
  - 旧 `image_url` 记录仍可读取
  - 回放对静态图失败保留直下重试，对 `Collect / Convert` 统一重规划后重试

## 硬合同落实

- `planning/detail cache` 已冻结为硬合同：
  - 下载规划与 `tags.json` 导出复用同一作品 detail 结果
  - 默认不再对同一作品重复请求 detail
- `--dry-run` 仍然无副作用，并增加负向合同：
  - 不显示 `originalSrc`
  - 不显示 zip URL
  - 不显示 `ugoira_source`
- manifest 不再允许持久化 transient path：
  - 不记录 zip / frame / workspace / `.part` / 半成品 `.gif` 路径
- ugoira 中间态统一落到 per-workspace `.picals-workspace`，成功与失败都会清理

## 关键代码落点

- `src/pixiv/selector.rs`
  - 新增 `IllustType`、`UgoiraMetadata`、`select_illust_type()`、`select_ugoira_metadata()`
- `src/net/catalog.rs` / `src/net/session.rs`
  - 新增 `RequestKind::UgoiraMeta`
  - 新增 `RequestKind::UgoiraDownload`
  - 新增 `fetch_ugoira_meta()` 与 ugoira zip 下载入口
- `src/crawler/shared.rs`
  - 新增 `PlannedArtworkBatch`
  - 统一 detail 拉取、资产规划与 tags 导出缓存复用
- `src/downloader/mod.rs` / `src/downloader/ugoira.rs`
  - 新增 `ArtworkDownloadPlan`
  - 新增 `UgoiraDownloadPlan`
  - GIF 编码与清理逻辑独立成模块
- `src/failure.rs` / `src/replay.rs`
  - 新增 `Convert` 阶段
  - 新旧失败记录兼容与回放收口

## 测试增量

- contracts
  - `tests/contracts/ugoira.rs`
  - `tests/fixtures/ugoira_detail.json`
  - `tests/fixtures/ugoira_meta.json`
  - `tests/fixtures/ugoira.zip`
- app
  - `tests/app/ugoira.rs`
  - 覆盖 ugoira 成功、转换失败 cleanup、mixed batch、detail 请求计数证明、convert replay
- cli
  - `tests/cli/ugoira.rs`
  - 覆盖 `illust --dry-run` 非泄漏、真实 GIF 落盘、`user` mixed batch

## 验证结果

- `cargo fmt --check` 通过
- `cargo check` 通过
- `cargo clippy --all-targets -- -D warnings` 通过
- `cargo test --all-targets` 通过

## 已知取舍

- GIF 采用纯 Rust 调色板量化与 10ms delay 量化，优先零外部依赖和可维护性，不追求 ffmpeg 级画质。
- 当前只支持最终 `.gif`，不暴露格式或素材源切换开关。
