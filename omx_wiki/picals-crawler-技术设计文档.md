---
title: "Picals Crawler 技术设计文档"
tags: ["technical", "design", "architecture", "rust", "cli"]
created: 2026-06-16T00:00:00.000Z
updated: 2026-06-22T15:47:42.000Z
sources: ["_notes/nea/technical-design.md"]
links: ["picals-crawler-产品设计文档.md", "picals-crawler-项目理解基线.md", "typescript-原项目实现观察.md"]
category: architecture
confidence: high
schemaVersion: 1
---

# Picals-Crawler 技术设计文档

> 版本：v0.4.0-draft
> 最后更新：2026-06-22
> 状态：net 层 SSOT、Pixiv 领域模块与失败回放闭环均已落地并通过验证

---

## 零、设计哲学

picals-crawler 的技术侧核心定位是：

> **现代 Rust + 轻量 + 优雅 DX**

具体来说：

- **现代**：依赖选择优先考虑 Rust 生态中设计最干净、维护最活跃、API 最符合人体工学的库。不守旧，不追新到不稳定。
- **轻量**：每个依赖都经过「真的需要吗？有没有更轻的替代？」的拷问。不需要的 feature 不开，不需要的依赖不加。单二进制、零运行时依赖。
- **优雅 DX**：代码即文档。clap derive 让命令结构一目了然。eyre 让错误信息对用户友好。inquire 让交互流程赏心悦目。开发者打开项目就能理解架构。

这三条原则贯穿下面每一个选型决策。

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

```
picals-crawler/
├── Cargo.toml
├── Cargo.lock
├── src/
│   ├── main.rs                  # 入口，解析 CLI，路由到各命令
│   ├── cli/
│   │   ├── mod.rs               # clap 命令定义
│   │   ├── download.rs          # download 子命令及参数
│   │   └── config.rs            # config 子命令
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── setup.rs             # picals-crawler setup
│   │   ├── download_direct.rs   # download <pixiv-url> 路由
│   │   ├── download_user.rs     # download user 逻辑
│   │   ├── download_keyword.rs  # download keyword 逻辑
│   │   ├── download_ranking.rs  # download ranking 逻辑
│   │   ├── download_illust.rs   # download illust 逻辑
│   │   ├── download_bookmark.rs # download bookmark 逻辑
│   │   ├── retry_cmd.rs         # retry manifest 回放命令
│   │   └── config_cmd.rs        # config show/set 逻辑
│   ├── auth/
│   │   ├── mod.rs
│   │   └── credential.rs        # 凭据读取、写入、验证
│   ├── crawler/
│   │   ├── mod.rs               # CrawlContext 与 crawler 模块汇总
│   │   ├── user.rs              # UserCrawler
│   │   ├── keyword.rs           # KeywordCrawler
│   │   ├── ranking.rs           # RankingCrawler
│   │   ├── illust.rs            # IllustCrawler
│   │   └── bookmark.rs          # BookmarkCrawler
│   ├── downloader/
│   │   ├── mod.rs               # 下载管理器
│   │   └── image.rs             # 结构化下载项与单图辅助
│   ├── pixiv/
│   │   ├── mod.rs               # Pixiv 领域模块汇总
│   │   ├── selector.rs          # Pixiv Ajax / HTML 响应解析
│   │   └── url.rs               # Pixiv URL 语义解析
│   ├── config.rs                # 全局配置管理（serde + toml）
│   ├── error.rs                 # 统一错误类型
│   ├── output.rs                # 按模式解析输出路径 grammar
│   ├── failure.rs               # 失败记录 / manifest / replay 元数据
│   ├── replay.rs                # manifest 回放执行器
│   ├── test_support.rs          # 测试期环境隔离工具
│   ├── net/
│   │   ├── mod.rs               # net façade 与受控 re-export
│   │   ├── catalog.rs           # Pixiv 请求目录 / URL / Referer / host 分类
│   │   ├── client.rs            # metadata/image reqwest client 分离
│   │   ├── event.rs             # 请求与传输事件
│   │   ├── policy.rs            # retry / cooldown / backoff 策略
│   │   ├── session.rs           # PixivNetSession 主执行器
│   │   ├── state.rs             # host 级共享 cooldown 状态
│   │   └── transfer.rs          # 图片流式写 .part 与 rename
│   └── utils/
│       ├── mod.rs
│       ├── progress.rs          # 进度条封装
│       └── retry.rs             # 通用非网络重试辅助
├── tests/
│   ├── integration/
│   │   ├── common.rs            # 测试辅助函数
│   │   ├── selector_test.rs     # selector 解析测试
│   │   └── crawler_test.rs      # 爬虫流程测试（mock）
│   └── fixtures/
│       └── *.json               # 模拟 API 响应数据
├── _notes/
│   └── nea/
│       ├── product-design.md    # 产品设计文档
│       └── technical-design.md  # 本文件
├── .github/
│   └── workflows/
│       └── release.yml          # 多平台构建 + Release 发布
└── README.md                    # 待撰写
```

