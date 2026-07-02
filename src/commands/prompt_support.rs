//! 交互式表单辅助。

use std::fmt;

use eyre::{Report, eyre};
use inquire::{InquireError, Select, Text};

use crate::{
    config::{BatchLayoutStrategy, SortOrder},
    error::AppResult,
};

pub(crate) fn prompt_text(message: &str, help: &str, default: &str) -> AppResult<String> {
    let prompt = Text::new(message).with_help_message(help);
    let prompt = if default.trim().is_empty() {
        prompt
    } else {
        prompt.with_default(default)
    };

    let value = prompt.prompt().map_err(map_inquire_error)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(eyre!("{message} 不能为空"));
    }

    Ok(trimmed.to_string())
}

pub(crate) fn prompt_optional_text(message: &str, help: &str, default: &str) -> AppResult<String> {
    let prompt = Text::new(message).with_help_message(help);
    let prompt = if default.trim().is_empty() {
        prompt
    } else {
        prompt.with_default(default)
    };

    Ok(prompt
        .prompt()
        .map_err(map_inquire_error)?
        .trim()
        .to_string())
}

pub(crate) fn prompt_usize(message: &str, help: &str, default: usize) -> AppResult<usize> {
    let value = Text::new(message)
        .with_help_message(help)
        .with_default(&default.to_string())
        .prompt()
        .map_err(map_inquire_error)?;

    value
        .trim()
        .parse::<usize>()
        .map_err(|_| eyre!("{message} 需要无符号整数"))
}

pub(crate) fn prompt_u64(message: &str, help: &str, default: u64) -> AppResult<u64> {
    let value = Text::new(message)
        .with_help_message(help)
        .with_default(&default.to_string())
        .prompt()
        .map_err(map_inquire_error)?;

    value
        .trim()
        .parse::<u64>()
        .map_err(|_| eyre!("{message} 需要无符号整数"))
}

pub(crate) fn prompt_bool(message: &str, help: &str, default: bool) -> AppResult<bool> {
    let default_choice = BoolChoice::from(default);
    let options = vec![BoolChoice::Yes, BoolChoice::No];
    let selected = Select::new(message, options)
        .with_help_message(help)
        .with_starting_cursor(if default_choice == BoolChoice::Yes {
            0
        } else {
            1
        })
        .prompt()
        .map_err(map_inquire_error)?;

    Ok(bool::from(selected))
}

pub(crate) fn prompt_sort_order(
    message: &str,
    help: &str,
    default: SortOrder,
) -> AppResult<SortOrder> {
    let options = vec![SortChoice::DateDesc, SortChoice::DateAsc];
    let cursor = if default == SortOrder::DateAsc { 1 } else { 0 };
    let selected = Select::new(message, options)
        .with_help_message(help)
        .with_starting_cursor(cursor)
        .prompt()
        .map_err(map_inquire_error)?;

    Ok(SortOrder::from(selected))
}

pub(crate) fn prompt_batch_layout(
    message: &str,
    help: &str,
    default: BatchLayoutStrategy,
) -> AppResult<BatchLayoutStrategy> {
    let options = vec![
        BatchLayoutChoice::Mixed,
        BatchLayoutChoice::PerIllust,
        BatchLayoutChoice::Flat,
    ];
    let cursor = match default {
        BatchLayoutStrategy::Mixed => 0,
        BatchLayoutStrategy::PerIllust => 1,
        BatchLayoutStrategy::Flat => 2,
    };

    let selected = Select::new(message, options)
        .with_help_message(help)
        .with_starting_cursor(cursor)
        .prompt()
        .map_err(map_inquire_error)?;

    Ok(BatchLayoutStrategy::from(selected))
}

pub(crate) fn map_inquire_error(error: InquireError) -> Report {
    match error {
        InquireError::OperationCanceled | InquireError::OperationInterrupted => {
            eyre!("操作已取消")
        }
        other => Report::new(other).wrap_err("交互式输入失败"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoolChoice {
    Yes,
    No,
}

impl From<bool> for BoolChoice {
    fn from(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }
}

impl From<BoolChoice> for bool {
    fn from(value: BoolChoice) -> Self {
        matches!(value, BoolChoice::Yes)
    }
}

impl fmt::Display for BoolChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yes => write!(f, "是"),
            Self::No => write!(f, "否"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortChoice {
    DateDesc,
    DateAsc,
}

impl From<SortChoice> for SortOrder {
    fn from(value: SortChoice) -> Self {
        match value {
            SortChoice::DateDesc => SortOrder::DateDesc,
            SortChoice::DateAsc => SortOrder::DateAsc,
        }
    }
}

impl fmt::Display for SortChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DateDesc => write!(f, "date_desc（新的作品优先）"),
            Self::DateAsc => write!(f, "date_asc（旧的作品优先）"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchLayoutChoice {
    Mixed,
    PerIllust,
    Flat,
}

impl From<BatchLayoutChoice> for BatchLayoutStrategy {
    fn from(value: BatchLayoutChoice) -> Self {
        match value {
            BatchLayoutChoice::Mixed => BatchLayoutStrategy::Mixed,
            BatchLayoutChoice::PerIllust => BatchLayoutStrategy::PerIllust,
            BatchLayoutChoice::Flat => BatchLayoutStrategy::Flat,
        }
    }
}

impl fmt::Display for BatchLayoutChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mixed => write!(f, "mixed（单输出平铺，多输出作品分目录）"),
            Self::PerIllust => write!(f, "per_illust（所有作品都分目录）"),
            Self::Flat => write!(f, "flat（所有作品都直接平铺）"),
        }
    }
}
