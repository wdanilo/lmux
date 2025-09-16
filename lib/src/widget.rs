use crate::prelude::*;
use crossterm::style::Stylize;

// ===============
// === spinner ===
// ===============

pub fn spinner(n: usize, i: usize) -> String {
    let prefix = " ".repeat(i);
    let suffix = " ".repeat(n.saturating_sub(i + 1));
    let marker = "█".green();
    format!("{prefix}{marker}{suffix}").bold().on_grey().to_string()
}

// ====================
// === progress_bar ===
// ====================

pub fn progress_bar(len: usize, progress: f32) -> String {
    const SYMBOL: &[char] = &[' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
    let fill_f = (len as f32) * progress;
    let fill_full = fill_f.floor() as usize;
    let fill_partial = fill_f.fract();
    let fill_full_str = "█".repeat(fill_full);
    let fill_partial_str = if fill_partial != 0.0 && fill_full < len {
        let symbol_index = (fill_partial * (SYMBOL.len() - 1) as f32).round() as usize;
        SYMBOL[symbol_index]
    } else {
        default()
    };
    let suffix = " ".repeat(len.saturating_sub(fill_f.ceil() as usize));
    format!("{fill_full_str}{fill_partial_str}{suffix}").on_grey().to_string()
}