---

## 三、模块设计

### 3.1 模块职责

```
┌─────────────────────────────────────────────────────────┐
│                      main.rs                            │
│  解析 CLI → 加载配置 → 路由到对应 command               │
└──────────────┬──────────────────────────────────────────┘
               │
    ┌──────────┼──────────┬──────────┬──────────┐
    ▼          ▼          ▼          ▼          ▼
  setup    user       keyword    ranking    illust   bookmark
    │          │          │          │          │          │
    │          └──────────┴──────────┴──────────┴──────────┘
    │                         │
    ▼                         ▼
  auth::                 commands::
  credential             create_shared_session
                              │
                              ▼
                        net::PixivNetSession
                              │
               ┌──────────────┼──────────────┐
               ▼              ▼              ▼
          pixiv::url    pixiv::selector   crawler::
                                            shared
                                               │
                                               ▼
                                          downloader::
                                          image / progress
```

### 3.2 Crawler 组织方式

当前实现没有引入统一 `Crawler trait`。项目采用更直接的组织方式：

- `crawler/mod.rs` 提供 `CrawlContext`
- `user / keyword / ranking / illust / bookmark` 各自维护独立 `run()` 主流程
- 共用逻辑下沉到 `crawler/shared.rs`，例如：
  - 作品 ID → 图片 URL 收集
  - `tags.json` 导出
  - ID 排序
  - 下载汇总

这样可以复用主链路能力，同时避免为当前 CLI 主链路引入过度抽象层。

### 3.3 认证模块

```rust
pub struct Credential {
    pub phpsessid: String,
    pub user_id: Option<String>,
}

impl Credential {
    /// 从 ~/.config/picals-crawler/credentials 读取
    pub fn load() -> Result<Option<Self>>;

    /// 写入凭据文件（权限 600）
    pub fn save(&self) -> Result<()>;

    /// 是否已配置
    pub fn exists() -> bool;
}
```

认证模块负责凭据的读写与校验，不负责浏览器侧获取；`setup` 会先采集 `PHPSESSID`，再优先从登录首页的响应头或 HTML 中自动解析当前账号 `userId`，失败时允许手动输入兜底。

### 3.4 配置模块

```rust
#[derive(Deserialize, Serialize)]
pub struct Config {
    pub download: DownloadConfig,
    pub proxy: ProxyConfig,
}

#[derive(Deserialize, Serialize)]
pub struct DownloadConfig {
    pub roots: DownloadRootsConfig,
    pub count: usize,
    pub sort: SortOrder,
    pub r18: bool,
    pub ai: bool,
    pub concurrent: usize,
    pub timeout: u64,
    pub retry: usize,
    pub with_tags: bool,
}

#[derive(Deserialize, Serialize)]
pub struct DownloadRootsConfig {
    pub illust: String,
    pub user: String,
    pub bookmark: String,
    pub keyword: String,
    pub ranking: String,
}

#[derive(Deserialize, Serialize)]
pub struct ProxyConfig {
    pub url: String,
}
```

