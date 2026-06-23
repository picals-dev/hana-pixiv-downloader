---
title: "Picals Crawler 产品设计文档"
tags: ["product", "design", "requirements"]
created: 2026-06-16T00:00:00.000Z
updated: 2026-06-23T09:19:53.000Z
sources: ["_notes/nea/product-design.md"]
links: ["picals-crawler-技术设计文档.md", "picals-crawler-项目理解基线.md", "typescript-原项目实现观察.md", "picals-crawler-产品设计附录-交互与路线图.md", "picals-crawler-ux-优化执行记录-2026-06-21.md"]
category: product
confidence: high
schemaVersion: 1
---

# Picals-Crawler 产品设计文档

> 版本：v0.3.0-draft
> 最后更新：2026-06-21
> 状态：UX 优化主链已落地并通过验证

---

## 一、定位

**一句话**：Picals-Crawler 是一个开箱即用的 Pixiv 图片下载 CLI 工具，面向二次元爱好者，一行命令批量下载喜欢的插画。

**电梯 pitch**：

> 在 Pixiv 上发现喜欢的画师，想把他所有作品都存下来？打开终端，输入 `picals-crawler download user 12345678`，坐下喝杯茶，回来就全下好了。不需要会编程，不需要配环境，不需要手动改配置文件。

**不是**：npm 库、浏览器扩展、通用图片站下载器、Web 应用。

---

## 二、目标用户

**主要用户画像**：二次元爱好者

- 喜欢在 Pixiv 上浏览插画
- 有「收藏癖」——看到喜欢的画师想把所有作品下载到本地
- 不懂编程，但愿意用终端执行简单命令（类似 `brew install` 的难度）
- 主要是中文用户，但工具本身不排斥英文用户
- 使用 macOS / Windows / Linux 桌面系统

**明确不是目标用户**：

- 需要 API 库集成到自己项目中的开发者 → 他们可以用 pixiv-client (Rust) 或 pixivpy (Python)
- 完全不想碰终端的人 → 他们应该用浏览器扩展，如 Powerful Pixiv Downloader
- 需要下载 200+ 个不同网站图片的 datahoarder → 他们应该用 gallery-dl

---

## 三、核心功能

### 3.1 功能概览

| 命令 | 功能 | 优先级 |
|---|---|---|
| `picals-crawler setup` | 交互式认证引导 + 配置初始化 | P0 |
| `picals-crawler download user <id>` | 下载指定画师的全部作品 | P0 |
| `picals-crawler download keyword <query>` | 下载包含指定关键词的搜索结果 | P1 |
| `picals-crawler download ranking` | 下载排行榜作品 | P1 |
| `picals-crawler download illust <id>` | 下载单张作品的所有图片 | P1 |
| `picals-crawler download bookmark` | 下载自己收藏的作品 | P1 |
| `picals-crawler config show` | 查看当前配置 | P1 |
| `picals-crawler config set <key> <value>` | 修改当前配置项 | P2 |

### 3.2 功能详情

#### `picals-crawler setup`

首次使用时的引导流程。交互式：

1. 欢迎信息
2. 引导用户获取 PHPSESSID（带步骤说明，只需复制一个字段）
3. 使用当前 `PHPSESSID` 从已登录首页的响应头或 HTML 自动解析当前账号 `userId`，失败时允许手动输入兜底
4. 逐项展示并允许修改五类下载根目录：`illust / user / bookmark / keyword / ranking`
5. 逐项展示并允许修改通用下载参数：`count / sort / r18 / ai / concurrent / timeout / retry / with_tags`
6. 可选：设置默认代理 `proxy.url`
7. 认证元数据持久化至 `~/.config/picals-crawler/credentials`（权限 600）

**设计原则**：只做一次，用户永远不需要再想认证的事。

**当前合同**：

- setup 中 `PHPSESSID` 输入与最终确认摘要都保持明文，便于用户完整核对。
- setup 正文必须提示“会明文显示凭据，避免录屏或共享屏幕”。
- `config show` 默认统一展示凭据与普通配置，并显式提示输出包含敏感凭据。
- `config set` 必须支持 `auth.phpsessid` 与 `auth.user_id`。

