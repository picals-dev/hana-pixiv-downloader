//! picals-crawler 的共享库入口。
//!
//! 当前阶段先把 phase-1 需要的 Rust 基建与目录骨架搭起来，
//! 让后续可以直接往真实下载链路里填实现。

pub mod auth;
pub mod cli;
pub mod collector;
pub mod commands;
pub mod config;
pub mod crawler;
pub mod downloader;
pub mod error;
pub mod failure;
pub mod net;
pub mod output;
pub mod replay;
pub mod test_support;
pub mod utils;

pub const APP_NAME: &str = "picals-crawler";
pub const PIXIV_BASE_URL: &str = "https://www.pixiv.net";
