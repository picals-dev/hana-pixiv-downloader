---
title: "Picals Crawler net 层 SSOT 实现记录 2026-06-22"
tags: ["session-log", "net", "pixiv", "architecture", "retry"]
created: 2026-06-22T15:47:42.000Z
updated: 2026-06-22T15:47:42.000Z
sources: []
links: ["picals-crawler-技术设计文档.md", "picals-crawler-项目理解基线.md", "picals-crawler-下载预览与-ranking-调研报告-2026-06-21.md"]
category: session-log
confidence: high
schemaVersion: 1
---

# Picals Crawler net 层 SSOT 实现记录 2026-06-22

## 本轮完成项

- `src/net/` 已拆分为：
  - `catalog.rs`
  - `client.rs`
  - `event.rs`
  - `policy.rs`
  - `session.rs`
  - `state.rs`
  - `transfer.rs`
- `PixivNetSession` 已成为唯一网络 façade：
  - 命令层一次下载只创建一个共享 session
  - `probe -> crawl -> download -> auto-replay` 复用同一 `Arc<PixivNetSession>`
  - 独立 `picals-crawler retry` 复用同一 net stack，但新建 session 实例
- metadata host 与 image host 已分治：
  - metadata client 携带登录 cookie
  - image client 不携带登录 cookie，但保留 `Referer` 与 `User-Agent`
  - cooldown 与策略按 host / request kind 独立收敛
- 图片下载已改为流式写 `.part` 文件：
  - 成功时 rename
  - 失败时清理 `.part`
- 旧 `collector` 概念已删除。
- Pixiv 领域辅助已收口到 `src/pixiv/`：
  - `selector.rs`：响应解析
  - `url.rs`：Pixiv URL 语义解析

## 关键合同落地

- `src/net/` 之外不再持有 Pixiv URL 模板、Referer、header/cookie 注入、`reqwest::Client` 创建入口。
- `auto-replay` 已通过命令级测试证明复用同一 session 实例。
- `standalone retry` 已通过命令级测试证明使用新 session 实例，但仍走同一 stack。

## 主要测试

- `tests/command_test.rs`
  - `auto_replay_reuses_same_session_instance_for_single_download_command`
  - `standalone_retry_uses_new_session_instance_but_same_net_stack`
- `src/net/session.rs`
  - `session_retries_429_with_host_cooldown`
  - `session_retries_503_without_host_cooldown`
  - `image_download_does_not_send_cookie_header`
  - `metadata_request_sends_cookie_header`
  - `stream_download_failure_cleans_part_file`

## 验证结果

- `cargo fmt --check` 通过
- `cargo check` 通过
- `cargo clippy --all-targets -- -D warnings` 通过
- `cargo test --all-targets` 通过
- 静态边界审计通过：
  - `rg -n "ajax/|ranking.php|artworks/\\{|Referer|cookie_header\\(|PixivRequestRuntime::new|reqwest::Client|Client::builder\\(" src`
  - 命中仅存在于 `src/net/**`

## 最终 review gate

- 独立 `code-reviewer`：`APPROVE`
- 独立 `architect`：`CLEAR`
