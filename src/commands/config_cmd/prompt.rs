use super::super::prompt_support::{
    prompt_batch_layout, prompt_bool, prompt_optional_text, prompt_sort_order, prompt_text,
    prompt_u64, prompt_usize,
};
use crate::{
    config::{BatchLayoutStrategy, SortOrder},
    error::AppResult,
};

pub(in crate::commands::config_cmd) trait ConfigPrompter {
    fn prompt_text(&self, message: &str, help: &str, default: &str) -> AppResult<String>;
    fn prompt_optional_text(&self, message: &str, help: &str, default: &str) -> AppResult<String>;
    fn prompt_usize(&self, message: &str, help: &str, default: usize) -> AppResult<usize>;
    fn prompt_u64(&self, message: &str, help: &str, default: u64) -> AppResult<u64>;
    fn prompt_bool(&self, message: &str, help: &str, default: bool) -> AppResult<bool>;
    fn prompt_sort_order(
        &self,
        message: &str,
        help: &str,
        default: SortOrder,
    ) -> AppResult<SortOrder>;
    fn prompt_batch_layout(
        &self,
        message: &str,
        help: &str,
        default: BatchLayoutStrategy,
    ) -> AppResult<BatchLayoutStrategy>;
}

pub(in crate::commands::config_cmd) struct InteractiveConfigPrompter;

impl ConfigPrompter for InteractiveConfigPrompter {
    fn prompt_text(&self, message: &str, help: &str, default: &str) -> AppResult<String> {
        prompt_text(message, help, default)
    }

    fn prompt_optional_text(&self, message: &str, help: &str, default: &str) -> AppResult<String> {
        prompt_optional_text(message, help, default)
    }

    fn prompt_usize(&self, message: &str, help: &str, default: usize) -> AppResult<usize> {
        prompt_usize(message, help, default)
    }

    fn prompt_u64(&self, message: &str, help: &str, default: u64) -> AppResult<u64> {
        prompt_u64(message, help, default)
    }

    fn prompt_bool(&self, message: &str, help: &str, default: bool) -> AppResult<bool> {
        prompt_bool(message, help, default)
    }

    fn prompt_sort_order(
        &self,
        message: &str,
        help: &str,
        default: SortOrder,
    ) -> AppResult<SortOrder> {
        prompt_sort_order(message, help, default)
    }

    fn prompt_batch_layout(
        &self,
        message: &str,
        help: &str,
        default: BatchLayoutStrategy,
    ) -> AppResult<BatchLayoutStrategy> {
        prompt_batch_layout(message, help, default)
    }
}
