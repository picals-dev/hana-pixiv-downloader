# hana-pixiv-downloader

`hana-pixiv-downloader` 是一个面向中文用户、开箱即用的 Pixiv 图片下载 CLI。

- 安装包名是 `hana-pixiv-downloader`
- 实际命令名是 `hpd`
- 支持用户作品、关键词、排行榜、单作品、收藏下载
- 支持直接粘贴 Pixiv URL
- 支持 ugoira 自动导出 GIF
- 支持代理、`--dry-run`、`tags.json` 导出、失败重试

## 安装

### 通过 Cargo

```bash
cargo install hana-pixiv-downloader --locked
```

需要 Rust 1.85 或更高版本。

### 通过 GitHub Releases

从 [GitHub Releases](https://github.com/picals-dev/hana-pixiv-downloader/releases) 下载对应平台的预编译二进制，并将 `hpd` 加入 `PATH`。

当前发布目标：

- macOS `aarch64`
- macOS `x86_64`
- Linux `x86_64`
- Windows `x86_64`

## 快速开始

首次使用先运行：

```bash
hpd setup
```

`setup` 会引导你填写 `PHPSESSID`，自动探测当前账号 `user_id`，并初始化下载目录、并发、超时、重试和代理等配置。

## 常用命令

### 直接粘贴 Pixiv URL

```bash
hpd download "https://www.pixiv.net/users/12345678"
hpd download "https://www.pixiv.net/artworks/12345678"
hpd download "https://www.pixiv.net/tags/%E5%88%9D%E9%9F%B3%E3%83%9F%E3%82%AF/artworks"
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
```

### 重试失败项

```bash
hpd retry /path/to/failures.toml
```

当批量下载里仍有可重试失败项时，工具会写出 manifest，后续可直接用 `retry` 回放。

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

默认配置目录：

- macOS / Linux：`${XDG_CONFIG_HOME:-~/.config}/hana-pixiv-downloader/`
- Windows：`%AppData%\\hana-pixiv-downloader\\`

其中：

- `config.toml` 保存普通配置
- `credentials` 保存 `PHPSESSID` 和 `user_id`

常用配置键：

- `auth.phpsessid`
- `auth.user_id`
- `download.count`
- `download.sort`
- `download.r18`
- `download.ai`
- `download.concurrent`
- `download.timeout`
- `download.retry`
- `download.with_tags`
- `download.roots.illust`
- `download.roots.user`
- `download.roots.bookmark`
- `download.roots.keyword`
- `download.roots.ranking`
- `proxy.url`

常用环境变量：

- `HTTPS_PROXY`
- `HPD_PROXY_URL`
- `XDG_CONFIG_HOME`

## 输出与行为

- 静态作品会按作品目录落盘图片文件
- ugoira 作品会自动导出为 GIF
- `--with-tags` 会在批次目录写出 `tags.json`
- 重新执行同一条下载命令即可跳过已存在文件并继续补齐

## 注意事项

- `setup` 和 `config show` 会明文显示凭据，避免在录屏或共享屏幕时操作
- 请仅在遵守 Pixiv 服务条款与原作者授权范围内使用本工具

## 帮助

```bash
hpd --help
hpd download --help
hpd config --help
```
