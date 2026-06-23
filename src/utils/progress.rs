//! 进度条封装。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use indicatif::{ProgressBar, ProgressStyle};

const PROGRESS_BAR_TEMPLATE: &str =
    "[{elapsed_precise}] {bar:40.cyan/blue} 总图片 | {pos}/{len} | {msg}";

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
    illusts: HashMap<String, IllustProgressState>,
    total_illusts: u64,
    handled_illusts: u64,
    successful_illusts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IllustProgressState {
    total_units: u64,
    completed_units: u64,
    has_failure: bool,
    handled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressSnapshot {
    pub downloaded_bytes: u64,
    pub bytes_per_second: u64,
    pub eta_seconds: Option<u64>,
    pub total_illusts: u64,
    pub handled_illusts: u64,
    pub successful_illusts: u64,
}

impl DownloadProgress {
    pub fn new(total_units: u64, illust_unit_totals: Vec<(String, u64)>) -> Self {
        let bar = ProgressBar::new(total_units);
        let style = ProgressStyle::with_template(PROGRESS_BAR_TEMPLATE)
            .expect("进度条模板必须有效")
            .progress_chars("=> ");

        bar.set_style(style);
        let state = ProgressState::new(total_units, illust_unit_totals);
        let initial_snapshot = ProgressSnapshot::from_state(&state, 0);
        bar.set_message(render_snapshot_message(initial_snapshot));

        Self {
            bar,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn inc(&self, delta: u64) {
        self.bar.inc(delta);
    }

    pub fn record_unit_completion(&self, illust_id: &str, bytes: u64, failed: bool) {
        self.bar.inc(1);
        let snapshot = {
            let mut state = self.state.lock().expect("progress state lock poisoned");
            state.downloaded_bytes += bytes;
            state.record_illust_progress(illust_id, failed);
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

impl ProgressState {
    fn new(total_units: u64, illust_unit_totals: Vec<(String, u64)>) -> Self {
        let total_illusts = illust_unit_totals.len() as u64;
        let illusts = illust_unit_totals
            .into_iter()
            .map(|(illust_id, total_units)| {
                (
                    illust_id,
                    IllustProgressState {
                        total_units,
                        completed_units: 0,
                        has_failure: false,
                        handled: false,
                    },
                )
            })
            .collect();

        Self {
            start: Instant::now(),
            downloaded_bytes: 0,
            total_units,
            illusts,
            total_illusts,
            handled_illusts: 0,
            successful_illusts: 0,
        }
    }

    fn record_illust_progress(&mut self, illust_id: &str, failed: bool) {
        let Some(illust) = self.illusts.get_mut(illust_id) else {
            return;
        };

        illust.completed_units += 1;
        illust.has_failure |= failed;

        if !illust.handled && illust.completed_units >= illust.total_units {
            illust.handled = true;
            self.handled_illusts += 1;
            if !illust.has_failure {
                self.successful_illusts += 1;
            }
        }
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
            total_illusts: state.total_illusts,
            handled_illusts: state.handled_illusts,
            successful_illusts: state.successful_illusts,
        }
    }
}

fn render_snapshot_message(snapshot: ProgressSnapshot) -> String {
    let mut parts = Vec::new();
    if snapshot.total_illusts > 0 {
        parts.push(format!(
            "已处理作品 {}/{} | 成功完成作品 {}/{}",
            snapshot.handled_illusts,
            snapshot.total_illusts,
            snapshot.successful_illusts,
            snapshot.total_illusts
        ));
    }

    parts.push(format!(
        "已下载 {} | 速度 {}/s",
        format_bytes(snapshot.downloaded_bytes),
        format_bytes(snapshot.bytes_per_second)
    ));

    if let Some(eta) = snapshot.eta_seconds {
        parts.push(format!("ETA {}s", eta));
    }

    parts.join(" | ")
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
        let progress = DownloadProgress::new(4, vec![("123456".to_string(), 4)]);
        progress.record_unit_completion("123456", 2048, false);
        let snapshot = progress.snapshot();

        assert_eq!(snapshot.downloaded_bytes, 2048);
        assert!(snapshot.bytes_per_second > 0);
        assert!(snapshot.eta_seconds.is_some());
    }

    #[test]
    fn progress_snapshot_tracks_handled_and_successful_illusts() {
        let progress =
            DownloadProgress::new(3, vec![("100".to_string(), 2), ("200".to_string(), 1)]);

        progress.record_unit_completion("100", 1024, false);
        let snapshot = progress.snapshot();
        assert_eq!(snapshot.handled_illusts, 0);
        assert_eq!(snapshot.successful_illusts, 0);

        progress.record_unit_completion("100", 0, true);
        let snapshot = progress.snapshot();
        assert_eq!(snapshot.handled_illusts, 1);
        assert_eq!(snapshot.successful_illusts, 0);

        progress.record_unit_completion("200", 512, false);
        let snapshot = progress.snapshot();
        assert_eq!(snapshot.handled_illusts, 2);
        assert_eq!(snapshot.successful_illusts, 1);
    }

    #[test]
    fn snapshot_message_includes_illust_summary() {
        let progress =
            DownloadProgress::new(2, vec![("100".to_string(), 1), ("200".to_string(), 1)]);

        let message = super::render_snapshot_message(progress.snapshot());
        assert!(message.contains("已处理作品 0/2"));
        assert!(message.contains("成功完成作品 0/2"));
        assert!(message.contains("已下载 0B"));
    }

    #[test]
    fn progress_template_labels_total_images() {
        assert!(super::PROGRESS_BAR_TEMPLATE.contains("总图片 | {pos}/{len}"));
    }
}
