---
title: "TypeScript 原项目实现观察"
tags: ["typescript", "migration", "crawler", "observations"]
created: 2026-06-17T03:16:44.818Z
updated: 2026-06-17T03:16:44.818Z
sources: []
links: []
category: reference
confidence: medium
schemaVersion: 1
---

# TypeScript 原项目实现观察

## 真实实现链路
- TS 原项目通过 `UserCrawler` / `KeywordCrawler` / `BookmarkCrawler` 先收集 artwork id，再由 `Collector` 请求 `/ajax/illust/{id}/pages` 汇总原图 URL，最后由 `Downloader` 并发下载。
- 标签抓取没有走 Ajax JSON，而是对 `/artworks/{id}` 页面做 HTML 解析，从 `#meta-preload-data` 里抽取 tags，这也是技术设计文档准备删除 cheerio/scraper 的直接动机。

## 配置模型
- 项目通过 `debugConfig` / `downloadConfig` / `networkConfig` / `userConfig` 四个全局单例对象驱动行为。
- 这适合库调用，但不适合开箱即用 CLI，因为 CLI 需要稳定的配置优先级、持久化与交互式 setup。

## 现存问题与迁移启示
- `KeywordCrawler` 中 `order=${this.order ? "popular_d" : "date_d"}` 这一段逻辑明显可疑，`this.order` 只要非空就会固定走 `popular_d`。
- `BookmarkCrawler` 在分页循环里重复构建并执行 `Array.from(urls)`，会让前面页重复被请求。
- `Downloader` 使用共享可变 `downloadTraffic` 进行并发累加，语义上可工作，但不够严谨。
- Rust 重写不应机械复刻这些实现细节，而应保留“两阶段采集 + 并发下载 + 可选 tags 输出”的主链路，同时借机会修掉状态模型与分页/排序瑕疵。

