---
title: "Picals Crawler 技术设计附录：实现细节"
tags: ["appendix", "implementation", "details"]
created: 2026-06-23T09:25:00.000Z
updated: 2026-06-23T09:25:00.000Z
sources: []
links: ["picals-crawler-技术设计文档.md", "picals-crawler-测试架构升级实现记录-2026-06-23.md", "picals-crawler-net-层-ssot-实现记录-2026-06-22.md"]
category: reference
confidence: high
schemaVersion: 1
---

# Picals Crawler 技术设计附录：实现细节

## 目录树摘要

- `src/cli`：clap 命令定义
- `src/commands`：命令执行与参数编排
- `src/auth`：凭据读写与校验
- `src/crawler`：五类下载模式主流程
- `src/downloader`：单图下载与批量汇总
- `src/pixiv`：selector / url 语义
- `src/net`：catalog、client、event、policy、session、state、test_hook、transfer
- `src/failure` / `src/replay`：失败 manifest 与回放
- `src/test_support.rs`：最小 shared test support
- `tests/{contracts,app,cli,live}.rs`：四层测试入口

## 模块职责补充

- `commands::download_common` 负责共享 session、probe、layout、manifest/replay 收尾。
- `PixivNetSession` 负责 metadata/image host 分治、retry、cooldown、observer 事件。
- `src/net/test_hook.rs` 只承接内部测试 seam，生产路径通过 `attach_session_observer(...)` 接入。
- `crawler::*` 不引入统一 trait，而是按模式保留清晰的显式主流程。

## 主链路时序

1. CLI 解析参数
2. 加载配置、环境变量、凭据
3. 创建共享 `PixivNetSession`
4. crawler 收集作品 ID / 图片 URL / tags
5. downloader 流式写 `.part`
6. 失败项形成 `FailureRecord`
7. 命令收尾执行 auto-replay，必要时写 manifest

## 依赖补充

- 运行时：`tokio`
- 网络：`reqwest + rustls`
- CLI/交互：`clap + inquire + indicatif`
- 配置与序列化：`serde + serde_json + toml`
- 错误：`eyre + thiserror`
- dev：`wiremock + tempfile + tokio-test + assert_cmd + predicates`

### 已明确不采用

- `tracing`：当前 CLI 不需要 span 级结构化日志
- `scraper`：tags 已从 Ajax JSON 获取
- `chrono`：统一用 `jiff`
- `anyhow`：统一用 `eyre`

## 阶段里程碑

- Phase 1：`download user` 主链路、setup、基础测试、CI 已完成
- Phase 2：`keyword / ranking / illust / bookmark`、tags.json、`.part` 恢复语义、net 层 SSOT 已完成
- 2026-06-23：四层测试架构与黑盒 CLI harness 已完成

## 关联页面

- 主文档：[[Picals Crawler 技术设计文档]]
- 测试迁移记录：[[Picals Crawler 测试架构升级实现记录 2026-06-23]]
- net 层实现记录：[[Picals Crawler net 层 SSOT 实现记录 2026-06-22]]
