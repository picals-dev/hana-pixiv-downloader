//! 进度条封装。

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use indicatif::{ProgressBar, ProgressStyle};

#[derive(Clone)]
pub struct DownloadProgress {
    bar: ProgressBar,
    state: Arc<Mutex<ProgressState>>,
}

#[derive(Debug)]
struct ProgressState {
    start: Instant,
    downloaded_bytes: u64,
    total_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressSnapshot {
    pub downloaded_bytes: u64,
    pub bytes_per_second: u64,
    pub eta_seconds: Option<u64>,
}

impl DownloadProgress {
    pub fn new(total_units: u64) -> Self {
        let bar = ProgressBar::new(total_units);
        let style = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}",
        )
        .expect("进度条模板必须有效")
        .progress_chars("=> ");

        bar.set_style(style);
        Self {
            bar,
            state: Arc::new(Mutex::new(ProgressState {
                start: Instant::now(),
                downloaded_bytes: 0,
                total_units,
            })),
        }
    }

    pub fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }

    pub fn record_downloaded_bytes(&self, bytes: u64) {
        let snapshot = {
            let mut state = self.state.lock().expect("progress state lock poisoned");
            state.downloaded_bytes += bytes;
            ProgressSnapshot::from_state(&state, self.bar.position())
        };
        self.bar.set_message(render_snapshot_message(snapshot));
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        let state = self.state.lock().expect("progress state lock poisoned");
        ProgressSnapshot::from_state(&state, self.bar.position())
    }

    pub fn set_message(&self, message: impl Into<String>) {
        self.bar.set_message(message.into());
    }

    pub fn finish_with_message(&self, message: impl Into<String>) {
        self.bar.finish_with_message(message.into());
    }
}

impl ProgressSnapshot {
    fn from_state(state: &ProgressState, completed_units: u64) -> Self {
        let elapsed_seconds = state.start.elapsed().as_secs_f64().max(0.001);
        let bytes_per_second = (state.downloaded_bytes as f64 / elapsed_seconds).round() as u64;
        let eta_seconds = if completed_units > 0 && completed_units < state.total_units {
            let average_unit_seconds = elapsed_seconds / completed_units as f64;
            Some(
                ((state.total_units - completed_units) as f64 * average_unit_seconds).ceil() as u64,
            )
        } else {
            None
        };

        Self {
            downloaded_bytes: state.downloaded_bytes,
            bytes_per_second,
            eta_seconds,
        }
    }
}

fn render_snapshot_message(snapshot: ProgressSnapshot) -> String {
    match snapshot.eta_seconds {
        Some(eta) => format!(
            "已下载 {} | 速度 {}/s | ETA {}s",
            format_bytes(snapshot.downloaded_bytes),
            format_bytes(snapshot.bytes_per_second.max(1)),
            eta
        ),
        None => format!(
            "已下载 {} | 速度 {}/s",
            format_bytes(snapshot.downloaded_bytes),
            format_bytes(snapshot.bytes_per_second.max(1))
        ),
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{}{}", bytes, UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::DownloadProgress;

    #[test]
    fn progress_snapshot_reports_speed_and_eta() {
        let progress = DownloadProgress::new(4);
        progress.inc(1);
        progress.record_downloaded_bytes(2048);
        let snapshot = progress.snapshot();

        assert_eq!(snapshot.downloaded_bytes, 2048);
        assert!(snapshot.bytes_per_second > 0);
        assert!(snapshot.eta_seconds.is_some());
    }
}
