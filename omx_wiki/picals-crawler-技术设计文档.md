---
title: "Picals Crawler 技术设计文档"
tags: ["technical", "architecture", "rust"]
created: 2026-06-16T00:00:00.000Z
updated: 2026-06-25T00:00:00.000Z
sources: ["_notes/nea/technical-design.md"]
links: ["picals-crawler-产品设计文档.md", "picals-crawler-项目理解基线.md", "typescript-原项目实现观察.md", "picals-crawler-测试架构升级实现记录-2026-06-23.md", "picals-crawler-技术设计附录-实现细节.md", "picals-crawler-ux-优化执行记录-2026-06-21.md", "picals-crawler-测试指南.md", "picals-crawler-ugoira-gif-下载调研报告-2026-06-24.md", "picals-crawler-ugoira-gif-实现记录-2026-06-24.md", "picals-crawler-返工收尾实现记录-2026-06-25.md", "hana-pixiv-downloader-标准发布流程.md"]
category: architecture
confidence: high
schemaVersion: 1
---

# Picals-Crawler 技术设计文档

> 版本：v0.7.0-draft
> 最后更新：2026-06-25
> 状态：net 层 SSOT、ugoira GIF、四层测试架构，以及 2026-06-25 Code Review 返工收敛均已落地并通过验证

---

## 零、设计哲学

- 核心定位：**现代 Rust + 轻量 + 优雅 DX**
- 现代：优先选择稳定、人体工学好的主流 Rust 生态方案
- 轻量：避免无必要依赖、无必要 feature、无必要抽象
- 优雅 DX：代码结构、CLI 文案、错误输出都要可直接阅读

---

## 一、技术栈选型

| 技术 | 选型 | 理由 |
|---|---|---|
| 语言 | **Rust** (stable, edition 2024) | 单二进制分发、零运行时依赖、内存安全、高性能 |
| 异步运行时 | **tokio**（按需 features） | Rust 生态事实标准，不启用 `full`，只开启实际需要的 feature |
| HTTP 客户端 | **reqwest**（精选 features） | HTTP/2、SOCKS5 代理、连接池、cookie 管理；默认关闭 native-tls，使用 rustls |
| TLS 后端 | **rustls** | 纯 Rust 实现，不依赖系统 OpenSSL，跨平台编译零配置 |
| CLI 框架 | **clap** (derive mode) | 声明式定义命令、自动生成 `--help`、编译期参数校验 |
| 交互式 UI | **inquire** | 比 dialoguer 更现代：内置密码隐藏、`↑↓` 选择、输入验证、漂亮的默认样式 |
| 进度条 | **indicatif** | 多栏进度条、速度、ETA、模板化样式 |
| 序列化 | **serde** + **serde_json** + **toml** | 配置解析、API 响应解析、凭据读写 |
| 错误处理 | **eyre**（应用层）+ **thiserror**（库层） | eyre 比 anyhow 更现代：更好的 backtrace 支持、更丰富的错误上下文 |
| 日志 | **log** + **env_logger** | CLI 工具不需要 tracing 的结构化日志和 span；log + env_logger 更轻量，`RUST_LOG` 环境变量控制日志级别 |
| 时间 | **jiff** | BurntSushi 出品，比 chrono 更现代、更正确、更符合人体工学。支持时区、格式化、时间戳转换 |
| 正则 | **regex** | 标准选择，URL 解析、文件名提取 |
| 配置目录 | **dirs-next** | 比 dirs 维护更活跃，跨平台获取 `$XDG_CONFIG_HOME` / `%APPDATA%` |
| 动图编码 | **gif + image + zip**（收紧 features） | 纯 Rust ugoira zip 解包与 GIF 编码，不引入 ffmpeg 或外部运行时 |
| 测试 | **cargo test**（内置） | 单元测试 + 集成测试，无需额外框架 |
| CI/CD | **GitHub Actions** | 多平台构建、自动发布 Release |

### 选型说明

**为什么是 reqwest 而不是直接裸写 hyper？**

reqwest 提供开箱即用的 cookie 存储、自动重定向、SOCKS5 代理、连接池、HTTP/2。这些功能如果用 hyper 裸写，需要几千行胶水代码。reqwest 是 Rust 生态中下载量最高的 HTTP 库，质量有保障。

**为什么不需要 HTML 解析器（scraper）？**

旧项目用 cheerio 解析 Pixiv `/artworks/{id}` 页面的 `<meta>` 标签来获取 tags。但 Pixiv 的 Ajax API（`/ajax/illust/{id}`）在 JSON 响应中直接包含了完整的 tags 数据，无需解析 HTML。因此移除 scraper 依赖，进一步瘦身。

**为什么是 eyre 而不是 anyhow？**

eyre 是 anyhow 的精神继承者。在 anyhow 的基础上提供了更好的 backtrace 捕获、`WrapErr` trait（给错误附加上下文时更顺手）、以及更漂亮的错误输出。API 几乎完全兼容 anyhow，迁移成本接近零。

**为什么是 jiff 而不是 chrono？**