#### `picals-crawler download user <id|url>`

```
picals-crawler download user 12345678
picals-crawler download user "https://www.pixiv.net/users/12345678"
picals-crawler download user 12345678 --to ~/wallpaper/miku/
```

- 支持数字 ID 和完整 URL 两种输入方式
- URL 模式：用户直接从浏览器复制粘贴，零认知成本
- 默认下载全部作品，按时间降序排列
- 当前实现使用 `user_root/{userId}/{illustId}/illustId_pn.ext`
- 下载过程中显示进度条、速度、ETA

#### `picals-crawler download keyword <query>`

```
picals-crawler download keyword "初音ミク"
picals-crawler download keyword "オリジナル 女の子" --count 100 --sort date_asc
```

- 支持多关键词（空格分隔）
- 选项：`--count` 数量、`--sort` 排序（date_desc / date_asc）、`--r18` 模式切换、`--no-ai`
- 默认：全部结果、按时间降序、安全模式
- 当前实现使用 `keyword_root/{规范化关键词}/{illustId}/illustId_pn.ext`

#### `picals-crawler download ranking`

```
picals-crawler download ranking
picals-crawler download ranking --mode weekly --count 100
picals-crawler download ranking --mode daily_r18
```

- 排行模式：daily / weekly / monthly / daily_r18 / weekly_r18 / male / female
- 默认：daily（今日插图榜）；若未指定 `--count`，当前实现按“下载全部抓到的结果”处理
- 当前实现仅支持 illust 下载，不包含 manga / ugoira 专项模式
- 当前实现使用 `ranking_root/{mode}/{illustId}/illustId_pn.ext`

#### `picals-crawler download illust <id>`

```
picals-crawler download illust 12345678
```

- 下载单张作品的所有图片（Pixiv 上多图作品用 `_p0`、`_p1` 等区分）
- 适用于只想下载特定一张图的场景
- 当前实现使用 `illust_root/{illustId}/illustId_pn.ext`

#### `picals-crawler download bookmark`

```
picals-crawler download bookmark
picals-crawler download bookmark --count 200
```

- 下载自己收藏的作品
- 依赖 setup 中保存的认证元数据：`PHPSESSID + userId`
- 当前实现使用 `bookmark_root/{当前账号userId}/{illustId}/illustId_pn.ext`

### 3.3 全局选项

| 选项 | 说明 |
|---|---|
| `--to <path>` | 覆盖下载目录 |
| `--proxy <url>` | 代理地址（如 `socks5://127.0.0.1:1080`），也支持 `HTTPS_PROXY` 环境变量 |
| `--dry-run` | 只列出将要下载的内容，不实际下载 |

**当前 `--to` 合同**：

- `--to` 覆盖的是当前模式对应的 root，而不是最终图片目录。
- 最终输出仍会追加该模式自己的 context / illust 目录层级。

### 3.4 下载配置

每个下载命令支持以下选项（有默认值，可在 `config.toml` 中预设）：

| 选项 | 默认值 | 说明 |
|---|---|---|
| `--count` | `0`（全部） | 下载数量 |
| `--sort` | `date_desc` | 排序：date_desc / date_asc |
| `--r18` | `false` | 是否包含 R-18 作品 |
| `--no-ai` | `false` | 是否排除 AI 作品 |
| `--concurrent` | `8` | 并发下载数 |
| `--with-tags` | `false` | 是否同时导出 tags.json |

### 3.5 配置可见性

- `config show` 当前统一输出：
  - `auth.phpsessid`
  - `auth.user_id`
  - `download.roots.*`
  - 通用下载参数
  - `proxy.url`
- `download.directory` 已冻结为历史兼容读键，不再作为主配置面展示或写入。

---

## 四、CLI 交互设计

- 当前交互设计的稳定结论是：
  - `setup` 保持多步向导，明文展示凭据并在过程中自动探测 `userId`
  - 日常下载体验强调“低认知成本 + 进度可视化 + 可重试补救”
  - 断点续传语义已收敛为“重新执行同一条命令即可”
  - 失败补救闭环固定为 `auto-replay + manifest + retry`

