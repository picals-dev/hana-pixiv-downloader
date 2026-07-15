# hana-pixiv-downloader

基于 Rust 的轻量 Pixiv CLI 下载器。

## 安装

macOS / Linux：

```bash
curl -fsSL https://raw.githubusercontent.com/picals-dev/hana-pixiv-downloader/master/install/hpd.sh | bash
```

Windows PowerShell：

```powershell
powershell -c 'irm https://raw.githubusercontent.com/picals-dev/hana-pixiv-downloader/master/install/hpd.ps1 | iex'
```

## 快速开始

首次使用先运行：

```bash
hpd setup
```

填写 `PHPSESSID`，自动探测当前账号 `user_id`，并初始化下载目录、并发、超时、重试和代理等配置。

## 常用命令

### 直接粘贴 Pixiv URL

```bash
hpd download "https://www.pixiv.net/users/12345678"
hpd download "https://www.pixiv.net/artworks/12345678"
hpd download "https://www.pixiv.net/tags/初音ミク/artworks"
```

### 下载指定画师

```bash
hpd download user 12345678
hpd download user 12345678 --count 20
hpd download user "https://www.pixiv.net/users/12345678" --with-tags
```

### 下载关键词搜索结果

```bash
hpd download keyword "初音ミク"
hpd download keyword "オリジナル 女の子" --count 100 --sort date_asc
hpd download keyword "東方Project" --r18 --no-ai
```

### 下载排行榜

```bash
hpd download ranking
hpd download ranking weekly --count 50
hpd download ranking daily_r18
```

支持的排行榜模式：

- `daily`
- `weekly`
- `monthly`
- `male`
- `female`
- `daily_r18`
- `weekly_r18`

### 下载单张作品

```bash
hpd download illust 12345678
hpd download illust "https://www.pixiv.net/artworks/12345678"
```

### 下载自己的收藏

```bash
hpd download bookmark
hpd download bookmark --count 200 --with-tags
```

### 查看和修改配置

```bash
hpd config show
hpd config set proxy.url socks5://127.0.0.1:7890
hpd config set download.concurrent 16
hpd config set download.with_tags true
hpd config set download.roots.user ~/Pictures/Pixiv/user
hpd config set auth.phpsessid <PHPSESSID>
hpd config set auth.user_id <USER_ID>
hpd config clean
```

### 重试失败项

```bash
hpd retry /path/to/failures.toml
```

当批量下载里仍有可重试失败项时，工具会生成 manifest，后续如果想要再重试可直接凭此 `retry`。

### 更新 hpd

```bash
hpd update
hpd upgrade
```

两个命令行为相同：检查 GitHub Releases 的最新正式版、下载当前平台资产、校验 SHA-256 后原地替换可执行文件。仅支持通过本项目官方安装脚本安装的预编译二进制；通过 Cargo、Homebrew 或源码构建安装时，请使用原安装方式更新。

## 常用选项

大多数下载命令都支持这些选项：

| 选项 | 说明 |
| --- | --- |
| `--to <PATH>` | 覆盖当前模式的下载根目录 |
| `--proxy <URL>` | 为本次下载指定代理 |
| `--count <N>` | 限制下载数量，`0` 表示全部 |
| `--sort <date_desc\|date_asc>` | 指定时间排序 |
| `--no-ai` | 排除 AI 作品 |
| `--concurrent <N>` | 指定并发下载数 |
| `--timeout <SECONDS>` | 指定单次请求超时 |
| `--retry <N>` | 指定网络错误重试次数 |
| `--with-tags` / `--no-tags` | 控制是否导出 `tags.json` |
| `--dry-run` | 只预览本次计划，不实际下载 |
| `--verbose` | 打开详细日志 |

## 配置文件

配置优先级：

```text
CLI 参数 > 环境变量 > config.toml > 默认值
```

默认配置目录（可通过 `XDG_CONFIG_HOME` 覆盖）：

- macOS / Linux：`~/.config/hana-pixiv-downloader/`
- Windows：`%AppData%\\hana-pixiv-downloader\\`

其中：

- `config.toml` 保存普通配置
- `credentials` 保存 `PHPSESSID` 和 `user_id`
- 如需恢复到首次使用状态，可运行 `hpd config clean` 清空整个配置目录

常用配置键：运行 `hpd config show`

常用环境变量：

- `HTTPS_PROXY`：通用 HTTPS 代理地址；未设置 `HPD_PROXY_URL` 时作为请求代理的回退值。
- `HPD_PROXY_URL`：本工具专用代理地址；优先级高于 `HTTPS_PROXY`。
- `XDG_CONFIG_HOME`：配置根目录；程序会在其下使用 `hana-pixiv-downloader/` 目录读写配置与凭据。