jiff 是 BurntSushi（ripgrep 作者）的新作，解决了 chrono 长期存在的一些设计问题：时区处理更安全、Duration 算术更直观、API 更一致。生态目前不如 chrono 广泛，但对于 picals-crawler 这种只需要格式化时间戳的场景完全够用。

**为什么是 clap (derive) 而不是手动解析？**

derive 模式下，命令结构即代码结构，编译器帮你检查参数冲突。自动生成 `--help` 输出，不需要额外维护。

---

## 二、项目目录结构

- `src/cli` / `src/commands`：命令定义与命令执行入口
- `src/auth` / `src/config` / `src/error`：认证、配置、错误模型
- `src/crawler` / `src/downloader` / `src/output`：下载主链路与落盘布局
- `src/net` / `src/pixiv` / `src/replay` / `src/failure`：请求栈、Pixiv 语义、失败回放
- `src/test_support.rs`：最小 shared test support
- `tests/{contracts,app,cli,live}.rs`：四层测试顶层入口

详细目录树与测试子目录说明已移动到附录：[[Picals Crawler 技术设计附录：实现细节]]

---

## 三、模块设计

当前模块设计的稳定结论是：

- `main.rs` 只做 CLI 解析与命令路由，不下沉业务。
- `main.rs` 在命令分发前先按 `--verbose` 初始化日志；默认级别为 `info`，若已设置 `RUST_LOG` 则以环境变量为准。
- `commands::*` 负责把 CLI 参数转换为主链路行为，并统一走共享 session / replay 收尾。
- `commands::download_common::confirm_bulk_plan()` 负责统一批量命令的探测摘要、计划数量回写、配置表打印与 dry-run 早退。
- `PixivNetSession` 是唯一网络 façade；Pixiv URL、referer、client、policy 不再分散。
- `crawler::*` 不引入过度统一 trait，而是按模式保留独立 `run()` 主流程，共用逻辑收口到 `shared`。
- 五类 crawler 统一只保留 `new(...) -> Self` 构造器；不再保留“生产用 Result 构造器 + 测试专用副本”双轨。
- `crawler::shared::plan_tags_and_download()` 统一承接作品规划、`tags.json` 导出、下载执行与失败计数合并，避免各 crawler 手抄尾部逻辑。
- 下载规划已经从 `pages -> 图片 URL 列表` 升级为 `detail -> ArtworkDownloadPlan`，静态图与 ugoira 共用作品级规划入口。
- 配置模型已经冻结为 `download.roots.* + 通用下载参数 + proxy.url`，`download.directory` 仅保留历史兼容读键。
- `SortOrder` 已收敛为 `date_desc / date_asc` 两个稳定值，`popular_desc` 被明确移除，不再做迁移兼容。
- crate 默认可见性策略已收紧为“默认 `pub(crate)`，只为 `main.rs` 和 `tests/` 保留 `pub`”，并用 `#![warn(unreachable_pub)]` 作为长期护栏。
- 错误模型继续使用 `thiserror + eyre` 双层组合，不改为更重的 tracing / anyhow 体系。

详细的模块职责图、配置结构与错误模型示例已移动到附录：[[Picals Crawler 技术设计附录：实现细节]]

---

## 四、核心数据流

- 当前主链路可概括为：
  1. CLI 解析模式参数，并按 `--verbose` / `RUST_LOG` 初始化日志
  2. 配置、环境变量、凭据合并成 resolved options
  3. 批量命令先执行 probe，并统一走 `confirm_bulk_plan()` 完成计划确认与 dry-run 早退
  4. 命令层创建共享 `PixivNetSession`
  5. crawler 拉取作品 ID，并先读取作品 detail 生成 `ArtworkDownloadPlan`
  6. 静态图作品走 `pages -> original image urls`；ugoira 作品走 `ugoira_meta.originalSrc zip -> GIF`
  7. downloader 流式写 `.part` 或 `.picals-workspace` 中间态，成功后原子落最终产物，失败形成 `FailureRecord`
  8. `tags.json` 导出复用规划阶段的 detail cache，不重复请求同一作品 detail
  9. 收尾阶段执行 auto-replay，并在仍失败时写 manifest 供 `retry` 回放

详细的 `download user` 时序与图片下载细节已移动到附录：[[Picals Crawler 技术设计附录：实现细节]]

---

## 五、Pixiv API 端点

本项目使用的 Pixiv Ajax API 端点：

| 用途 | 端点 | 需要认证 |
|---|---|---|
| 获取画师全部作品 | `GET /ajax/user/{id}/profile/all` | 部分需要 |
| 获取作品页面 | `GET /ajax/illust/{id}/pages` | 部分需要 |
| 关键词搜索 | `GET /ajax/search/artworks/{keyword}` | 部分需要 |
| 排行榜 | `GET /ranking.php?mode={mode}&format=json` | 部分需要 |
| 收藏列表 | `GET /ajax/user/{id}/illusts/bookmarks` | 需要 |
| 作品详情（含 tags） | `GET /ajax/illust/{id}` | 部分需要 |
| 动图元数据 | `GET /ajax/illust/{id}/ugoira_meta` | 部分需要 |

---

## 六、测试策略

### 6.1 四层测试架构

