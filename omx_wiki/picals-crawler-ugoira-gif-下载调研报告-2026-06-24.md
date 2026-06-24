---
title: "Picals Crawler Ugoira GIF 下载调研报告 2026-06-24"
tags: ["research", "ugoira", "gif", "pixiv", "download"]
created: 2026-06-24T20:30:00.000Z
updated: 2026-06-24T20:30:00.000Z
sources:
  - "src/crawler/illust.rs"
  - "src/crawler/shared.rs"
  - "src/downloader/mod.rs"
  - "src/downloader/image.rs"
  - "src/net/catalog.rs"
  - "src/net/session.rs"
  - "src/pixiv/selector.rs"
  - "src/output.rs"
  - "tests/fixtures/illust_detail.json"
  - "https://www.pixiv.net/ajax/illust/{id}"
  - "https://www.pixiv.net/ajax/illust/{id}/pages"
  - "https://www.pixiv.net/ajax/illust/{id}/ugoira_meta"
  - "https://app-api.pixiv.net/v1/ugoira/metadata?illust_id={id}"
  - "https://github.com/mikf/gallery-dl"
  - "https://github.com/Nandaka/PixivUtil2"
  - "https://github.com/lifegpc/pixiv_downloader"
  - "https://github.com/upbit/pixivpy"
  - "https://github.com/xuejianxianzun/PixivBatchDownloader"
  - "https://docs.rs/gif/latest/gif/"
  - "https://docs.rs/zip/latest/zip/"
  - "https://docs.rs/image/latest/image/"
links:
  - "picals-crawler-技术设计文档.md"
  - "picals-crawler-产品设计文档.md"
  - "typescript-原项目实现观察.md"
  - "picals-crawler-下载预览与-ranking-调研报告-2026-06-21.md"
  - "picals-crawler-测试指南.md"
category: architecture
confidence: high
schemaVersion: 1
---

# Picals Crawler Ugoira GIF 下载调研报告 2026-06-24

## 结论摘要

本次调研的核心结论有四条：

1. Pixiv 所谓“GIF 图”在实现上并不是直接存一份 `.gif` 文件，而是 **ugoira**：
   - 作品详情里表现为 `illustType = 2`
   - `urls.original` 指向封面帧，例如 `..._ugoira0.jpg` / `..._ugoira0.png`
   - 真正的动画元数据与帧时序在专用端点 `/ajax/illust/{id}/ugoira_meta`
2. 目前 `picals-crawler` 之所以只会下载封面 JPG，本质原因不是“少改一个后缀”，而是整条下载链路都建立在 `pages -> original image urls -> 每个 URL 落一个文件` 这一静态图假设上。
3. 主流项目真正支持 Pixiv 动图下载时，普遍都会走 **两段式链路**：
   - 先识别作品是 ugoira
   - 再请求 `ugoira_meta`，拿到 zip 包地址与 `frames[]` 时序，然后再转 GIF / WebM / WebP / APNG
4. 对 `picals-crawler` 来说，最稳妥的首版集成方案不是把现有 `DownloadItem` 硬改成特殊分支，而是补一条独立的 **ugoira 规划与编码管线**。  
   推荐的 v1 路线是：`ugoira_meta.originalSrc zip -> 解压帧 -> GIF 编码 -> 原子写入 .gif`。  
   这和主流下载器最一致，也最符合本项目“单二进制、轻依赖、可测试”的方向。

---

## 1. 先把术语说准

### 1.1 Pixiv 的“GIF 图”其实是什么

Pixiv 动图的正式类型是 **ugoira**，不是上传后一份现成的 `.gif` 文件。

证据：

- Pixiv 作品详情响应里，动图作品使用 `illustType = 2`。PixivUtil2 的测试数据 `test-image-ugoira-46281014.json` 直接展示了这一点，同时 `urls.original` 指向 `..._ugoira0.jpg`。
- `pixivpy` 的 App API 暴露了专门的 `ugoira_metadata(illust_id)`，底层调用 `GET /v1/ugoira/metadata`。
- `gallery-dl`、`PixivUtil2`、`PixivBatchDownloader`、`pixiv_downloader` 都有单独的 ugoira 分支，而不是把它当普通单图处理。

