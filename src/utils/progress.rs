//! 进度条封装。

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use indicatif::{ProgressBar, ProgressStyle};

const PROGRESS_BAR_TEMPLATE: &str =
    "[{elapsed_precise}] {bar:20.cyan/blue} 总图片 {pos}/{len} | {wide_msg}";
const ORGANIZE_PROGRESS_BAR_TEMPLATE: &str =
    "[{elapsed_precise}] {bar:20.cyan/blue} 总作品 {pos}/{len} | {wide_msg}";
const PROGRESS_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub(crate) struct DownloadProgress {
    bar: ProgressBar,
    state: Arc<Mutex<ProgressState>>,
    stop_refresh: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(crate) struct OrganizeProgress {
    bar: ProgressBar,
    state: Arc<Mutex<OrganizeProgressState>>,
}

#[derive(Debug)]
struct ProgressState {
    start: Instant,
    downloaded_bytes: u64,
    pending_window_bytes: u64,
    bytes_per_second: u64,
    last_speed_sample_at: Instant,
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

#[derive(Debug, Default)]
struct OrganizeProgressState {
    moved_files: u64,
    skipped_files: u64,
    conflicts: u64,
    unknown_files: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProgressSnapshot {
    pub downloaded_bytes: u64,
    pub bytes_per_second: u64,
    pub eta_seconds: Option<u64>,
    pub total_illusts: u64,
    pub handled_illusts: u64,
    pub successful_illusts: u64,
}

impl DownloadProgress {
    pub(crate) fn new(total_units: u64, illust_unit_totals: Vec<(String, u64)>) -> Self {
        let bar = ProgressBar::new(total_units);
        let style = ProgressStyle::with_template(PROGRESS_BAR_TEMPLATE)
            .expect("进度条模板必须有效")
            .progress_chars("=> ");

        bar.set_style(style);
        let state = ProgressState::new(total_units, illust_unit_totals);
        let initial_snapshot = ProgressSnapshot::from_state(&state, 0);
        bar.set_message(render_snapshot_message(initial_snapshot));
        let progress = Self {
            bar,
            state: Arc::new(Mutex::new(state)),
            stop_refresh: Arc::new(AtomicBool::new(false)),
        };
        progress.spawn_message_renderer();
        progress
    }

    pub(crate) fn record_downloaded_bytes(&self, bytes: u64) {
        if bytes == 0 {
            return;
        }

        let mut state = self.state.lock().expect("progress state lock poisoned");
        state.record_downloaded_bytes(bytes);
    }

    pub(crate) fn record_unit_completion(&self, illust_id: &str, failed: bool) {
        {
            let mut state = self.state.lock().expect("progress state lock poisoned");
            state.record_illust_progress(illust_id, failed);
        }
        self.bar.inc(1);
    }

    #[cfg(test)]
    fn snapshot(&self) -> ProgressSnapshot {
        let mut state = self.state.lock().expect("progress state lock poisoned");
        state.snapshot(self.bar.position(), Instant::now())
    }

    pub(crate) fn finish_with_message(&self, message: impl Into<String>) {
        self.stop_refresh.store(true, Ordering::Relaxed);
        self.bar.finish_with_message(message.into());
    }

    fn spawn_message_renderer(&self) {
        let bar = self.bar.clone();
        let state = Arc::clone(&self.state);
        let stop_refresh = Arc::clone(&self.stop_refresh);

        thread::Builder::new()
            .name("hpd-progress".to_string())
            .spawn(move || {
                loop {
                    if stop_refresh.load(Ordering::Relaxed) || Arc::strong_count(&state) == 1 {
                        break;
                    }

                    thread::sleep(PROGRESS_REFRESH_INTERVAL);

                    if stop_refresh.load(Ordering::Relaxed) || Arc::strong_count(&state) == 1 {
                        break;
                    }

                    let snapshot = {
                        let mut state = state.lock().expect("progress state lock poisoned");
                        state.snapshot(bar.position(), Instant::now())
                    };

                    if stop_refresh.load(Ordering::Relaxed) {
                        break;
                    }

                    bar.set_message(render_snapshot_message(snapshot));
                }
            })
            .expect("进度刷新线程必须可以启动");
    }
}

impl OrganizeProgress {
    pub(crate) fn new(total_artworks: u64, unknown_files: u64) -> Self {
        let bar = ProgressBar::new(total_artworks);
        let style = ProgressStyle::with_template(ORGANIZE_PROGRESS_BAR_TEMPLATE)
            .expect("整理进度条模板必须有效")
            .progress_chars("=> ");
        bar.set_style(style);

        let state = OrganizeProgressState {
            unknown_files,
            ..OrganizeProgressState::default()
        };
        bar.set_message(render_organize_snapshot(&state));

        Self {
            bar,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub(crate) fn record_artwork(&self, moved_files: u64, skipped_files: u64, conflicts: u64) {
        self.bar.inc(1);
        let message = {
            let mut state = self
                .state
                .lock()
                .expect("organize progress state lock poisoned");
            state.moved_files += moved_files;
            state.skipped_files += skipped_files;
            state.conflicts += conflicts;
            render_organize_snapshot(&state)
        };
        self.bar.set_message(message);
    }

    pub(crate) fn finish_with_message(&self, message: impl Into<String>) {
        self.bar.finish_with_message(message.into());
    }
}

impl ProgressState {
    fn new(total_units: u64, illust_unit_totals: Vec<(String, u64)>) -> Self {
        Self::new_at(total_units, illust_unit_totals, Instant::now())
    }

    fn new_at(total_units: u64, illust_unit_totals: Vec<(String, u64)>, start: Instant) -> Self {
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
            start,
            downloaded_bytes: 0,
            pending_window_bytes: 0,
            bytes_per_second: 0,
            last_speed_sample_at: start,
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

    fn record_downloaded_bytes(&mut self, bytes: u64) {
        self.downloaded_bytes += bytes;
        self.pending_window_bytes += bytes;
    }

    fn snapshot(&mut self, completed_units: u64, now: Instant) -> ProgressSnapshot {
        self.refresh_speed(now);
        ProgressSnapshot::from_state(self, completed_units)
    }

    fn refresh_speed(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_speed_sample_at);
        if elapsed < PROGRESS_REFRESH_INTERVAL {
            return;
        }

        let elapsed_seconds = elapsed.as_secs_f64().max(0.001);
        self.bytes_per_second = (self.pending_window_bytes as f64 / elapsed_seconds).round() as u64;
        self.pending_window_bytes = 0;
        self.last_speed_sample_at = now;
    }
}

impl ProgressSnapshot {
    fn from_state(state: &ProgressState, completed_units: u64) -> Self {
        let elapsed_seconds = state.start.elapsed().as_secs_f64().max(0.001);
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
            bytes_per_second: state.bytes_per_second,
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
        "已传输 {} | 速度 {}/s",
        format_bytes(snapshot.downloaded_bytes),
        format_bytes(snapshot.bytes_per_second)
    ));

    if let Some(eta) = snapshot.eta_seconds {
        parts.push(format!("预计剩余 {}", format_duration(eta)));
    }

    parts.join(" | ")
}

fn render_organize_snapshot(snapshot: &OrganizeProgressState) -> String {
    format!(
        "已移动文件 {} | 已跳过 {} | 冲突 {} | 未识别 {}",
        snapshot.moved_files, snapshot.skipped_files, snapshot.conflicts, snapshot.unknown_files
    )
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    match (hours, minutes, secs) {
        (0, 0, secs) => format!("{secs} 秒"),
        (0, minutes, 0) => format!("{minutes} 分钟"),
        (0, minutes, secs) => format!("{minutes} 分 {secs} 秒"),
        (hours, 0, 0) => format!("{hours} 小时"),
        (hours, 0, secs) => format!("{hours} 小时 {secs} 秒"),
        (hours, minutes, 0) => format!("{hours} 小时 {minutes} 分钟"),
        (hours, minutes, secs) => format!("{hours} 小时 {minutes} 分 {secs} 秒"),
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
    use std::time::{Duration, Instant};

    use super::{DownloadProgress, ProgressState, render_snapshot_message};

    #[test]
    fn progress_snapshot_reports_recent_speed_and_eta() {
        let start = Instant::now();
        let mut state = ProgressState::new_at(4, vec![("123456".to_string(), 4)], start);
        state.record_downloaded_bytes(2048);
        let snapshot = state.snapshot(1, start + Duration::from_secs(1));

        assert_eq!(snapshot.downloaded_bytes, 2048);
        assert_eq!(snapshot.bytes_per_second, 2048);
        assert!(snapshot.eta_seconds.is_some());
    }

    #[test]
    fn progress_snapshot_resets_speed_after_idle_window() {
        let start = Instant::now();
        let mut state = ProgressState::new_at(4, vec![("123456".to_string(), 4)], start);
        state.record_downloaded_bytes(2048);
        let _ = state.snapshot(1, start + Duration::from_secs(1));
        let snapshot = state.snapshot(1, start + Duration::from_secs(2));

        assert_eq!(snapshot.bytes_per_second, 0);
    }

    #[test]
    fn progress_snapshot_tracks_handled_and_successful_illusts() {
        let progress =
            DownloadProgress::new(3, vec![("100".to_string(), 2), ("200".to_string(), 1)]);

        progress.record_downloaded_bytes(1024);
        progress.record_unit_completion("100", false);
        let snapshot = progress.snapshot();
        assert_eq!(snapshot.handled_illusts, 0);
        assert_eq!(snapshot.successful_illusts, 0);

        progress.record_unit_completion("100", true);
        let snapshot = progress.snapshot();
        assert_eq!(snapshot.handled_illusts, 1);
        assert_eq!(snapshot.successful_illusts, 0);

        progress.record_downloaded_bytes(512);
        progress.record_unit_completion("200", false);
        let snapshot = progress.snapshot();
        assert_eq!(snapshot.handled_illusts, 2);
        assert_eq!(snapshot.successful_illusts, 1);
    }

    #[test]
    fn snapshot_message_includes_illust_summary() {
        let progress =
            DownloadProgress::new(2, vec![("100".to_string(), 1), ("200".to_string(), 1)]);

        let message = render_snapshot_message(progress.snapshot());
        assert!(message.contains("已处理作品 0/2"));
        assert!(message.contains("成功完成作品 0/2"));
        assert!(message.contains("已传输 0B"));
    }

    #[test]
    fn snapshot_message_renders_eta_in_plain_chinese() {
        let message = render_snapshot_message(super::ProgressSnapshot {
            downloaded_bytes: 1024,
            bytes_per_second: 512,
            eta_seconds: Some(195),
            total_illusts: 1,
            handled_illusts: 0,
            successful_illusts: 0,
        });

        assert!(message.contains("预计剩余 3 分 15 秒"));
    }

    #[test]
    fn progress_template_labels_total_images() {
        assert!(super::PROGRESS_BAR_TEMPLATE.contains("总图片 {pos}/{len}"));
    }

    #[test]
    fn progress_templates_truncate_messages_to_terminal_width() {
        assert!(super::PROGRESS_BAR_TEMPLATE.contains("{wide_msg}"));
        assert!(!super::PROGRESS_BAR_TEMPLATE.contains("{msg}"));
        assert!(super::ORGANIZE_PROGRESS_BAR_TEMPLATE.contains("{wide_msg}"));
        assert!(!super::ORGANIZE_PROGRESS_BAR_TEMPLATE.contains("{msg}"));
    }

    #[test]
    fn progress_templates_keep_fixed_content_compact() {
        assert!(super::PROGRESS_BAR_TEMPLATE.contains("{bar:20.cyan/blue}"));
        assert!(super::ORGANIZE_PROGRESS_BAR_TEMPLATE.contains("{bar:20.cyan/blue}"));
    }
}
