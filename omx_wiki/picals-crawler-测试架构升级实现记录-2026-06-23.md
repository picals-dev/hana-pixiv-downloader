---
title: "Picals Crawler 测试架构升级实现记录 2026-06-23"
tags: ["session-log", "contracts", "migration"]
created: 2026-06-23T09:13:17.000Z
updated: 2026-06-23T09:13:17.000Z
sources: []
links: ["picals-crawler-技术设计文档.md", "picals-crawler-项目理解基线.md", "picals-crawler-net-层-ssot-实现记录-2026-06-22.md", "picals-crawler-测试指南.md"]
category: session-log
confidence: high
schemaVersion: 1
---

# Picals Crawler 测试架构升级实现记录 2026-06-23

## 本轮完成项

- 正式落地四层测试架构 SSOT：
  - Layer 1：`src` 邻近单元测试
  - Layer 2：`tests/contracts.rs`
  - Layer 3：`tests/app.rs`
  - Layer 4：`tests/cli.rs`
  - opt-in live：`tests/live.rs`
- 旧顶层散文件已拆分并删除：
  - `tests/selector_test.rs`
  - `tests/crawler_test.rs`
  - `tests/command_test.rs`
- `tests/support/` 已明确为 Layer 2/3/4 专用 support：
  - `fixtures.rs`
  - `env.rs`
  - `cli.rs`
  - `mock_pixiv.rs`

## 关键边界收口

- `src/test_support.rs` 最终公共面只保留：
  - `EnvVarGuard`
  - `lock_env`
  - `ConfigHomeGuard`
  - `set_config_home`
  - `SessionObserverGuard`
  - `install_session_observer`
- `current_session_observer` 已迁入 `src/net/test_hook.rs`，并通过 `attach_session_observer(...)` 统一接入 `download_common` 与 `replay` 两条 session 构造路径。
- Layer 2 contracts 已固定为只依赖 `picals_crawler` 的 `pub` API 与 `tests/support/*`。

## 黑盒 CLI harness

- 新增 `assert_cmd` + `predicates`。
- CLI acceptance 统一采用：
  - `assert_cmd::Command::cargo_bin("picals-crawler")`
  - 临时 `HOME / XDG_CONFIG_HOME`
  - `wiremock`
  - 落盘副作用断言
- 已覆盖：
  - `--help`
  - 参数解析错误
  - `download user <userId> --dry-run`
  - `retry <manifest>`
  - 至少一条真实下载文件落盘路径

## 结构性验收

- `find tests -maxdepth 1 -name '*_test.rs'`：空结果
- `cargo test --test contracts --test app --test cli`：通过
- `rg 'use picals_crawler::test_support' tests`：仅命中 `tests/support/env.rs` 的受控 re-export
- `tests/live.rs`：默认 `#[ignore]`

## 最终验证与 review gate

- `cargo fmt --check` 通过
- `cargo check` 通过
- `cargo clippy --all-targets -- -D warnings` 通过
- `cargo test --all-targets` 通过
- 独立 `code-reviewer`：`APPROVE`
- 独立 `architect`：初审 `WATCH`，在收口 `test_support` 文档可见性与 observer attach helper 后复审为 `CLEAR`

## 结论

- 该仓库的测试组织不再是临时散放状态，而是已具备长期可维护的 Rust 原生双轨 + 四层测试合同。
- 后续新增测试应优先进入现有四个固定入口或 `src` 邻近单元测试，不应恢复顶层 `*_test.rs` 散文件模式。
