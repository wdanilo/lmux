use crate::prelude::*;

use std::time::SystemTime;
use crossterm::style::Stylize;

use crate::group;
use crate::widget;
use crate::index_to_group_char;
use crate::group::Group;
use crate::LineRange;

// ================
// === Duration ===
// ================

fn format_duration(total_ms: u128, show_ms: bool) -> String {
    let total_seconds = total_ms / 1000;
    let ms = total_ms % 1000;
    let s = total_seconds % 60;
    let m = (total_seconds / 60) % 60;
    let h = (total_seconds / 3600) % 24;
    let d = total_seconds / 86400;

    let mut parts = Vec::new();
    if d > 0 { parts.push(format!("{d}d")) }
    if h > 0 { parts.push(format!("{h}h")) }
    if m > 0 { parts.push(format!("{m}m")) }
    parts.push(format!("{s}s"));
    if show_ms && ms > 0 && d == 0 {
        parts.push(format!("{ms}ms"));
    }
    parts.join(" ")
}

pub trait Style: Send + Sync {
    fn header(&mut self, group: &LineRange<&'_ Group>, group_index: group::Id, s: &str) -> String;
    fn log_line(&mut self, group: &LineRange<&'_ Group>, group_index: group::Id, s: &str) -> String;
    fn footer(&mut self, group: &LineRange<&'_ Group>, group_index: group::Id, s: &str) -> String;
}

// ===========
// === Any ===
// ===========

#[derive(Deref, DerefMut)]
pub struct Any {
    style: Box<dyn Style>,
}

impl Debug for Any {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Any").finish()
    }
}

impl Default for Any {
    fn default() -> Self {
        Self { style: Box::new(DefaultStyle) }
    }
}

// ====================
// === DefaultStyle ===
// ====================

#[derive(Clone, Copy, Debug)]
pub struct DefaultStyle;

impl Style for DefaultStyle {
    fn header(&mut self, group: &LineRange<&'_ Group>, group_index: group::Id, s: &str) -> String {
        let progress_bar_len = 10;
        let state = group.state();
        let last_line = state.view_lines().last();
        let progress = last_line.and_then(|t| t.log.status.progress);
        let finished = last_line.map(|t| t.log.status.is_finished()).unwrap_or_default();
        let progress_bar = match (progress, finished) {
            (Some(progress), _) =>
                Self::header_style(group, &widget::progress_bar(progress_bar_len, progress)),
            (_, true) =>
                Self::header_style(group, &widget::progress_bar(progress_bar_len, 1.0)),
            _ => {
                let time = group.next_line.map(|t| t.0 % progress_bar_len).unwrap_or_else(|| {
                    let now = SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_millis();
                    ((now / 100) % progress_bar_len as u128) as usize
                });
                widget::spinner(progress_bar_len, time)
            }
        };
        let label = index_to_group_char(group_index.0).unwrap_or('…');
        let index = Self::border_style(group, &format!("[{label}]"));
        let border = Self::border_top_left(group);
        let content = Self::header_style(group, s);
        format!("{border} {index} {progress_bar} {content}")
    }

    fn log_line(&mut self, group: &LineRange<&'_ Group>, _group_index: group::Id, s: &str) -> String {
        let border = Self::border_left(group);
        format!("{border} {s}")
    }

    fn footer(&mut self, group: &LineRange<&'_ Group>, _group_index: group::Id, s: &str) -> String {
        let state = group.state();
        let lines = state.view_lines();
        let border = lines.first().zip(lines.last());
        let ms = if let Some((start, line_end)) = border.map(|(a, b)| (a.time, b.time)) {
            let history_view = group.next_line.is_some();
            let finished = lines.last().map(|t| t.log.status.is_finished()).unwrap_or_default();
            let end = if history_view || finished { line_end } else { SystemTime::now() };
            let duration = end.duration_since(start).unwrap_or_default();
            duration.as_millis()
        } else {
            0
        };
        let is_finished = group.state().view_lines().last().map(|t| t.log.status.is_finished())
            .unwrap_or_default();
        let is_history_view = group.next_line.is_some();
        let show_ms = is_finished || is_history_view;

        let status = format_duration(ms, show_ms);
        let border = Self::border_bottom_left(group);
        let status = Self::border_style(group, &status);
        format!("{border} {status} {s}")
    }
}

impl DefaultStyle {
    fn is_newest_output(group: &LineRange<&'_ Group>) -> bool {
        group.state().view_lines().last().zip(group.next_line).map(|(line, rage)| {
            line.timestamp.0 == rage.0 - 1
        }).unwrap_or_default()
    }

    fn header_style(group: &LineRange<&'_ Group>, s: &str) -> String {
        if group.state().view_lines().last().map(|t| t.log.status.is_error()).unwrap_or_default() {
            s.red().bold().to_string()
        } else {
            s.green().bold().to_string()
        }
    }

    fn left_padding_style(group: &LineRange<&'_ Group>) -> String {
        if Self::is_newest_output(group) {
            "▍".green().to_string()
        } else {
            " ".to_string()
        }
    }

    fn border_style(group: &LineRange<&'_ Group>, border: &str) -> String {
        if group.selected {
            border.white().bold().to_string()
        } else if group.state().view_lines().last().map(|t| t.log.status.is_error())
            .unwrap_or_default() {
            border.red().bold().to_string()
        } else {
            border.grey().bold().to_string()
        }
    }

    fn border_top_left(group: &LineRange<&'_ Group>) -> String {
        let padding = Self::left_padding_style(group);
        let border = Self::border_style(group, if group.is_collapsed() { "▶" } else { "▼" });
        format!("{padding}{border}")
    }

    fn border_left(group: &LineRange<&'_ Group>) -> String {
        let padding = Self::left_padding_style(group);
        let border = Self::border_style(group, "│");
        format!("{padding}{border}")
    }

    fn border_bottom_left(group: &LineRange<&'_ Group>) -> String {
        let padding = Self::left_padding_style(group);
        let border = Self::border_style(group, "╰");
        format!("{padding}{border}")
    }
}
