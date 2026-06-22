---
title: "Picals Crawler 下载预览与 Ranking 调研报告 2026-06-21"
tags: ["research", "ux", "pixiv", "ranking", "keyword", "bookmark", "user"]
created: 2026-06-21T16:10:00.000Z
updated: 2026-06-22T15:47:42.000Z
sources: [
  "src/crawler/user.rs",
  "src/crawler/bookmark.rs",
  "src/crawler/keyword.rs",
  "src/crawler/ranking.rs",
  "src/pixiv/selector.rs",
  "https://www.pixiv.net/ajax/search/artworks/%E5%8E%9F%E7%A5%9E?word=%E5%8E%9F%E7%A5%9E&order=date_d&mode=safe&p=1&s_mode=s_tag&type=all&lang=zh",
  "https://www.pixiv.net/ajax/user/11/profile/all?lang=zh",
  "https://www.pixiv.net/ranking.php?format=json&mode=daily&p=1",
  "https://github.com/xuejianxianzun/PixivBatchDownloader",
  "https://github.com/Nandaka/PixivUtil2",
  "https://github.com/mikf/gallery-dl",
  "https://github.com/daydreamer-json/pixiv-ajax-api-docs"
]
links: [
  "picals-crawler-产品设计文档.md",
  "picals-crawler-技术设计文档.md",
  "picals-crawler-项目理解基线.md",
  "picals-crawler-ux-优化执行记录-2026-06-21.md"
]
category: architecture
confidence: high
schemaVersion: 1
---

# Picals Crawler 下载预览与 Ranking 调研报告 2026-06-21

## 结论摘要

本次调研回答两个问题：

1. 预览阶段收集“作品 ID 数量”是否对所有除 `illust` 外的下载模式都可行？
2. Pixiv ranking 到底是有限的还是无限的？

结论如下：

- `user`、`keyword`、`bookmark`、`ranking` 四种批量模式，都可以在“真正下载前”得到本次可下载作品数。
- 但四种模式的“计数成本”并不相同：
  - `keyword`：一次请求即可得到总数，成本最低。
  - `user`：一次请求即可得到总数，但需要对返回 JSON 中的作品 ID map 做计数。
  - `bookmark`：接口协议本身有 `body.total`，理论上一次请求即可得到总数；但该接口依赖登录态。
  - `ranking`：不是无限列表，而是有限榜单；应以接口返回的 `rank_total` 作为总量依据。
- `ranking` 不是“无限往下翻直到没有”，而是**有限榜单**。
  - 当前实测 `daily`、`weekly`、`male`、`female` 都返回 `rank_total = 500`。
  - 当前实测 `monthly` 返回 `rank_total = 479`。
  - 当前实测 `daily` 第 11 页直接 `404`，说明该榜单存在明确页数上界。

因此，后续 UX 设计里：

- `keyword` / `user` / `bookmark` / `ranking` 都适合加入“预览总量 + 限制数量 + 确认顺序”的交互。
- `ranking` 不应按“潜在无限流”来设计，而应按“有限榜单”来设计。

## 1. 当前仓内实现现状

### 1.1 `user`

当前 `user` 模式先请求 `/ajax/user/{user_id}/profile/all`，再从 `body.illusts` 与 `body.manga` 的 key 中提取作品 ID：

- [src/crawler/user.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/user.rs:50)
- [src/pixiv/selector.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/pixiv/selector.rs:18)

这意味着当前主链路本身已经具备“拿到全量作品 ID 再下载”的结构，预览计数不需要新增接口。

### 1.2 `bookmark`

当前 `bookmark` 模式分页请求：

- `/ajax/user/{user_id}/illusts/bookmarks?tag=&offset={offset}&limit={limit}&rest=show&lang=zh`

然后从 `body.works` 提取作品 ID：

- [src/crawler/bookmark.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/bookmark.rs:57)
- [src/pixiv/selector.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/pixiv/selector.rs:93)

现状没有读 `body.total`，而是靠“翻页直到不足一页”收集。

### 1.3 `keyword`

当前 `keyword` 模式请求：

- `/ajax/search/artworks/{keyword}?word=...&order=...&mode=...&p=...&s_mode=s_tag&type=all&lang=zh`

并从 `body.illustManga.data` 取第一页或指定页数的作品 ID：

