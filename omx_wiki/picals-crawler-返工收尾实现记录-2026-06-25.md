---
title: "Picals Crawler 返工收尾实现记录 2026-06-25"
tags: ["session-log", "refactor", "cleanup", "verbose", "visibility", "review"]
created: 2026-06-25T00:00:00.000Z
updated: 2026-06-25T00:00:00.000Z
sources:
  - "_notes/claude/picals-crawler-返工交接-2026-06-25.md"
  - "src/cli/download.rs"
  - "src/cli/mod.rs"
  - "src/commands/download_common.rs"
  - "src/config.rs"
  - "src/crawler/shared.rs"
  - "src/error.rs"
  - "src/failure.rs"
  - "src/lib.rs"
  - "src/main.rs"
  - "src/net/session.rs"
  - "tests/app/command_dispatch.rs"
  - "tests/app/crawler.rs"
  - "tests/app/ugoira.rs"
  - "tests/cli/verbose.rs"
links:
  - "picals-crawler-产品设计文档.md"
  - "picals-crawler-技术设计文档.md"
  - "picals-crawler-项目理解基线.md"
  - "picals-crawler-ugoira-gif-实现记录-2026-06-24.md"
  - "picals-crawler-测试指南.md"
category: session-log
confidence: high
schemaVersion: 1
---

# Picals Crawler 返工收尾实现记录 2026-06-25

## 本轮完成项

- 按 Code Review 返工计划完成 6 个阶段的收尾：
  - 移除已确认死代码
  - 永久移除 `popular_desc`
  - 收敛 crawler 构造器
  - 收编重复编排
  - 落地 `--verbose`
  - 把 crate 公开面最小化到 `main/tests` 真正需要的范围
- 本轮改动净效果以“删除与收口”为主：
  - 大量 `pub` 下沉到 `pub(crate)`
  - 重复的 probe / dry-run / tags / failure merge 尾部逻辑归并到共享 helper
  - 无新增依赖

## 六个阶段的稳定结论

### 1. 死代码清理

- 删除 `CrawlerError` 中从不构造的分支与相关分类逻辑。
- 删除 `src/utils/retry.rs` 及其他已无调用的访问器、常量、辅助函数。
- `cargo build` 与 `clippy -D warnings` 现在可以真实暴露新的死代码，而不会被大面积 `pub` 掩盖。

### 2. `popular_desc` 永久移除

- `SortOrder` 现在只剩 `DateDesc / DateAsc`。
- CLI、配置解析、测试断言与用户可见文案都不再为 `popular_desc` 留兼容逻辑。
- 手改配置写入 `sort = "popular_desc"` 时，当前预期行为是直接得到无效枚举/排序值错误。

### 3. crawler 构造器收敛

- `UserCrawler / IllustCrawler / KeywordCrawler / RankingCrawler / BookmarkCrawler` 统一只保留 `new(...) -> Self`。
- 旧的“永不失败却返回 `Result`”以及“测试专用 `new_with_session`”双轨已经移除。
- app 测试改为直接调用正式构造器，生产与测试共享同一入口。

### 4. 共享编排收口

- `commands::download_common::confirm_bulk_plan()` 统一处理：
  - probe 摘要打印
  - 计划下载数解析
  - 配置表输出
  - dry-run 早退
- `crawler::shared::plan_tags_and_download()` 统一处理：
  - detail 规划
  - `tags.json` 导出
  - 下载执行
  - 失败计数与 `failure_records` 合并

### 5. `--verbose` 全局开关

- `main.rs` 现在先 `Cli::parse()`，再按 `cli.global.verbose` 初始化日志。
- 默认日志级别：
  - 普通运行 `info`
  - 带 `--verbose` 时 `debug`
  - 若显式设置 `RUST_LOG`，则以环境变量优先
- `src/net/session.rs` 新增 `request.attempt` 调试日志，作为黑盒可断言的 debug 面。
- `tests/cli/verbose.rs` 固定了两条用户可见合同：
  - 带 `--verbose` 时可以看到 `request.attempt`
  - 不带时该类 debug 输出被抑制

### 6. 可见性最小化

- `lib.rs` 开启 `#![warn(unreachable_pub)]`。
- crate 默认策略改为：
  - 默认 `pub(crate)`
  - 只有 `main.rs` 与三层测试真正 import 的符号维持 `pub`
- 这轮收紧同时清出了新暴露的死代码，并把“只为测试存在的访问器”收回到 `#[cfg(test)]`。

## 用户可见合同更新

- 当前唯一全局 CLI 开关是 `--verbose`。
- 排序只支持 `date_desc / date_asc`。
- ugoira GIF 能力与本轮返工互相兼容：
  - mixed batch 仍可同时处理静态图与 ugoira
  - tags 导出与下载规划继续共享 detail 缓存
- `net/` 仍是唯一网络 SSOT façade；本轮没有引入新的旁路 HTTP 客户端。

## 验证命令

- `cargo fmt --check`
- `cargo build`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`

## 后续提醒

- 若后续再新增公开 API，默认应先设为 `pub(crate)`，只在 `main.rs` 或 `tests/` 编译失败时再提升。
- 若后续想继续优化性能，当前最可疑的成本点是“每个作品先请求一次 detail 以区分静态图与 ugoira”；在未验证列表字段可靠性前，不应贸然删除这一步。
