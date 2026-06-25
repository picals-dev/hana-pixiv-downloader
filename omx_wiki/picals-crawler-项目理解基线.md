---
title: "Picals Crawler 项目理解基线"
tags: ["project", "baseline", "architecture", "rust"]
created: 2026-06-17T03:17:25.240Z
updated: 2026-06-25T00:00:00.000Z
sources: []
links: ["typescript-原项目实现观察.md", "picals-crawler-测试架构升级实现记录-2026-06-23.md", "picals-crawler-ux-优化执行记录-2026-06-21.md", "picals-crawler-下载预览与-ranking-调研附录-实验与竞品.md", "picals-crawler-ugoira-gif-实现记录-2026-06-24.md", "picals-crawler-返工收尾实现记录-2026-06-25.md"]
category: architecture
confidence: medium
schemaVersion: 1
---

# Picals Crawler 项目理解基线

## 产品定位

- 面向最终用户的 Pixiv 图片下载 CLI，而不是库、浏览器扩展或 Web 应用。
- 核心承诺是开箱即用：首次通过 setup 完成 `PHPSESSID + userId` 认证元数据初始化、五类下载根目录与通用下载参数配置，之后用户用一行命令或直接粘贴 Pixiv URL 执行下载。

## 设计目标

- 产品设计把 `setup` 与 `download user` 定义为 P0。`keyword`、`ranking`、`illust`、`bookmark`、`config` 属于后续扩展。
- 用户体验重点是中文化、低认知成本、进度可视化、断点续传、部分失败不拖垮整体。

## Rust 技术方向

- 技术设计已经明确 Rust 2024 + tokio + reqwest(rustls) + clap derive + inquire + indicatif + serde/toml + eyre/thiserror。
- 目标架构现已收敛为命令层、认证层、crawler 层、net 层、pixiv 领域层、downloader 层、config/error/utils 分层，不做过度抽象。
- 当前 crate 边界已进一步收紧为“默认 `pub(crate)`，仅对 `main.rs` 与 `tests/` 暴露必需项”。

## 与 TS 原项目的关键差异

- TS 原项目是可嵌入的下载库，入口是 `downloadUser` / `downloadKeyword` / `downloadBookmark` 三个函数，依赖全局单例配置对象。
- 新项目要做的是面向终端用户的单二进制 CLI，配置来源与交互模型需要重做，而不是直接一比一翻译 TypeScript API。

## 当前仓库状态

- 早期主链路和 Phase 2 能力已经完成：
  - `download <pixiv_url>` 已进入主链路，可自动分发到 user / illust / keyword
  - `download user / illust / keyword / ranking / bookmark` 均已进入主链路
  - `config show/set`、`tags.json`、`.part` 恢复语义、速度与 ETA 统计 seam 已实现
- 本轮 UX 优化已经完成：
  - `setup` 改为多步向导，完整展示凭据、五类 mode roots、通用下载参数与代理配置
  - `config show` 统一展示凭据与普通配置；`config set` 支持 `auth.phpsessid / auth.user_id / download.roots.*`
  - 下载目录模型升级为五类模式 root，并统一通过共享布局解析器输出 `context/illustId/illustId_pn.ext`
  - 请求层已统一到 `PixivNetSession` + `src/net/{catalog,client,event,policy,session,state,transfer}.rs`
  - `probe -> crawl -> download -> auto-replay` 已证明复用同一 `Arc<PixivNetSession>`；独立 `retry` 走同一 net stack 但新建实例
  - `collector` 概念已移除，Pixiv 响应解析与 URL 语义已归入 `src/pixiv/{selector,url}.rs`
  - 失败项已升级为结构化 manifest，并支持 `picals-crawler retry <manifest-path>` 回放
  - 测试架构已升级为四层 SSOT：
    - Layer 1：单元测试继续留在 `src` 邻近位置
    - Layer 2：`tests/contracts.rs`
    - Layer 3：`tests/app.rs`
    - Layer 4：`tests/cli.rs`
    - opt-in live：`tests/live.rs`
  - `src/test_support.rs` 仅保留最小 shared support；`current_session_observer` 已迁入 `src/net/test_hook.rs`
- 2026-06-24 ugoira GIF 已完成：
  - 作品规划升级为 `ArtworkDownloadPlan`
  - mixed batch 可同时处理静态图与 ugoira
  - ugoira 当前产出标准 `.gif`
- 2026-06-25 Code Review 返工已完成：
  - 确认死代码已删除，`popular_desc` 已永久移除
  - crawler 构造器统一为 `new(...) -> Self`
  - 批量命令与 crawler 尾部共享编排已收口
  - `--verbose` 已作为唯一全局开关落地，并有黑盒回归测试
  - `#![warn(unreachable_pub)]` 已开启，公开面缩减到 `main/tests` 实际需要
- 当前仓库的主要工作重心已从“实现 UX 优化”切换到“整理文档、准备后续发布与体验打磨”。

## 当前阶段判断

- **Phase 1**：已完成。
- **Phase 2**：已完成。
- **本轮 UX 优化**：已完成，并通过 `cargo fmt --check / cargo check / cargo clippy --all-targets -- -D warnings / cargo test --all-targets`。
- **测试架构升级**：已完成，并已固定四层测试入口、黑盒 CLI harness 与 live test 门控。
- **ugoira GIF 集成**：已完成，并已固定 detail 规划、tags 复用、GIF 转码与回放合同。
- **Code Review 返工收尾**：已完成，并通过 `fmt / build / clippy -D warnings / test` 验证闭环。
- **后续起点**：
  - 可以开始 README、安装分发、发布准备与后续体验打磨。

## 当前主要偏差

- 需继续保持产品/技术文档中的“明文凭据可见性属于冻结产品合同”这一事实一致，避免后续按默认安全偏好回滚。
- 当前 `SortOrder` 已只保留 `date_desc / date_asc`；不要再恢复 `popular_desc` 或为其增加兼容分支。
- 当前 replay / manifest 结构已经稳定，但若后续要继续抽边界，应在不破坏现有 CLI 合同的前提下进行。
- 测试架构现在已有明确 SSOT；后续新增测试不应再恢复 `tests/*_test.rs` 顶层散文件模式。
- 当前每个作品都会先请求一次 detail 以区分静态图与 ugoira；若后续优化请求成本，需要先验证列表接口里的 `illustType` 可靠性。

## 关联页面

- 参考 [[typescript-原项目实现观察]]
- 测试架构迁移记录见 [[Picals Crawler 测试架构升级实现记录 2026-06-23]]
- 下载预览研究附录见 [[Picals Crawler 下载预览与 Ranking 调研附录：实验与竞品]]
- ugoira 集成记录见 [[Picals Crawler ugoira GIF 实现记录 2026-06-24]]
- 返工收尾记录见 [[Picals Crawler 返工收尾实现记录 2026-06-25]]
