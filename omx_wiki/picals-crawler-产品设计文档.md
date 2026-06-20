---
title: "Picals Crawler 产品设计文档"
tags: ["product", "design", "requirements", "cli", "pixiv"]
created: 2026-06-16T00:00:00.000Z
updated: 2026-06-19T00:00:00.000Z
sources: ["_notes/nea/product-design.md"]
links: ["picals-crawler-技术设计文档.md", "picals-crawler-项目理解基线.md", "typescript-原项目实现观察.md"]
category: product
confidence: high
schemaVersion: 1
---

# Picals-Crawler 产品设计文档

> 版本：v0.2.0-draft
> 最后更新：2026-06-19
> 状态：v0.2.0 / Phase 2 功能已完成，bookmark 已实现

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
4. 可选：设置默认下载目录（回车使用默认值 `~/Pictures/Pixiv/`）
5. 认证元数据持久化至 `~/.config/picals-crawler/credentials`（权限 600）

**设计原则**：只做一次，用户永远不需要再想认证的事。

#### `picals-crawler download user <id|url>`

```
picals-crawler download user 12345678
picals-crawler download user "https://www.pixiv.net/users/12345678"
picals-crawler download user 12345678 --to ~/wallpaper/miku/
```

- 支持数字 ID 和完整 URL 两种输入方式
- URL 模式：用户直接从浏览器复制粘贴，零认知成本
- 默认下载全部作品，按时间降序排列
- 当前实现使用 `{画师ID}/` 子目录
- 下载过程中显示进度条、速度、ETA

#### `picals-crawler download keyword <query>`

```
picals-crawler download keyword "初音ミク"
picals-crawler download keyword "オリジナル 女の子" --count 100 --sort date_asc
```

- 支持多关键词（空格分隔）
- 选项：`--count` 数量、`--sort` 排序（date_desc / date_asc）、`--r18` 模式切换、`--no-ai`
- 默认：全部结果、按时间降序、安全模式

#### `picals-crawler download ranking`

```
picals-crawler download ranking
picals-crawler download ranking --mode weekly --count 100
picals-crawler download ranking --mode daily_r18
```

- 排行模式：daily / weekly / monthly / daily_r18 / weekly_r18 / male / female
- 默认：daily（今日插图榜）；若未指定 `--count`，当前实现按“下载全部抓到的结果”处理
- 当前实现仅支持 illust 下载，不包含 manga / ugoira 专项模式

#### `picals-crawler download illust <id>`

```
picals-crawler download illust 12345678
```

- 下载单张作品的所有图片（Pixiv 上多图作品用 `_p0`、`_p1` 等区分）
- 适用于只想下载特定一张图的场景

#### `picals-crawler download bookmark`

```
picals-crawler download bookmark
picals-crawler download bookmark --count 200
```

- 下载自己收藏的作品
- 依赖 setup 中保存的认证元数据：`PHPSESSID + userId`

### 3.3 全局选项

| 选项 | 说明 |
|---|---|
| `--to <path>` | 覆盖下载目录 |
| `--proxy <url>` | 代理地址（如 `socks5://127.0.0.1:1080`），也支持 `HTTPS_PROXY` 环境变量 |
| `--dry-run` | 只列出将要下载的内容，不实际下载 |

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

---

## 四、CLI 交互设计

### 4.1 首次使用完整流程

```
$ picals-crawler setup

  🌸 欢迎使用 Picals Crawler！

  在开始下载之前，需要先完成 Pixiv 认证。请按以下步骤操作：

  ─────────────────────────────────────────────

  Step 1: 在浏览器中打开 https://www.pixiv.net 并登录你的 Pixiv 账号

  Step 2: 登录后，按 F12 打开开发者工具
        → 点击顶部的 "Application" 标签
        → 左侧找到 Cookies → https://www.pixiv.net
        → 找到 PHPSESSID 这一项

  Step 3: 复制 PHPSESSID 的值（一串字母数字），粘贴到下面：

  PHPSESSID: █

  ─────────────────────────────────────────────

  可选：设置默认下载目录（回车使用默认值：~/Pictures/Pixiv/）

  下载目录: █

  ✅ 配置完成！认证信息与当前账号身份已保存。

  现在可以开始下载了：

    picals-crawler download user <画师ID>
    picals-crawler download bookmark

  查看完整帮助: picals-crawler --help
```

### 4.2 日常下载体验

```
$ picals-crawler download user 12345678

  👤 作者: はな (ID: 12345678)
  🖼️  共 247 幅作品

  [00:15] ████████████████░░░░  83%  (206/247)
  📥 已下载: 152.3 MB  ⚡ 速度: 10.2 MB/s  ⏱ 预估剩余: 3s

  ✅ 下载完成！
  📂 /Users/nonhana/Pictures/Pixiv/はな(12345678)/
     ├── 12345678_p0.jpg
     ├── 12345679_p0.png
     ├── 12345679_p1.png  (多图作品)
     ├── ...
     └── tags.json
```

### 4.3 断点续传

```
$ picals-crawler download user 12345678

  👤 作者: はな (ID: 12345678)
  🖼️  共 247 幅作品
  ⏭️  已跳过 206 幅（已下载）
  📥 剩余 41 幅

  [00:03] ████████████████████  100%  (41/41)
  ...
```

### 4.4 错误处理

```
$ picals-crawler download user 99999999

  ❌ 错误: 未找到该用户，请检查用户 ID 是否正确
```

```
$ picals-crawler download user 12345678

  ❌ 错误: 未找到认证信息，请先运行 picals-crawler setup
```

- 错误信息中文化
- 网络错误时自动重试（默认 3 次），并显示重试进度
- 部分下载失败不影响整体流程，最后汇总失败列表

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
[download]
directory = "~/Pictures/Pixiv"
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
- **可靠性**：网络错误自动重试 3 次，支持断点续传
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

### v0.1.0 — MVP（当前目标）

- `picals-crawler setup`（交互式认证）
- `picals-crawler download user <id|url>`（下载画师作品）
- 基本进度条
- 断点续传
- macOS / Windows 预编译二进制
- GitHub Release 自动构建

> 现状注记（2026-06-18）：上述主链路已经完成。当前仓库的主要实施目标已切换到 v0.2.0 / Phase 2。

### v0.2.0 — 功能完善

- `download keyword / ranking / illust / bookmark`
- `config show / set`
- tags.json 保存
- 彩色进度条（速度、ETA）

> 现状注记（2026-06-20）：`download illust / keyword / ranking / bookmark`、`config show / set` 的 Phase 2 约束、`tags.json`、`.part` 恢复语义、速度与 ETA 统计 seam 已完成。当前实现采用“setup 保存 `userId` 认证元数据”的方案，bookmark 不再是 deferred/blocked。

### v0.3.0 — 体验打磨

- Homebrew formula 提交
- Scoop bucket 创建
- 错误信息中文化完善
- 项目文档（中文 README）

### v1.0.0 — 正式发布

- 全功能稳定
- Ugoira 支持
- 文件名模板
- crates.io 发布