配置加载优先级：CLI 参数 → 环境变量 → config.toml → 默认值。

当前实现保留 `download.directory` 作为历史兼容读键，但新的写入与主输出统一使用 `download.roots.*`。

### 3.5 错误处理

```rust
// 库层：使用 thiserror 定义结构化错误
#[derive(Error, Debug)]
pub enum CrawlerError {
    #[error("认证失败: {0}")]
    Auth(String),

    #[error("网络请求失败: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API 响应解析失败: {0}")]
    Parse(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("未找到用户: {0}")]
    UserNotFound(String),

    #[error("未找到作品: {0}")]
    IllustNotFound(String),

    #[error("下载中断: {0}")]
    DownloadInterrupted(String),

    #[error("HTTP 请求失败: 状态码 {status}，{context}")]
    HttpStatus { status: u16, context: String },
}

// 应用层：使用 eyre 简化错误传播
pub type Result<T> = eyre::Result<T>;
```

---

## 四、核心数据流

### 4.1 完整下载流程（以 `download user` 为例）

```
用户执行: picals-crawler download user 12345678

1. CLI 解析 (clap)
   └→ DownloadUserArgs { id: "12345678", to: None, ... }

2. 加载配置
   ├→ 读取 ~/.config/picals-crawler/config.toml
   ├→ 合并 CLI 参数
   └→ 合并环境变量

3. 加载凭据
   ├→ 读取 ~/.config/picals-crawler/credentials
   └→ 若不存在 → 提示运行 picals-crawler setup

4. 命令层创建共享网络会话
   └→ `Arc<PixivNetSession>`，在 probe / crawl / download / auto-replay 全链路复用同一实例

5. UserCrawler::run()
   │
   ├─ Phase 1: collect()
   │  ├→ `session.fetch_user_profile_all(user_id)`
   │  ├→ `pixiv::selector::select_user_illust_ids(...)`
   │  └→ 返回 Vec<illust_id>
   │
   ├─ Phase 2: collect_download_items(ids)
   │  ├→ for each id: `session.fetch_illust_pages(id)`
   │  ├→ `pixiv::selector::select_page_original_urls(...)`
   │  ├→ OutputLayout 解析 context / illust 目录
   │  └→ 返回 Vec<DownloadItem { illust_id, image_url, target_dir }>
   │
   └─ Phase 3: download(items)
      ├→ 断点续传：检查本地是否已有文件
      ├→ 并发下载（Semaphore 控制）
      ├→ 每张图：`session.download_original_image(...)`
      ├→ 流式写入 `.part` → 校验 → rename 成目标文件
      ├→ 更新进度条
      └→ 返回 DownloadResult { total, downloaded, skipped, failed, failure_records }

6. 输出结果
   ├→ 对 retryable 失败项自动 replay 一次，并复用同一 `Arc<PixivNetSession>`
   ├→ 若仍失败则写入 manifest
   ├→ 输出 `picals-crawler retry <manifest-path>`
   └→ 写入 tags.json（若启用）

7. 独立 retry
   └→ `picals-crawler retry <manifest-path>` 会走同一 net stack 创建新 `PixivNetSession`，但不复用旧实例
```

### 4.2 图片下载流程

