---
title: "Picals Crawler 产品设计附录：交互与路线图"
tags: ["appendix", "interaction", "roadmap"]
created: 2026-06-23T09:25:00.000Z
updated: 2026-06-23T09:25:00.000Z
sources: []
links: ["picals-crawler-产品设计文档.md", "picals-crawler-ux-优化执行记录-2026-06-21.md"]
category: reference
confidence: high
schemaVersion: 1
---

# Picals Crawler 产品设计附录：交互与路线图

## 交互样例

### `setup`

- 引导用户获取 `PHPSESSID`
- 自动从登录态响应头或 HTML 提取 `userId`
- 逐项确认 `download.roots.*`
- 逐项确认下载参数与代理
- 最终明文摘要确认

### 日常下载

- `download user <id|url>` 仍是第一优先入口
- 下载过程中持续显示进度、速度、ETA
- 失败后优先 auto-replay，再写 manifest 供 `retry` 使用

## 断点续传体验

- 已完成文件自动跳过
- 未完成文件重新下载
- 用户不需要记住上次中断位置

## 版本路线图摘要

- `v0.1.0`：setup、download user、基础进度条、跨平台构建
- `v0.2.0`：keyword / ranking / illust / bookmark、config、retry 闭环
- `v0.3.x`：README、安装分发、发布准备
- `v1.0.0`：稳定安装路径与正式发布节奏

## 关联页面

- 主文档：[[Picals Crawler 产品设计文档]]
- UX 执行记录：[[Picals Crawler UX 优化执行记录 2026-06-21]]
