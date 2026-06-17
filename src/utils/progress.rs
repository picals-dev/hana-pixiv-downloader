//! 进度条封装。

use indicatif::{ProgressBar, ProgressStyle};

#[derive(Clone)]
pub struct DownloadProgress {
    bar: ProgressBar,
}

impl DownloadProgress {
    pub fn new(total: u64) -> Self {
        let bar = ProgressBar::new(total);
        let style = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}",
        )
        .expect("进度条模板必须有效")
        .progress_chars("=> ");

        bar.set_style(style);
        Self { bar }
    }

    pub fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }

    pub fn set_message(&self, message: impl Into<String>) {
        self.bar.set_message(message.into());
    }

    pub fn finish_with_message(&self, message: impl Into<String>) {
        self.bar.finish_with_message(message.into());
    }
}
