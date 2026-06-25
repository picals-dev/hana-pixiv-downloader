//! picals-crawler 的共享库入口。
//!
//! 对外只暴露 CLI 入口与命令分发；其余模块均为 crate 内部实现，
//! 仅按测试需要选择性放开可见性。

#![warn(unreachable_pub)]

pub mod auth;
pub mod cli;
pub mod commands;
pub mod config;
pub mod crawler;
pub mod downloader;
pub mod error;
pub mod failure;
pub mod net;
pub(crate) mod output;
pub mod pixiv;
pub mod replay;
#[doc(hidden)]
pub mod test_support;
pub(crate) mod utils;

pub(crate) const PIXIV_BASE_URL: &str = "https://www.pixiv.net";