```
download_image(url, target_path)
│
├─ 1. 检查文件是否已存在 → 存在则 skip
│
├─ 2. 构建请求
│  ├─ 设置 Referer header
│  ├─ 设置 User-Agent
│  └─ image client 默认不携带登录 cookie
│
├─ 3. 发送 GET 请求
│  ├─ 通过 `PixivNetSession::execute()` 进入统一请求主链路
│  ├─ metadata / image client 分治
│  ├─ host 级 cooldown
│  ├─ 指数退避 + jitter
│  └─ Retry-After / 429 收敛
│
├─ 4. 完整性校验
│  ├─ 流式消费 body
│  ├─ 对比 Content-Length 与实际写入大小
│  └─ 失败时删除 `.part`
│
└─ 5. 写入磁盘
   └─ `.part` 成功后原子 rename，返回写入字节数
```

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

---

## 六、测试策略

### 6.1 单元测试

- **Selector 测试**：对每个 selector 函数，用 `tests/fixtures/` 中的 mock JSON 数据验证解析逻辑
- **URL 解析测试**：验证 user URL 提取、illust ID 提取
- **配置合并测试**：验证 CLI 参数覆盖 config.toml 的优先级

### 6.2 集成测试

- Mock HTTP server（使用 `wiremock`）模拟 Pixiv API 响应
- 完整的 `UserCrawler::run()` 流程测试
- 断点续传测试（模拟已下载部分文件）
- `retry` manifest 回放测试
- 自动 replay / manifest 落盘测试
- 请求运行时 `429 / 503 / timeout / 401` 行为测试

### 6.3 测试隔离

- 当前实现统一通过 `src/test_support.rs` 管理 `HOME / XDG_CONFIG_HOME / PICALS_PIXIV_BASE_URL / PICALS_DOWNLOAD_*` 等全局环境变量隔离。
- integration tests 与库内单元测试共用同一套 `lock_env()` 与 `EnvVarGuard`。

### 6.4 测试覆盖率目标

- 核心模块（selector / auth / config）≥ 80%
- Crawler 模块 ≥ 60%

---

## 七、CI/CD 方案

### GitHub Actions Workflow: `release.yml`

触发条件：推送 `v*` 格式的 tag

流程：

```yaml
jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: macos-13
            target: x86_64-apple-darwin
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    steps:
      - checkout
      - setup Rust toolchain
      - cargo build --release --target ${{ matrix.target }}
      - 打包 (tar.gz for unix, zip for windows)
      - upload artifact

  release:
    needs: build
    steps:
      - download all artifacts
      - generate SHA256 checksums
      - create GitHub Release
      - upload all artifacts
```

首次手动构建后，后续 tag push 自动触发。

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
- **HTTPS 强制**：所有 Pixiv API 请求使用 HTTPS
- **TLS 验证**：默认启用 TLS 证书验证
- **代理安全**：SOCKS5 代理使用 `reqwest` 内置支持，不依赖外部工具

---

## 九、Cargo.toml 依赖

```toml
[package]
name = "picals-crawler"
version = "0.1.0"
edition = "2024"
description = "开箱即用的 Pixiv 图片下载 CLI 工具"
authors = ["nonhana"]
license = "MIT"

[dependencies]
tokio = { version = "1", features = [
  "rt-multi-thread",
  "macros",
  "sync",
  "time",
  "signal",
] }
reqwest = { version = "0.12", default-features = false, features = [
  "http2",
  "socks",
  "cookies",
  "rustls-tls",
  "charset",
  "json",
] }
clap = { version = "4", features = ["derive", "wrap_help"] }
inquire = "0.7"             # 交互式 UI（密码输入、选择、确认）
indicatif = "0.17"          # 多栏进度条
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
eyre = "0.6"                # 应用层错误处理（比 anyhow 更现代）
thiserror = "2"             # 库层结构化错误
log = "0.4"                 # 日志门面（轻量替代 tracing）
env_logger = "0.11"         # 基于 RUST_LOG 环境变量的日志输出
jiff = "0.1"                # 时间处理（比 chrono 更现代）
regex = "1"
dirs-next = "2"             # 跨平台配置目录
url = "2"                   # URL 解析与构造
futures = "0.3"             # 并发流处理

[dev-dependencies]
wiremock = "0.6"            # HTTP mock
tempfile = "3"              # 临时目录
tokio-test = "0.4"          # tokio 测试工具
```

