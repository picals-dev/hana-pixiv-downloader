---
title: "Picals Crawler 下载预览与 Ranking 调研报告 2026-06-21"
tags: ["research", "ranking", "keyword", "bookmark", "user"]
created: 2026-06-21T16:10:00.000Z
updated: 2026-06-23T09:19:53.000Z
sources:
  - "src/crawler/user.rs"
  - "src/crawler/bookmark.rs"
  - "src/crawler/keyword.rs"
  - "src/crawler/ranking.rs"
  - "src/pixiv/selector.rs"
  - "https://www.pixiv.net/ajax/search/artworks/%E5%8E%9F%E7%A5%9E?word=%E5%8E%9F%E7%A5%9E&order=date_d&mode=safe&p=1&s_mode=s_tag&type=all&lang=zh"
  - "https://www.pixiv.net/ajax/user/11/profile/all?lang=zh"
  - "https://www.pixiv.net/ranking.php?format=json&mode=daily&p=1"
  - "https://github.com/xuejianxianzun/PixivBatchDownloader"
  - "https://github.com/Nandaka/PixivUtil2"
  - "https://github.com/mikf/gallery-dl"
  - "https://github.com/daydreamer-json/pixiv-ajax-api-docs"
links:
  - "picals-crawler-产品设计文档.md"
  - "picals-crawler-技术设计文档.md"
  - "picals-crawler-项目理解基线.md"
  - "picals-crawler-ux-优化执行记录-2026-06-21.md"
  - "picals-crawler-下载预览与-ranking-调研附录-实验与竞品.md"
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

本轮实验的稳定结论已经足够清晰：

- `keyword`：一次请求直接读取 `body.illustManga.total`
- `user`：一次请求统计 `body.illusts + body.manga` 的作品 ID 数量
- `bookmark`：需要登录态，但协议层有 `body.total`
- `ranking`：有限榜单，应以 `rank_total` 为准

详细实验样本、请求耗时与返回体观察已移动到附录：[[Picals Crawler 下载预览与 Ranking 调研附录：实验与竞品]]

## 3. 主流项目证据

竞品证据的稳定结论也已足够明确：

- Powerful Pixiv Downloader：直接做 preview，并把 ranking 当有限榜单
- PixivUtil2：`keyword/bookmark/ranking/user` 四类总量读取方式与本报告结论一致
- gallery-dl：更偏持续抓取，不适合作为本项目的 UX 交互模板

详细竞品对照与源码证据已移动到附录：[[Picals Crawler 下载预览与 Ranking 调研附录：实验与竞品]]

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