完整示例与交互稿已移动到附录：[[Picals Crawler 产品设计附录：交互与路线图]]

---

## 五、配置体系

### 5.1 配置文件

```
~/.config/picals-crawler/
├── credentials        # 认证凭据（权限 600）
└── config.toml        # 用户配置
```

### 5.2 config.toml 结构

```toml
[download.roots]
illust = "~/Pictures/Pixiv/illust"
user = "~/Pictures/Pixiv/user"
bookmark = "~/Pictures/Pixiv/bookmark"
keyword = "~/Pictures/Pixiv/keyword"
ranking = "~/Pictures/Pixiv/ranking"

[download]
count = 0              # 0 = 全部
sort = "date_desc"
r18 = false
ai = true
concurrent = 8
timeout = 30
retry = 3
with_tags = false

[proxy]
url = ""               # socks5://127.0.0.1:1080
```

### 5.3 优先级

CLI 参数 > 环境变量 > config.toml > 默认值

---

## 六、安装方式

| 平台 | 方式 |
|---|---|
| macOS | 规划中：Homebrew formula |
| Windows | 规划中：Scoop bucket |
| Linux | `cargo install picals-crawler` 或 `.deb`/`AppImage` |
| 通用 | 从 GitHub Releases 下载预编译二进制 |

安装后即可使用，无需安装任何运行时依赖。

---

## 七、竞品对比

| 维度 | gallery-dl | PixivUtil2 | **picals-crawler** |
|---|---|---|---|
| 定位 | 通用下载器，200+ 站点 | 功能最全的 Pixiv 专用下载器 | 极简 Pixiv 下载器，面向普通用户 |
| 安装 | brew/scoop/pip (需 Python) | pip (需 Python + FFmpeg) | 当前以 cargo / Release 二进制为主（单二进制，无依赖） |
| 交互模型 | URL 驱动 | TUI 数字菜单 | 语义化命令 + URL 双模式 |
| 认证 | OAuth 或手动写 cookie | 手动写 config.ini | 交互式引导，30 秒完成 |
| 学习曲线 | 中等 | 陡峭 | 低 |
| 用户群体 | datahoarder、开发者 | 重度 Pixiv 用户 | 二次元爱好者 |
| 输出语言 | 英文 | 英文/日文 | 中文 |

**差异化核心**：picals-crawler 是唯一一个「不需要任何技术背景就能用的 Pixiv 专用 CLI 下载器」。

---

## 八、非功能需求

- **性能**：单作品下载 < 5s（正常网络），100 幅作品批量下载 < 5min（8 并发）
- **可靠性**：统一请求层对 `429 / 5xx / timeout` 做收敛；失败项持久化为 manifest，并可通过 `retry` 回放
- **兼容性**：macOS (aarch64 + x86_64)、Windows (x86_64)、Linux (x86_64)
- **安全性**：凭据文件权限 600，敏感信息不在日志中输出
- **可维护性**：Pixiv API 变更时，只需修改 selector 模块

---

## 九、明确不做

以下功能**明确不在 v1 范围内**：

- ❌ Web UI / 浏览器扩展
- ❌ Ugoira 动图转 GIF/MP4（v1.1 考虑）
- ❌ 多账号管理
- ❌ 自定义文件名模板（v1.1 考虑）
- ❌ 定时下载任务
- ❌ Docker 部署
- ❌ 作为 npm/Python 库提供给其他程序调用
- ❌ 下载小说（Novel）

---

## 十、版本规划

- 当前版本判断已经更新为：
  - `v0.1.0 / v0.2.0` 对应的主链路与功能完善目标已完成
  - `v0.3.x` 阶段重点已转向 README、分发与发布准备
  - `v1.0.0` 仍以安装体验、文档与稳定发布节奏为主要门槛

历史路线图与交互样例已移动到附录：[[Picals Crawler 产品设计附录：交互与路线图]]