- Layer 1：单元测试留在 `src` 邻近位置，优先覆盖私有实现、配置合并、URL 解析、错误分类、局部状态机。
- Layer 2：`tests/contracts.rs` 及其子模块，只允许依赖 `picals_crawler` 的 `pub` API 与 `tests/support/*`。
- Layer 3：`tests/app.rs` 及其子模块，承接 `commands::dispatch(cli).await`、crawler 主链路、retry / replay / shared session 语义。
- Layer 4：`tests/cli.rs` 及其子模块，承接真实二进制黑盒测试。
- live tests：`tests/live.rs`，默认 `#[ignore]`，不进入默认验证。

### 6.2 Contracts / App / CLI 策略

- **contracts**：selector fixture contract、manifest roundtrip、ugoira meta/zip fixture contract 等公开契约。
- **app integration**：Mock HTTP server（`wiremock`）+ 临时目录，覆盖完整 `UserCrawler::run()`、ugoira mixed batch、detail cache 请求计数、`retry` manifest 回放、auto-replay / session identity 与副作用。
- **cli acceptance**：`assert_cmd::Command::cargo_bin("picals-crawler")` + 临时 `HOME / XDG_CONFIG_HOME` + `wiremock`，覆盖 help、参数错误、`download user --dry-run`、ugoira `--dry-run` 非泄漏、`retry <manifest>` 与真实 GIF 落盘断言。
- **cli acceptance**：额外覆盖 `--verbose` 合同，保证开启时能看到 `request.attempt` 级别调试日志，关闭时不会泄漏该类 debug 输出。

### 6.3 Support 边界

- `src/test_support.rs` 只保留最小 shared support：`EnvVarGuard`、`lock_env`、`ConfigHomeGuard`、`set_config_home`、`SessionObserverGuard`、`install_session_observer`。
- `current_session_observer` 不再暴露给测试层，已迁入 `src/net/test_hook.rs`，仅供生产路径读取。
- `tests/support/` 只服务 Layer 2/3/4，不再承担 crate 私有实现测试 helper。

### 6.4 测试覆盖率目标

- 核心模块（selector / auth / config）≥ 80%
- Crawler 模块 ≥ 60%
- 本轮测试架构落地细节见 [[Picals Crawler 测试架构升级实现记录 2026-06-23]]

---

## 七、CI/CD 方案

版本升级、发布门禁、commit、tag、推送与远端产物验收以 [[Hana Pixiv Downloader 标准发布流程]] 为准。

### GitHub Actions Workflow: `release.yml`

- 触发条件：推送 `v*` 格式的 tag
- 当前目标矩阵：macOS `aarch64/x86_64`、Linux `x86_64`、Windows `x86_64`
- 基本流程：checkout → toolchain → `cargo build --release --target ...` → 打包 artifact → 生成 checksum → 创建 GitHub Release

### 发布检查清单

- [x] `cargo test` 全部通过
- [x] `cargo clippy` 无 warning
- [x] `cargo fmt --check` 通过
- [ ] 手动测试 macOS 二进制
- [ ] 手动测试 Windows 二进制
- [ ] tag push → CI 构建成功
- [ ] GitHub Release 发布
- [ ] 更新 Homebrew formula（后续）

---

## 八、安全考虑

- **凭据存储**：`~/.config/picals-crawler/credentials` 权限 600（仅 owner 可读写）
- **日志安全**：凭据不会自动进入日志；但 setup/config 的明文凭据可见性是显式产品合同，因此通过用户提示与文件权限控制来约束暴露面
- **调试日志合同**：`--verbose` 只把默认日志级别抬到 `debug`，不会覆盖显式 `RUST_LOG`，也不应打印认证凭据
- **HTTPS 强制**：所有 Pixiv API 请求使用 HTTPS
- **TLS 验证**：默认启用 TLS 证书验证
- **代理安全**：SOCKS5 代理使用 `reqwest` 内置支持，不依赖外部工具

---

## 九、Cargo.toml 依赖

- 当前依赖决策已经稳定：
  - 运行时：`tokio`
  - 网络：`reqwest + rustls`
  - 动图：`gif + image + zip`
  - CLI / 交互：`clap + inquire + indicatif`
  - 序列化与配置：`serde + serde_json + toml`
  - 错误：`eyre + thiserror`
  - dev：`wiremock + tempfile + tokio-test + assert_cmd + predicates`

完整依赖片段与“为什么不用 tracing / scraper / chrono / anyhow”的对照说明已移动到附录：[[Picals Crawler 技术设计附录：实现细节]]

---

## 十、分阶段交付计划

- 分阶段结论已经很明确：
  - Phase 1：已完成
  - Phase 2：已完成
  - 测试架构升级：已完成
  - ugoira GIF 正式集成：已完成
  - Code Review 返工收尾：已完成（死代码清理、构造器收敛、共享编排、`--verbose`、可见性最小化）
  - 当前下一阶段重点：README、安装分发、发布准备、体验打磨，以及后续是否需要进一步优化 detail 请求成本

更细的历史阶段清单与里程碑摘要已移动到附录：[[Picals Crawler 技术设计附录：实现细节]]
