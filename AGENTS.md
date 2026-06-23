# AGENTS.md

picals-crawler — 开箱即用的 Pixiv 图片下载 CLI 工具。Rust, edition 2024。

## 构建与测试

```bash
cargo build                          # debug 构建
cargo build --release                # 发布构建
cargo test                           # 运行全部测试（含 wiremock 集成测试）
cargo test -- --nocapture            # 查看测试中的 println 输出
cargo check                          # 仅类型检查，无产物
```

## 测试路由

- **凡是新增、迁移、重构、审查测试，或新增 `tests/support/*` / `src/test_support.rs` / `src/net/test_hook.rs` 相关测试 seam 时，必须先阅读**：[omx_wiki/picals-crawler-测试指南.md](/Users/nonhana/code_life/Picals/picals-crawler/omx_wiki/picals-crawler-测试指南.md)
- 该文档是当前仓库测试分层、support 边界、CLI harness、结构性禁止项与验证命令的 SSOT。
- 如果历史测试写法、旧 session-log、或个人习惯与该文档冲突，以测试指南为准。

## 技术栈

| 领域 | 选型 | 理由 |
| ---- | ---- | ---- |
| 异步运行时 | tokio（rt-multi-thread） | 图像下载需要高并发 |
| HTTP 客户端 | reqwest 0.12（rustls-tls, http2, socks, cookies, stream） | 原生 async + 文件流式写入 |
| 错误处理 | eyre + thiserror | `CrawlerError` enum 定义分类错误，`AppResult<T>` 作为全项目别名 |
| CLI | clap 4 derive | 子命令多，产物扁平 |
| 日期时间 | jiff（非 chrono） | 轻量，RFC2822 解析 |
| 配置 | toml + serde + env 变量 | 支持默认值 → 配置文件 → 环境变量 → CLI 参数逐层覆盖 |
| 进度显示 | indicatif | 并发下载进度条 |
| 交互式输入 | inquire | setup 交互式引导 |
| 测试 | wiremock + tempfile | HTTP mock 隔离，环境变量沙箱 |

## 编码约定

1. **不做过度实现**。只写当前需求明确要求的代码。不预留抽象层或泛型参数。
2. **中文优先**。CLI 输出、错误信息、`--help` 文本用中文。代码注释用中文。模块文档（`//!`）用中文。
3. **错误信息具体可操作**。返回值必须是 `AppResult<T>` 或 `Result<T, CrawlerError>`。错误消息指明原因 + 可行的下一步。
4. **新增依赖时，必须逐个论证**。选型向「现代 Rust + 轻量 + 优雅 DX」看齐。默认偏好：eyre（非 anyhow）、jiff（非 chrono）、inquire（非 dialoguer）、dirs-next（非 dirs）、log + env_logger（非 tracing）。
5. **不要碰的文件**：`.omx/`、`.codegraph/`、`omx_wiki/`、`.gitmodules`。它们不是项目源码。
6. **不要在 comments 里写 byline 或 attribution**。纯中文注释，没有 `// 作者: xxx` 或 `// NOTE:`