- [src/crawler/keyword.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/keyword.rs:79)
- [src/pixiv/selector.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/pixiv/selector.rs:33)

值得注意的是：当前实现里 `count = 0` 时只抓 1 页，不是“抓全部”：

- [src/crawler/keyword.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/keyword.rs:74)

### 1.4 `ranking`

当前 `ranking` 模式请求：

- `/ranking.php?mode={mode}&p={page}&format=json`

并从 `contents` 提取作品 ID：

- [src/crawler/ranking.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/ranking.rs:59)
- [src/pixiv/selector.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/pixiv/selector.rs:62)

当前实现里 `count = 0` 时同样只抓 1 页：

- [src/crawler/ranking.rs](/Users/nonhana/code_life/Picals/picals-crawler/src/crawler/ranking.rs:54)

## 2. 小型实验

本次调研编写了一个最小探针脚本，分别验证：

- `keyword` 是否能一次请求拿总数
- `user` 是否能一次请求推导总数
- `ranking` 是否存在明确页数边界

实验脚本核心逻辑如下：

```python
def probe_keyword(query):
    # 读取 body.illustManga.total

def probe_user(user_id):
    # 统计 body.illusts + body.manga 的数字 key 数量

def probe_ranking(mode='daily', max_pages=20):
    # 顺序请求 ranking 页面，直到 404 或 next=false
```

本次实验没有把脚本落入仓库，只作为临时调研工具执行。

### 2.1 `keyword` 实测

请求：

`GET https://www.pixiv.net/ajax/search/artworks/%E5%8E%9F%E7%A5%9E?word=%E5%8E%9F%E7%A5%9E&order=date_d&mode=safe&p=1&s_mode=s_tag&type=all&lang=zh`

实测结果：

- 1 次请求
- 响应耗时约 `866.9 ms`
- 响应体约 `74 KB`
- `body.illustManga.total = 476042`
- 首屏 `data.len = 60`

结论：

- `keyword` 模式可以**一次请求直接拿总数**。
- 这是最适合做“预览数量 + 让用户决定下载多少”的模式。

### 2.2 `user` 实测

请求：

`GET https://www.pixiv.net/ajax/user/11/profile/all?lang=zh`

实测结果：

- 1 次请求
- 响应耗时约 `647.3 ms`
- 响应体约 `86 KB`
- `body` 顶层 key 包含 `illusts`、`manga`
- 统计数字 key 后：
  - `illust_count = 1532`
  - `manga_count = 701`
  - 合计 `2233`

并且真实返回里未看到一个可以直接读取的 `totalIllusts` / `totalManga` 字段。

结论：

- `user` 模式也可以**一次请求拿总数**。
- 方式不是读 `total` 字段，而是**数 `body.illusts` / `body.manga` 的作品 ID key**。
- 这对当前 Rust 实现很友好，因为现有 selector 已经在遍历这些 key。

### 2.3 `bookmark` 实测与限制

匿名请求以下接口：

`GET https://www.pixiv.net/ajax/user/{uid}/illusts/bookmarks?...`

对多个用户 ID 的实测都返回 `400 Bad Request`。

这说明：

- `bookmark` 的真实在线实验依赖登录态。
- 但协议层证据并不缺失，因为：
  - `pixiv-ajax-api-docs` 的样例里明确存在 `body.total`
  - 现有主流项目也在直接读取这个字段

结论：

- `bookmark` 模式可以预览总数，但**必须建立在已登录凭据可用**的前提上。
- 最佳实现方式不是自己翻页统计，而是**首请求直接读 `body.total`**。

### 2.4 `ranking` 实测

#### 2.4.1 `daily` 实测

请求：

`GET https://www.pixiv.net/ranking.php?format=json&mode=daily&p={page}`

结果：

- 第 1 页：`rank_total = 500`，`contents.len = 50`
- 第 10 页：`first_rank = 451`，`last_rank = 500`
- 第 11 页：`404 Not Found`

还观察到：

- 第 10 页的 `next = false`
- 匿名态逐页累加 `contents.len` 得到的是 `499`，而不是 `500`

结论：

- `daily` 榜单是**有限榜单**
- 上界是 `500` 名
- 页数上界是 `10` 页
- 对实现来说，**应以 `rank_total` 作为榜单总量的权威值**，而不是用各页 `contents.len` 简单求和。

