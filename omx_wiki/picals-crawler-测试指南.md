---
title: "Picals Crawler 测试指南"
tags: ["reference", "test", "ssot", "guide"]
created: 2026-06-23T10:00:00.000Z
updated: 2026-06-23T10:00:00.000Z
sources: []
links: ["picals-crawler-技术设计文档.md", "picals-crawler-测试架构升级实现记录-2026-06-23.md"]
category: reference
confidence: high
schemaVersion: 1
---

# Picals Crawler 测试指南

> 本文档是 `picals-crawler` 后续 Agent 编写、迁移、审查测试时的 **SSOT**。  
> 任何新增测试、移动测试、补 support、改测试边界之前，都应先阅读本文。

## 1. 测试架构总则

- 本仓库采用 **Rust 原生双轨 + 四层测试架构**。
- Layer 1 单元测试留在 `src` 邻近位置。
- `tests/` 只承接外部可见契约、跨模块主链路、真实 CLI 黑盒、以及 opt-in live tests。
- 不允许恢复顶层 `tests/*_test.rs` 散文件模式。
- 不允许为了把测试塞进 `tests/` 而暴露不必要的 public API。

## 2. 四层分工

### Layer 1：单元测试

- 放置位置：
  - `src/*.rs` 内联 `#[cfg(test)]`
  - 或目录模块下的 `tests.rs`
- 适用内容：
  - 私有实现
  - 配置合并
  - 错误分类
  - URL 语义解析
  - 局部状态机
  - 不需要真实进程边界的纯逻辑
- 规则：
  - 可以访问私有实现
  - 不应依赖 `tests/support/*`

### Layer 2：contracts

- 顶层入口：`tests/contracts.rs`
- 子模块目录：`tests/contracts/*`
- 当前示例：
  - `tests/contracts/selector.rs`
  - `tests/contracts/manifest.rs`
- 适用内容：
  - selector fixture contract
  - manifest roundtrip
  - 任何基于公开输入输出的稳定契约
- 硬约束：
  - **只允许依赖 `picals_crawler` 的 `pub` API 与 `tests/support/*`**
  - 任何依赖 `pub(crate)` / 私有实现的断言，必须留在 Layer 1

### Layer 3：app

- 顶层入口：`tests/app.rs`
- 子模块目录：`tests/app/*`
- 当前示例：
  - `tests/app/crawler.rs`
  - `tests/app/command_dispatch.rs`
- 适用内容：
  - `commands::dispatch(cli).await`
  - crawler 主链路
  - retry / replay
  - shared session identity
  - 断点续传与文件副作用
- 特征：
  - 允许 `wiremock`
  - 允许临时目录
  - 不启动真实二进制进程

### Layer 4：cli

- 顶层入口：`tests/cli.rs`
- 子模块目录：`tests/cli/*`
- 当前示例：
  - `tests/cli/help.rs`
  - `tests/cli/parse.rs`
  - `tests/cli/download_user.rs`
  - `tests/cli/retry.rs`
- 适用内容：
  - 真实二进制入口
  - help / usage / parse error
  - `download user --dry-run`
  - `retry <manifest>`
  - 真实落盘副作用
- 硬约束：
  - 黑盒 CLI 测试必须优先使用真实子进程，而不是 `commands::dispatch(cli).await`

### live

- 顶层入口：`tests/live.rs`
- 规则：
  - 默认 `#[ignore]`
  - 不进入默认验证
  - 只用于手动 live / 真网测试

## 3. 固定入口合同

`tests/` 顶层稳定入口只允许：

- `tests/contracts.rs`
- `tests/app.rs`
- `tests/cli.rs`
- `tests/live.rs`

新增测试主题时：

- 优先挂到现有入口的子模块
- 不新增新的顶层 integration crate
- 不新增新的 `*_test.rs`

## 4. support 边界

### `src/test_support.rs`

允许的最终公共面只有：

- `EnvVarGuard`
- `lock_env`
- `ConfigHomeGuard`
- `set_config_home`
- `SessionObserverGuard`
- `install_session_observer`

说明：

- 这是 **最小 shared support**
- 它不是通用外部 API，也不是任意测试 helper 的收纳箱
- `current_session_observer` 不属于这里

### `src/net/test_hook.rs`

- `current_session_observer` 已迁入此处
- 仅允许生产路径读取
- `attach_session_observer(...)` 是统一接入 observer seam 的唯一入口

### `tests/support/*`

当前职责：

- `fixtures.rs`：fixture 读取
- `env.rs`：Layer 2/3/4 环境隔离封装
- `cli.rs`：`assert_cmd` harness
- `mock_pixiv.rs`：mock 路径 / referer 辅助

规则：

- 只服务 Layer 2/3/4
- 不承接 crate 私有实现测试 helper
- 不应被生产代码消费

## 5. CLI 黑盒 harness 规范

Layer 4 CLI 测试的最小合同是：

- `assert_cmd::Command::cargo_bin("picals-crawler")`
- 临时 `HOME`
- 临时 `XDG_CONFIG_HOME`
- `wiremock`
- 显式落盘副作用断言

推荐复用：

- `tests/support/cli.rs::CliTestContext`

CLI 黑盒测试应优先覆盖：

- `--help`
- 参数解析错误
- `download user <userId> --dry-run`
- `retry <manifest>`
- 至少一条真实下载文件落盘

## 6. 选层决策规则

遇到新测试时，按下面规则放置：

- 需要访问私有实现：Layer 1
- 只验证 `pub` 输入输出契约：Layer 2
- 验证跨模块主链路，但不需要真实进程：Layer 3
- 验证真实二进制、退出码、stdout/stderr、副作用：Layer 4
- 需要真实 Pixiv 网络：`tests/live.rs`

如果一个测试同时像 app 又像 cli，优先问自己：

- 是否必须启动真实 `picals-crawler` 子进程？
  - 是：Layer 4
  - 否：Layer 3

## 7. 结构性禁止项

- 禁止把所有测试硬搬进 `tests/`
- 禁止让 Layer 2 依赖私有实现
- 禁止在 `tests/` 顶层继续新增 `*_test.rs`
- 禁止把 live 测试放进默认验证
- 禁止把 observer/internal seam 直接暴露给 contracts 层
- 禁止把 `src/test_support.rs` 继续扩张成通用垃圾桶

## 8. 编写新测试时的操作清单

1. 先确定测试应该属于哪一层
2. 检查是否已有对应入口和 support 可复用
3. 若是 Layer 2，确认只依赖 `pub` API
4. 若是 Layer 4，优先用 `CliTestContext`
5. 写完后至少跑对应层命令
6. 若改动测试边界或 support，同时跑结构性审计

## 9. 推荐验证命令

按层验证：

```bash
cargo test --test contracts
cargo test --test app
cargo test --test cli
```

结构审计：

```bash
find tests -maxdepth 1 -name '*_test.rs'
rg 'use picals_crawler::test_support' tests
```

全量质量门禁：

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

## 10. 与历史文档的关系

- 测试迁移过程与背景：[[Picals Crawler 测试架构升级实现记录 2026-06-23]]
- 当前整体技术架构：[[Picals Crawler 技术设计文档]]

若历史实现记录与本文冲突，以本文为后续编写测试时的 SSOT。