### 1.2 为什么它现在会落成封面 JPG

`picals-crawler` 当前链路是：

1. crawler 先拿作品 ID
2. 通过 `PixivNetSession::fetch_illust_pages()` 请求 `/ajax/illust/{id}/pages`
3. `select_page_original_urls()` 从 `body[].urls.original` 中提取 URL
4. `DownloadItem` 直接按 URL 文件名落盘

对应位置：

- [src/crawler/illust.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/illust.rs:40)
- [src/crawler/shared.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/shared.rs:17)
- [src/pixiv/selector.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/pixiv/selector.rs:131)
- [src/downloader/image.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/downloader/image.rs:18)

这条链路对普通插画完全成立，但对 ugoira 只会拿到 `..._ugoira0.jpg/png` 这一张封面帧。

结论：

- 当前问题不是“下载结果扩展名不对”
- 而是“作品资产模型不对”

---

## 2. Pixiv ugoira 的真实数据模型

## 2.1 作品详情层：它告诉你“这是一张动图作品”

作品详情端点：

- `GET /ajax/illust/{id}`

它至少提供这些关键信号：

- `illustType = 2`
- `pageCount = 1`
- `urls.original = ..._ugoira0.jpg/png`

这说明：

- 作品在 Pixiv 语义上是“单作品”
- 但这个“单作品”不是单张静态图
- `urls.original` 更像“封面帧 / 第 0 帧入口”，不是完整动画文件

### 2.2 专用元数据层：它告诉你“动画该怎么播放”

Web 端专用端点：

- `GET /ajax/illust/{id}/ugoira_meta`

App API 对应端点：

- `GET /v1/ugoira/metadata?illust_id={id}`

主流项目从这里读取的稳定字段是：

- `src`
  - 600x600 尺寸 zip
- `originalSrc`
  - 原尺寸 zip
- `frames`
  - 帧列表，每帧至少有 `file` 与 `delay`
- `mime_type`
  - zip 包中帧图像的 MIME 类型

这也是所有“真正能下载 Pixiv 动图”的项目共有的核心依赖。

### 2.3 `frames` 字段为什么是关键

ugoira 不是简单的视频帧序列。它有逐帧 `delay`。

例如主流实现都会使用：

- `frames[i].file`
- `frames[i].delay`

去构造 ffmpeg concat 文件、浏览器端逐帧 GIF 编码，或本地视频编码输入。

如果没有 `frames`：

- 你不知道播放顺序
- 你不知道每帧停留多久
- 你无法正确生成 GIF

### 2.4 `originalSrc zip` 和 “原始帧” 不是一回事

这是调研里最容易被忽略，但最关键的一点。

证据：

- `PixivBatchDownloader` 的研究笔记 `notes/预览动图.md` 明确记录：  
  `originalSrc` 指向原尺寸 zip，但 zip 内帧图通常是压缩过的 JPEG，体积显著小于作者上传原始帧总量。
- `gallery-dl` 专门保留了 `ugoira original` 模式：
  - 默认可以直接下载 zip
  - 也可以根据 `..._ugoira0.<ext>` 推导每一帧原图 URL，逐帧抓取真正的 original frames

因此必须把两个概念分开：

- **zip 模式**
  - 请求数少
  - 主流下载器最常用
  - 但可能不是作者上传原始帧的逐字节原件
- **original frames 模式**
  - 质量潜力最高
  - 但请求数高、耗时高、实现复杂度更高

这不是猜测，而是由主流实现的分支设计反向证明的。

---

## 3. 主流项目到底怎么做

## 3.1 gallery-dl

项目：