### 依赖精简说明

与旧项目（TypeScript / npm）相比：

- **无 HTML 解析器**：Pixiv Ajax API 直接返回 JSON 数据（含 tags），无需 cheerio/scraper
- **无 async-mutex**：tokio 的 `Mutex` + `Semaphore` 原生覆盖并发控制
- **无 chalk**：inquire 和 indicatif 自带终端着色
- **无 dayjs / moment**：jiff 覆盖所有时间处理

与初始选型草案相比：

| 移除 | 替代 | 原因 |
|---|---|---|
| `tracing` / `tracing-subscriber` | `log` + `env_logger` | CLI 工具无需结构化日志和 span |
| `scraper` | 无 | tags 从 Ajax API JSON 获取 |
| `anyhow` | `eyre` | 更现代的 backtrace + 错误上下文 |
| `chrono` | `jiff` | 更安全、更现代的 API 设计 |
| `dialoguer` | `inquire` | 更现代的交互式 UI 体验 |
| `dirs` | `dirs-next` | 维护更活跃 |
| `tokio` full features | 按需 features | 减少编译时间，缩小二进制大小 |

---

## 十、分阶段交付计划

### Phase 1 — MVP（预计 2-4 周）

**目标**：跑通 `download user` 完整链路

- [x] 项目结构搭建（Cargo.toml、目录结构）
- [x] `cli/` 模块：clap 命令定义，`download user` 子命令
- [x] `auth/` 模块：凭据读写
- [x] `config.rs`：配置加载
- [x] `crawler/user.rs`：UserCrawler 实现
- [x] `pixiv/`：Pixiv URL 语义与响应解析
- [x] `downloader/`：图片下载 + 重试 + 完整性校验
- [x] `utils/progress.rs`：基础进度条
- [x] `commands/setup.rs`：交互式认证引导
- [x] `error.rs`：错误类型定义
- [x] `tests/`：selector 单元测试
- [x] GitHub Actions 多平台构建

> 现状说明（2026-06-18）：Phase 1 主链路已完成。当前仍未完成的用户体验项，如速度/ETA、`tags.json`、更完整的断点续传语义，留在 Phase 2 处理。

### Phase 2 — 功能完善（预计 2-3 周）

- [x] `download keyword`：KeywordCrawler
- [x] `download ranking`：RankingCrawler
- [x] `download illust`：IllustCrawler
- [x] `download bookmark`：基于 setup 保存的 `userId` 完成收藏下载闭环
- [x] `config show/set` 命令（已补齐 `download.sort` 约束、迁移错误与测试）
- [x] 断点续传（本轮收敛为 `.part` 清理/覆盖与成品文件跳过）
- [x] tags.json 保存
- [x] 精致进度条（速度、ETA 统计 seam）

> 现状说明（2026-06-22）：当前实现已继续演进为 `src/net/{catalog,client,event,policy,session,state,transfer}.rs` + `src/pixiv/{selector,url}.rs` 结构。命令层一次下载只创建一个共享 `Arc<PixivNetSession>`，`probe -> crawl -> download -> auto-replay` 复用同一实例；独立 `retry` 复用同一 net stack 但新建实例。旧 `collector` 概念已删除，图片下载已改为流式 `.part` 写盘，并通过 `cargo fmt --check`、`cargo check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all-targets` 验证。

### Phase 3 — 体验打磨（预计 1-2 周）

- [ ] 错误信息中文化
- [ ] Homebrew formula 提交
- [ ] Scoop bucket 创建
- [ ] 完整中文 README

### Phase 4 — 进阶（后续）

- [ ] Ugoira → GIF/MP4 转换
- [ ] 文件名模板自定义
- [ ] 多账号支持
- [ ] crates.io 发布
