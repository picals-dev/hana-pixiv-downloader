---
title: "Picals Crawler 下载预览与 Ranking 调研附录：实验与竞品"
tags: ["appendix", "experiment", "competitor"]
created: 2026-06-23T09:25:00.000Z
updated: 2026-06-23T09:25:00.000Z
sources: []
links: ["picals-crawler-下载预览与-ranking-调研报告-2026-06-21.md"]
category: reference
confidence: high
schemaVersion: 1
---

# Picals Crawler 下载预览与 Ranking 调研附录：实验与竞品

## 实验摘要

### `keyword`

- 1 次请求即可读取 `body.illustManga.total`
- 成本最低，最适合做“预览总量 + 用户确认”

### `user`

- 1 次 `profile/all` 请求即可通过 `illusts + manga` 的数字 key 推导总数
- 不依赖额外 `total` 字段

### `bookmark`

- 在线实验依赖登录态
- 协议与竞品证据均表明首请求可直接读取 `body.total`

### `ranking`

- `daily / weekly / male / female` 实测 `rank_total = 500`
- `monthly` 实测 `rank_total = 479`
- `daily` 第 11 页直接 `404`
- 结论：ranking 是有限榜单，不是无限流

## 竞品证据摘要

### Powerful Pixiv Downloader

- 搜索页直接基于 total 计算页数
- 存在 preview 结果数展示
- ranking 直接按 500 榜处理

### PixivUtil2

- `keyword`：读 `illustManga.total`
- `bookmark`：读 `body.total`
- `ranking`：读 `rank_total`
- `user`：以全量作品 ID 数量为准

### gallery-dl

- 更偏持续提取器而不是交互式 UX 下载器
- 可作为反例，不适合作为本项目数量确认 UX 的直接模板

## 结论

- 本项目的数量预览设计有充分协议证据与竞品证据支撑。
- 真正需要区分的不是“能不能预览”，而是“每种模式应该以哪一个总量字段/推导方式为准”。

## 关联页面

- 主报告：[[Picals Crawler 下载预览与 Ranking 调研报告 2026-06-21]]