- [gallery-dl](https://github.com/mikf/gallery-dl)
- GitHub 指标（2026-06-24 查询）：18.6k stars，最近更新时间 2026-06-23

实现特点：

- 在 Pixiv extractor 中先识别 `work["type"] == "ugoira"`
- 然后调用 `_extract_ugoira()`
- `_extract_ugoira()` 读取 `/illust/{id}/ugoira_meta` 或 App API `ugoira_metadata`
- 支持两种模式：
  - 下载 zip
  - 逐帧推导 original frame URLs

关键观察：

- 这是本轮调研里“最彻底”的实现
- 它明确承认 zip 与 original frames 是两个不同策略
- 说明“真正支持 ugoira”不等于“只要拿到一个 zip 就万事大吉”

对我们最有价值的启示：

- **作品资产建模必须区分普通图与 ugoira**
- **后续如果要做高质量模式，架构上要给 original frames 留扩展口**

## 3.2 PixivUtil2

项目：

- [PixivUtil2](https://github.com/Nandaka/PixivUtil2)
- GitHub 指标（2026-06-24 查询）：2.6k stars，最近更新时间 2026-06-23

实现特点：

- 通过详情里 `urls.original` 是否包含 `ugoira` 判断作品模式
- 请求 `/ajax/illust/{id}/ugoira_meta`
- 把 `ugoira600x600.zip` 升格为 `ugoira1920x1080.zip`
- 下载 zip 后，解压 `animation.json`
- 再借助 ffmpeg 生成 GIF / APNG / WebP / WebM / MKV

关键观察：

- 这是“经典 CLI 下载器”路线
- GIF 生成不是手搓，而是交给 ffmpeg
- 它在 concat 文件里额外重复最后一帧，以修正最后一帧时间戳问题

对我们的启示：

- GIF 转码不仅是“遍历帧写出去”  
  还要考虑最后一帧时长、调色板、编码失败与中间产物清理。

## 3.3 PixivBatchDownloader

项目：

- [PixivBatchDownloader](https://github.com/xuejianxianzun/PixivBatchDownloader)
- GitHub 指标（2026-06-24 查询）：5.3k stars，最近更新时间 2026-06-23

实现特点：

- 详情页识别 `illustType === 2`
- 请求 `API.getUgoiraMeta(body.id)`
- 保存：
  - `original = meta.body.originalSrc`
  - `regular/small = meta.body.src`
  - `ugoiraInfo.frames`
  - `ugoiraInfo.mime_type`
  - `originalThumbnail = body.urls.original`
- 浏览器端可转：
  - WebM
  - WebP
  - GIF
  - APNG
  - ZIP / Ugoira 容器

关键观察：

- GIF 不是它推荐的默认格式，WebP / WebM 优先级更高
- 转 GIF 时直接把每一帧的 `delay` 喂给 `gif.js`
- 项目维护者自己写了大量关于体积、预览、后台页性能的研究笔记

对我们的启示：

- “能导出 GIF” 与 “推荐默认导出 GIF” 是两个问题
- 如果项目目标是“先实现真 GIF 文件”，首版可以做；  
  但产品默认值未必应该长期偏向 GIF

## 3.4 pixiv_downloader（Rust）

项目：

- [pixiv_downloader](https://github.com/lifegpc/pixiv_downloader)
- GitHub 指标（2026-06-24 查询）：22 stars，最近更新时间 2025-06-04

实现特点：

- 单独有 `download_artwork_ugoira()`
- 先 `get_ugoira(id)` 读 `/ajax/illust/{id}/ugoira_meta`
- 下载 `originalSrc`
- 把 `frames` 落成 JSON
- 再走本地 `ugoira` 编码库 / 子进程，把 zip 转成 MP4

关键观察：

- 它虽然不是最主流项目，但对 Rust 侧集成很有参考价值
- 它已经把 ugoira 视为与 image 完全不同的一类下载任务

对我们的启示：

- Rust 代码里单开 `ugoira` 模块是自然做法
- 不必勉强把它塞回“图片 URL 列表”

## 3.5 pixivpy

项目：

- [pixivpy](https://github.com/upbit/pixivpy)
- GitHub 指标（2026-06-24 查询）：2.0k stars，最近更新时间 2026-06-18

价值不在下载，而在 API 语义：

- 它明确暴露了 `ugoira_metadata(illust_id)`
- 调用目标是 `/v1/ugoira/metadata`

这进一步证明：

- Pixiv 官方 App API 把 ugoira 视作专门的数据模型
- 不是社区项目“自创”的特例

---

## 4. 从主流实现提炼出的稳定模式

无论语言和运行时差异多大，真正支持 ugoira 的项目都在做下面几件事：

1. **识别作品类型**
   - 常见依据：`illustType = 2`，或 `type == "ugoira"`，或 `urls.original` 包含 `_ugoira0`
2. **单独拉取 ugoira 元数据**
   - `/ajax/illust/{id}/ugoira_meta`
   - 或 `/v1/ugoira/metadata`
3. **把 `frames[]` 当作一等输入**
   - 不能丢
4. **zip 与最终输出分离**
   - zip 是输入
   - gif/webm/webp/apng 是输出
5. **转换层单独处理**
   - 不是“下载完文件就结束”
6. **大多数实现把 GIF 当兼容性输出，而不是最佳默认输出**

这六条就是 picals-crawler 后续实现时应当遵守的“行业共识”。

---

## 5. picals-crawler 现状与结构性缺口

## 5.1 当前资产模型只有一种：静态图片 URL

当前下载模型是：

- `DownloadItem { illust_id, image_url, target_dir }`

它有两个强假设：

1. 一个下载单元就是一个远端图片 URL
2. 本地目标文件名可直接从 URL 推导

这对 ugoira 不成立，因为 ugoira 的最终目标不是下载 `..._ugoira0.jpg`，而是“拿到动画输入后再生成 `.gif`”。

## 5.2 批量链路把 `/pages` 当成 SSOT

批量模式统一走：

- `collect_download_items_for_illust_ids()`

它只做一件事：

- 对每个 `illust_id` 请求 `/ajax/illust/{id}/pages`

这意味着：

- 没有识别作品类型的步骤
- 没有请求 `ugoira_meta` 的机会
- 没有把“作品 -> 下载计划”建模成 enum

## 5.3 标签导出与类型识别目前是分离的

当前 tags 导出已经会额外请求：

- `/ajax/illust/{id}`

这很重要，因为它说明：

- 我们其实已经为每个作品支付过 detail 请求成本
- 只是这些 detail 结果现在只用于 tags，没有用于下载规划

这给了一个明显的重构方向：

- 把 detail 请求前置到“作品规划”阶段
- 同时复用给 tags 导出

## 5.4 当前失败模型里还没有“转换失败”语义

现在 `FailureStage` 只有：

- `Collect`
- `Download`
- `Tags`

如果未来支持 ugoira GIF：

- zip 下载失败是 `Download`
- 但 GIF 编码失败不应再伪装成普通图片下载失败

因此建议新增：

- `FailureStage::Convert`

这样 replay / 诊断才不会混淆。

---

## 6. 我对本项目的推荐集成方案

## 6.1 总体原则

推荐原则有四条：

1. **继续以 Web Ajax API 为主，不引入 App API 认证栈**
   - 当前 net 层已经稳定在 Web Ajax 路线
   - 加一个 `/ajax/illust/{id}/ugoira_meta` 成本最低
2. **把 ugoira 当成独立作品资产类型，而不是特殊图片 URL**
3. **首版先实现“真实 GIF 文件闭环”，不抢做 ZIP / Ugoira / WebM 多格式矩阵**
4. **避免外部 ffmpeg 依赖，优先保持单二进制体验**

## 6.2 推荐的最小可落地路线

### 第一步：补 Pixiv 领域模型

建议新增如下概念：

- `IllustKind`
  - `SingleImage`
  - `Manga`
  - `Ugoira`
- `UgoiraFrame`
  - `file`
  - `delay_ms`
- `UgoiraMeta`
  - `src`
  - `original_src`
  - `mime_type`
  - `frames`
- `ArtworkDownloadPlan`
  - `Images(Vec<DownloadItem>)`
  - `Ugoira(UgoiraDownloadPlan)`

这样以后 crawler 拿到的不是“图片 URL 列表”，而是“作品下载计划”。

### 第二步：补 net 层端点

建议新增：

- `RequestKind::UgoiraMeta`
- `PixivCatalog::illust_ugoira_meta(illust_id)`
- `PixivNetSession::fetch_ugoira_meta(illust_id)`

理由：

- 这完全符合当前 `PixivNetSession` 作为唯一 façade 的 SSOT 方向
- 不会把新 URL 模板泄漏回 crawler 层

### 第三步：把 detail 请求前置为“作品规划”

当前批量模式拿到的是裸 `illust_id`。  
推荐改成：

1. 对每个 `illust_id` 先请求 detail
2. 根据 `illustType` 决定：
   - 普通图 / manga：继续走 `/pages`
   - ugoira：走 `/ugoira_meta`
3. 把 detail 结果缓存或直接传给 tags 导出，避免重复请求

这是本轮最值得做的结构调整。

理由：

- 最稳
- 逻辑最清楚
- 与当前 tags 导出天然可复用
- 不依赖搜索 / 收藏 / ranking 列表响应里是否稳定带 `type`

### 第四步：新增独立 `downloader::ugoira`

建议不要把 GIF 编码硬塞到 `downloader::image`。

更合理的做法是新增：

- `src/downloader/ugoira.rs`

大致职责：

1. 下载 `originalSrc` zip 到临时文件
2. 解压 zip
3. 依据 `frames[]` 顺序与 delay 生成 GIF
4. 写入 `<illust_id>.gif.part`
5. 成功后 rename 为 `<illust_id>.gif`
6. 清理 zip 临时文件与解压目录

目标路径建议：

- `illust` 模式：`<root>/<illust_id>/<illust_id>.gif`
- 批量模式：`<context>/<illust_id>/<illust_id>.gif`

这样与当前目录布局保持一致。

## 6.3 GIF 编码实现：我建议的首版技术路线

### 方案 A：纯 Rust 管线

依赖候选：

- `zip`
  - 负责读取完整 zip
- `image`
  - 负责解码 zip 中的 jpg/png 帧
- `gif`
  - 负责编码 GIF

官方文档证据：

- `zip` 文档说明它支持读写简单 ZIP 文件
- `image::load_from_memory()` 可从内存解码图片
- `gif::Frame.delay` 的单位是 10ms

优点：

- 不需要系统安装 ffmpeg
- CI 与跨平台分发简单
- 更符合本项目“开箱即用 CLI”的产品方向

缺点：

- GIF 调色板质量大概率不如 ffmpeg `palettegen/paletteuse`
- 需要自己处理内存与帧延迟量化

### 方案 B：外部 ffmpeg 管线

优点：

- 主流成熟
- GIF 调色板效果通常更好
- 最后一帧与时序问题更容易复用成熟参数

缺点：

- 引入系统依赖
- 破坏“单二进制”体验
- 测试和用户环境诊断复杂很多

### 结论

对 `picals-crawler` 的首版目标，我推荐 **方案 A**。  
如果后续用户反馈“GIF 质量明显不够”，再评估：

- 可选 ffmpeg backend
- 或更高质量但更重的 GIF 编码方案

## 6.4 一个必须提前接受的格式事实

GIF 不是无损容器，而且它的帧延迟单位是 **10ms**。

因此从 Pixiv 的 `delay(ms)` 写入 GIF 时，必须做量化：

- 推荐：`delay_cs = max(1, round(delay_ms / 10.0))`

含义：

- 1 个 GIF delay unit = 10ms
- 如果 Pixiv 将来出现不是 10ms 整倍数的帧延迟，GIF 端只能近似表示

这不是实现缺陷，而是 GIF 格式本身的边界。

## 6.5 关于“要不要直接下载原始帧而不是 zip”

这是本轮报告里的关键取舍。

### 我对 v1 的建议

首版先用：

- `ugoira_meta.originalSrc` zip -> GIF

理由：

- 这是多数主流项目的默认路径
- 请求数最少
- 实现复杂度显著更低
- 足够完成“真正导出 .gif 文件”的目标

### 但我建议在设计上保留未来扩展口

未来可以增加：

- `ugoira_source = compressed_zip | original_frames`

因为 `gallery-dl` 已经证明：

- original frames 模式是有价值的
- 它代表更高画质路线

所以架构上最好不要把“uгоira 输入一定是 zip”写死在所有层里。

---

## 7. 我会如何拆到当前仓库

推荐的落点如下：

### `src/net/catalog.rs`

- 新增 `RequestKind::UgoiraMeta`
- 新增 `illust_ugoira_meta()`

### `src/net/session.rs`

- 新增 `fetch_ugoira_meta()`
- 视实现需要，抽出更通用的二进制下载接口

### `src/pixiv/selector.rs`

- 新增 `select_illust_type()`
- 新增 `select_ugoira_meta()` 或拆成：
  - `select_ugoira_original_src()`
  - `select_ugoira_frames()`
  - `select_ugoira_mime_type()`

### `src/crawler/shared.rs`

- 把 `collect_download_items_for_illust_ids()` 升级为“收集下载计划”
- 复用 detail 结果给 tags 导出

### `src/downloader/`

- 新增 `ugoira.rs`
- `mod.rs` 层面允许下载结果汇总同时包含：
  - image 下载
  - ugoira 转 GIF

### `src/error.rs` / `src/failure.rs`

- 补充转换错误与失败阶段

### `tests/fixtures/`

- 新增：
  - `ugoira_illust_detail.json`
  - `ugoira_meta.json`

---

## 8. 测试策略建议

必须遵守当前测试 SSOT：[[Picals Crawler 测试指南]]

推荐测试分层如下：

### Layer 1：单元测试

- `select_illust_type()`
- `select_ugoira_frames()`
- GIF delay 量化
- ugoira 输出路径生成

### Layer 2：contracts

- `ugoira_meta` fixture contract
- `ugoira detail -> type` contract

### Layer 3：app

- `download illust <ugoira_id>` 通过 wiremock 落出真实 `.gif`
- `download user` / `download keyword` 混合普通图 + ugoira
- 转换失败时产生 `FailureStage::Convert`

### Layer 4：cli

- 黑盒验证真实二进制能生成 `.gif`
- `--dry-run` 不生成文件
- 失败 manifest 可读

### 一个实现层面的建议

为了让测试稳定、无外部依赖：

- 首版不要把 ffmpeg 作为必须条件
- 集成测试里使用一个极小的本地 zip fixture 即可

---

## 9. 风险与取舍

### 风险 1：GIF 文件体积会大

这是格式问题，不是实现 bug。  
主流项目里很多都不把 GIF 当默认首选，就是因为体积与画质都不占优。

### 风险 2：zip 模式不是作者原始逐帧文件

如果用户未来追求“最接近作者上传原件”，需要 original frames 路线。

### 风险 3：纯 Rust GIF 质量可能不如 ffmpeg

这是我最明确的技术风险判断。  
如果 v1 只追求功能闭环，可接受；如果追求更优观感，后续要继续迭代。

### 风险 4：批量模式会增加 detail 请求

这是可以接受的：

- tags 本来就会额外请求 detail
- 通过复用 detail 结果，可以把新增成本压回去

---

## 10. 最终建议

如果现在就要为 `picals-crawler` 真正补齐 Pixiv 动图下载，我的建议是：

1. 把 Pixiv 动图明确建模为 `ugoira`，不要继续走“图片 URL 列表”假设
2. 在 net 层补 `/ajax/illust/{id}/ugoira_meta`
3. 在 collect 阶段先请求 detail，再分流普通图与 ugoira
4. 首版采用 `originalSrc zip -> GIF` 的纯 Rust 实现
5. 复用 detail 响应给 tags 导出，避免重复请求
6. 增加 `FailureStage::Convert`
7. 先把“真实 `.gif` 文件写入本地”做稳，再考虑：
   - ZIP / `.ugoira` 原样保存
   - WebM / WebP / APNG
   - original frames 高质量模式

一句话总结：

- **现在的缺口不是“少一个后缀”，而是“缺一整条 ugoira 资产管线”。**
- **最合适的首版方案，是沿着主流项目的共识，先实现 `ugoira_meta + zip + frames + GIF 编码` 这条最短闭环。**