#### 2.4.2 多模式横向实测

在与当前实现一致的接口形态下：

`GET /ranking.php?format=json&mode={mode}&p=1`

实测结果：

- `daily`: `rank_total = 500`
- `weekly`: `rank_total = 500`
- `monthly`: `rank_total = 479`
- `male`: `rank_total = 500`
- `female`: `rank_total = 500`

额外说明：

- `daily_r18` / `weekly_r18` 在匿名态下返回 `403`
- 这更像认证限制，不是“无限榜单”的证据

结论：

- `ranking` 不是无限流，而是**每个 mode 都有自己的有限总榜大小**
- 这个总量应以 `rank_total` 为准，而不是以“翻到没数据”为准

## 3. 主流项目证据

### 3.1 Powerful Pixiv Downloader

仓库：

- [xuejianxianzun/PixivBatchDownloader](https://github.com/xuejianxianzun/PixivBatchDownloader)

关键证据：

1. 搜索页直接基于 `data.total` 计算总页数：
   - `/tmp/PixivBatchDownloader/src/ts/crawlArtworkPage/InitSearchArtworkPage.ts`
   - 逻辑中有 `let pageCount = Math.ceil(data.total / this.worksNoPerPage)`

2. 明确有 preview 结果数展示：
   - 同文件中有 `settings.previewResult`
   - 会把抓到的数量回写到页面计数元素

3. 对搜索页存在明确上限控制：
   - 非会员最多 1000 页
   - 会员最多 5000 页

4. ranking 被明确当作 500 榜单处理：
   - `/tmp/PixivBatchDownloader/src/ts/crawlArtworkPage/InitRankingArtworkPage.ts`
   - 当用户设置“抓全部”时，直接把 `crawlNumber = 500`

结论：

- 主流 Pixiv 下载器并不回避“先拿总数再决定抓多少”
- 它甚至明确把 ranking 视为**500 个作品的有限集合**

### 3.2 PixivUtil2

仓库：

- [Nandaka/PixivUtil2](https://github.com/Nandaka/PixivUtil2)

关键证据：

1. tag 搜索：
   - `/tmp/PixivUtil2/model/PixivTags.py`
   - 直接读取 `payload["body"]["illustManga"]["total"]`

2. bookmark：
   - `/tmp/PixivUtil2/model/PixivBookmark.py`
   - 直接读取 `image_bookmark["body"]["total"]`

3. ranking：
   - `/tmp/PixivUtil2/model/PixivRanking.py`
   - 直接读取 `js_data["rank_total"]`

4. artist 全量作品：
   - `/tmp/PixivUtil2/model/PixivArtist.py`
   - 在 `profile/all` 场景下，直接以 `len(self.imageList)` 作为总量

结论：

- 这个老牌项目的处理方式，和本次调研得到的结论高度一致：
  - `keyword` 用接口 total
  - `bookmark` 用接口 total
  - `ranking` 用 rank_total
  - `user` 用全量 ID 数量

### 3.3 gallery-dl

仓库：

- [mikf/gallery-dl](https://github.com/mikf/gallery-dl)

它不是“交互式 UX 优先”的工具，因此没有明确 preview 设计；
但源码层面仍然把 `ranking` 视为独立榜单源，而不是无限滚动源：

- `/tmp/gallery-dl/gallery_dl/extractor/pixiv.py`
- `class PixivRankingExtractor`

它的产品取向更偏“持续提取直到接口停止返回”，而不是“在 CLI 里显式确认数量”。

这可以作为反例：

- gallery-dl 适合 datahoarder 风格
- picals-crawler 的目标用户不是这一路线

## 4. 每种下载模式如何获取作品数

本节只讨论除 `illust` 之外的四种模式。

### 4.1 `user`

接口：

- `GET /ajax/user/{user_id}/profile/all?lang=zh`

取数方式：

- 遍历 `body.illusts` 的数字 key
- 遍历 `body.manga` 的数字 key
- 去重后计数

请求成本：

- 1 次请求

性能特点：

- 响应体会随作者作品数增大，但仍然属于“单请求预览”
- 不需要先访问每张作品详情页

推荐程度：

- 高

### 4.2 `keyword`

接口：

- `GET /ajax/search/artworks/{keyword}?word=...&order=...&mode=...&p=1&s_mode=s_tag&type=all&lang=zh`

取数方式：

- 直接读取 `body.illustManga.total`

请求成本：

- 1 次请求

性能特点：

- 成本最低
- 非常适合在交互前先展示总量

推荐程度：

- 很高

### 4.3 `bookmark`

接口：

- `GET /ajax/user/{user_id}/illusts/bookmarks?tag=&offset=0&limit=1..48&rest=show&lang=zh`

取数方式：

- 直接读取 `body.total`

请求成本：

- 1 次请求

性能特点：

- 计数本身很便宜
- 但必须依赖登录态

推荐程度：

- 高

### 4.4 `ranking`

接口：

- `GET /ranking.php?mode={mode}&p=1&format=json`

取数方式：

- 直接读取 `rank_total`

请求成本：

- 1 次请求

性能特点：

- 最适合做“有限榜单确认”
- 不需要先翻到最后一页
- 不应把逐页 `contents.len` 的累加值当成权威总量，因为匿名态下可能出现少于 `rank_total` 的情况

推荐程度：

- 很高

## 5. 是否“所有模式都适合预览”

答案是：**适合，但实现方式应分模式**。

### 5.1 适合预览的模式

- `user`
- `keyword`
- `bookmark`
- `ranking`

### 5.2 不需要预览的模式

- `illust`

原因：

- `illust` 的目标是单作品，不存在“会不会一口气下 10000+”的问题。

### 5.3 为什么不能一刀切实现

因为四种批量模式的“总数来源”不同：

- `user`：数 key
- `keyword`：读 `body.illustManga.total`
- `bookmark`：读 `body.total`
- `ranking`：读 `rank_total`

如果强行统一成“先把所有作品 ID 拉完再计数”，会带来两个问题：

1. 对 `keyword` / `ranking` 明显多做无用请求
2. 对大数量关键词会把“预览”本身做得很重

正确做法应该是：

- 先走“最低成本总量探针”
- 再根据用户选择的数量，进入正式收集与下载

## 6. ranking 到底是有限还是无限

本次调研给出的结论是：

- **ranking 是有限榜单，不是无限列表。**

证据链：

1. Pixiv 真实接口返回 `rank_total`
2. `daily` 第 11 页直接 `404`
3. `daily` 第 10 页 `next = false`
4. 主流项目 `PixivBatchDownloader` 把“抓全部 ranking”直接写成 `500`
5. `PixivUtil2` 也把 `rank_total` 视为一等字段

补充：

- “有限”不代表所有 mode 都恒等于 500
- 目前至少 `monthly` 就实测为 479
- 所以后续实现不应把 500 写死在业务逻辑里，而应以 `rank_total` 为准

## 7. 对后续实现的直接约束

基于本次调研，后续如果实现 UX 优化，建议遵守以下约束：

1. `user` 预览：
   - 不新增额外接口
   - 直接复用 `/profile/all`
   - 总量来自 `illusts + manga` key 数

2. `keyword` 预览：
   - 只请求第 1 页
   - 直接读 `body.illustManga.total`
   - 不要为了预览去抓完全部页

3. `bookmark` 预览：
   - 只请求第 1 页
   - 直接读 `body.total`
   - 必须保证凭据已加载

4. `ranking` 预览：
   - 只请求第 1 页
   - 直接读 `rank_total`
   - 不要把 `500` 写死，除非只是 UI 文案 fallback

5. 排序选择：
   - `user` / `keyword` / `bookmark` 可以继续暴露“新到旧 / 旧到新”
   - `ranking` 是否允许重新排序，应由产品决定；从数据语义上看，它天然有自己的榜单顺序

## 8. 最终结论

本次调研已经足够支持后续设计与实现：

- 批量模式预览是可行的，而且可以做得很轻。
- 最低成本的方案不是“先把所有 ID 都拉下来”，而是按模式读取最便宜的总数字段或结构：
  - `user` -> `profile/all` 的 ID key 数
  - `keyword` -> `body.illustManga.total`
  - `bookmark` -> `body.total`
  - `ranking` -> `rank_total`
- Pixiv ranking 应明确视为**有限榜单**，不是无限流。
- 后续 ranking UX 的产品设计，应围绕“有限榜单确认”而不是“无限抓取保护”展开。
