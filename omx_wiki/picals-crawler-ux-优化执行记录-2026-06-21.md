---
title: "Picals Crawler UX 优化执行记录 2026-06-21"
tags: ["session-log", "ux", "config", "retry", "net", "paths"]
created: 2026-06-21T07:20:39.000Z
updated: 2026-06-21T07:20:39.000Z
sources: []
links: ["picals-crawler-产品设计文档.md", "picals-crawler-技术设计文档.md", "picals-crawler-项目理解基线.md"]
category: session-log
confidence: high
schemaVersion: 1
---

# Picals Crawler UX 优化执行记录 2026-06-21

## 本轮完成项

- `setup` 改为多步向导，展示并允许修改：
  - `auth.phpsessid`
  - `auth.user_id`
  - `download.roots.illust / user / bookmark / keyword / ranking`
  - `download.count / sort / r18 / ai / concurrent / timeout / retry / with_tags`
  - `proxy.url`
- `config show` 统一输出凭据与普通配置，并显式提示输出包含敏感凭据。
- `config set` 已支持 `auth.phpsessid`、`auth.user_id` 与 `download.roots.*`；`download.directory` 冻结为历史兼容读键。
- 下载路径模型已统一通过 `OutputLayout` 解析：
  - `illust_root/{illustId}/...`
  - `user_root/{userId}/{illustId}/...`
  - `bookmark_root/{selfUserId}/{illustId}/...`
  - `keyword_root/{normalizedQuery}/{illustId}/...`
  - `ranking_root/{mode}/{illustId}/...`
- 批量模式已按 `context/illustId/illustId_pn.ext` 组织图片，不再扁平铺放。
- 请求层已统一到 `PixivRequestRuntime`，覆盖：
  - 请求分类
  - 状态码分类
  - 指数退避
  - jitter
  - `Retry-After`
  - `429 cooldown`
  - `fresh-on-retry`
  - `attempt / retry / failure` 事件
- 失败闭环已完成：
  - `DownloadResult` 携带结构化 `failure_records`
  - 命令收尾会对 `retryable=true` 项自动 replay 一次
  - 若仍失败则写入 manifest 到配置目录 `failures/`
  - 显式回放入口固定为 `picals-crawler retry <manifest-path>`
- 测试隔离已统一到 `src/test_support.rs`，收口对 `HOME / XDG_CONFIG_HOME / PICALS_PIXIV_BASE_URL / PICALS_DOWNLOAD_*` 的测试环境控制。

## 新增/关键模块

- `src/output.rs`
- `src/net/mod.rs`
- `src/failure.rs`
- `src/replay.rs`
- `src/commands/retry_cmd.rs`
- `src/test_support.rs`

## 验证结果

- `cargo fmt --check` 通过
- `cargo check` 通过
- `cargo clippy --all-targets -- -D warnings` 通过
- `cargo test --all-targets` 通过

## 最终 review gate

- 独立 `code-reviewer`：`APPROVE`
- 独立 `architect`：`CLEAR`

## 约束说明

- `setup` / `config show` 中的明文凭据可见性是冻结产品合同，而不是当前实现疏漏。
- `download.directory` 仍保留兼容读取，但不再作为新的主配置面或写入键。
