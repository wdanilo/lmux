pub mod framebuffer;
pub mod group;
pub mod hash_tree;
pub mod prelude;
pub mod terminal;
pub mod style;
pub mod widget;

use crate::prelude::*;

use crate::hash_tree::HashTree;
use crossterm::style::Stylize;
use group::Group;
use std::time::SystemTime;

pub use group::Status;
pub use group::Log;

// ==============
// === LineId ===
// ==============

/// Global log line index, unique across all groups. It grows chronologically for each new logged
/// line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deref)]
pub struct LineId(pub usize);

impl LineId {
    pub fn inc(self) -> LineId {
        LineId(self.0 + 1)
    }
}

// =================
// === LineRange ===
// =================

#[derive(Debug, Default, Deref, DerefMut)]
pub struct LineRange<T> {
    #[deref]
    #[deref_mut]
    pub data: T,
    pub next_line: Option<LineId>,
}

impl<'t, T> LineRange<&'t mut T> {
    pub fn as_ref(&'t self) -> LineRange<&'t T> {
        LineRange { data: self.data, next_line: self.next_line }
    }
}

impl<T> LineRange<T> {
    pub fn map<U>(&self, f: impl FnOnce(&T) -> U) -> LineRange<U> {
        LineRange { data: f(&self.data), next_line: self.next_line }
    }
}

// ==============
// === Groups ===
// ==============

type Groups = LineRange<Vec<Group>>;

impl Groups {
    pub fn nonempty_mut(&mut self) -> Vec<LineRange<&'_ mut Group>> {
        self.data
            .iter_mut()
            .map(|data| LineRange { data, next_line: self.next_line })
            .filter(|g| !g.as_ref().state().view_lines().is_empty())
            .collect()
    }

    pub fn nonempty(&self) -> Vec<LineRange<&'_ Group>> {
        self.data
            .iter()
            .map(|data| LineRange { data, next_line: self.next_line })
            .filter(|g| !g.state().view_lines().is_empty())
            .collect()
    }
}

// ==============
// === Logger ===
// ==============

#[derive(Debug, Default)]
pub struct Logger {
    groups: Groups,
    path_to_group_id: HashTree<String, group::Id>,
    style: style::Any,
    next_line_id: LineId,
    frame_buffer: framebuffer::Framebuffer,
    debug_lines: Vec<String>,
    history: Vec<(group::Id, group::StatusTag)>,
}

impl Logger {
    fn next_line_id(&mut self) -> LineId {
        let line_id = self.next_line_id;
        self.next_line_id = line_id.inc();
        line_id
    }
}

impl Logger {
    pub fn create_group(&mut self, selector: &[String]) -> group::Id {
        *self.path_to_group_id.get_or_insert_with(selector, || {
            let group_index = self.groups.len();
            let group_id = group::Id(group_index);
            let mut group = Group::new(group_id);
            group.header = selector.join("::");
            self.groups.push(group);
            group_id
        })
    }

    pub fn group_mut(&mut self, selector: impl GroupSelector) -> Result<LineRange<&'_ mut Group>> {
        let next_line = self.groups.next_line;
        GroupSelector::group_id(selector, self).map(|id|
           LineRange { data: &mut self.groups[*id], next_line }
        )
    }

    pub fn push_line(&mut self, selector: impl GroupSelector, log: Log) -> Result {
        let group_id = GroupSelector::group_id(selector, self)?;
        let time = SystemTime::now();
        let timestamp = self.next_line_id();
        self.history.push((group_id, log.status.tag));
        let line = group::Line { timestamp, time, log };
        self.groups[*group_id].lines.push(line);
        Ok(())
    }

    pub fn get_last_line(&mut self, selector: impl GroupSelector) -> Result<Option<&Log>> {
        let group_id = GroupSelector::group_id(selector, self)?;
        Ok(self.groups[*group_id].lines.last().map(|l| &l.log))
    }

    pub fn shift_selection(&mut self, shift: isize) {
        let mut groups = self.groups.nonempty_mut();
        if !groups.is_empty() {
            let count = groups.len();
            let border_ix = group::Id(if shift >= 0 { 0 } else { count.saturating_sub(1) });
            let any_selected = groups.iter().any(|g| g.selected);
            if !any_selected {
                groups[*border_ix].selected = true;
            } else {
                let mut prev_selected = false;
                if shift < 0 { groups.reverse() };
                for group in &mut groups {
                    swap(&mut prev_selected, &mut group.selected);
                }
                if prev_selected {
                    groups[0].selected = true;
                }
            }
        }
    }

    pub fn shift_history(&mut self, shift: isize) {
        let max = LineId(self.history.len());
        let current = self.groups.next_line.unwrap_or(max);
        let new = LineId(((*current as isize + shift).max(0) as usize).min(*max));
        self.groups.next_line = if new == max { None } else { Some(new) };
    }

    pub fn scroll(&mut self, selector: impl GroupSelector, offset: isize) -> Result {
        let group_id = selector.group_id(self)?;
        let line_range = self.frame_buffer.group_to_group_lines.get(&group_id).copied();
        let group = &mut self.groups[*group_id];
        let line_count = line_range.map(|t| *t.1 - *t.0 + 1).unwrap_or_default();
        let max = group.lines.len().saturating_sub(line_count);
        let current_scroll = group.scroll.unwrap_or_else(|| *line_range.unwrap_or_default().0);
        let new_scroll = if offset > 0 {
            current_scroll.saturating_add(offset as usize).min(max)
        } else {
            current_scroll.saturating_sub((-offset) as usize)
        };
        group.scroll = (new_scroll != max).then_some(new_scroll);
        Ok(())
    }
}

// ====================
// === SharedLogger ===
// ====================

#[derive(Clone, Debug, Default, Deref)]
pub struct SharedLogger {
    arc: Arc<Mutex<Logger>>,
}

static LOGGER: OnceLock<SharedLogger> = OnceLock::new();

pub fn logger() -> &'static SharedLogger {
    LOGGER.get_or_init(SharedLogger::default)
}

// =====================
// === GroupSelector ===
// =====================

pub trait GroupSelector {
    fn group_id(self, logger: &mut Logger) -> Result<group::Id>;
}

impl GroupSelector for group::Id {
    fn group_id(self, logger: &mut Logger) -> Result<group::Id> {
        if self.0 >= logger.groups.len() {
            return Err(anyhow!("Group index out of bounds: {}", self.0));
        }
        Ok(self)
    }
}

impl GroupSelector for &[String] {
    fn group_id(self, logger: &mut Logger) -> Result<group::Id> {
        logger.path_to_group_id.get(self).copied()
            .with_context(|| format!("Group not found: '{}'", self.join(".")))
    }
}

impl<const N: usize> GroupSelector for &[String; N] {
    fn group_id(self, logger: &mut Logger) -> Result<group::Id> {
        let slice: &[String] = self;
        slice.group_id(logger)
    }
}


// ===========================
// === GroupStringSelector ===
// ===========================

pub trait GroupStringSelector {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T;
}

impl GroupStringSelector for &[String] {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(self)
    }
}

impl GroupStringSelector for &[&str] {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(&self.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
}

impl GroupStringSelector for &[&String] {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(&self.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
}

impl<const N: usize> GroupStringSelector for &[String; N] {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(self)
    }
}

impl<const N: usize> GroupStringSelector for &[&str; N] {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(&self.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
}

impl<const N: usize> GroupStringSelector for &[&String; N] {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(&self.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }
}

impl GroupStringSelector for &str {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(&[self.to_string()])
    }
}

impl GroupStringSelector for &String {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(&[self.to_string()])
    }
}

impl GroupStringSelector for String {
    fn with_selector<T>(self, f: impl FnOnce(&[String]) -> T) -> T {
        f(&[self])
    }
}

// ===========
// === API ===
// ===========

fn modify_logger<T>(f: impl FnOnce(&mut Logger) -> T) -> Result<T> {
    let mut logger = logger().lock().map_err(|e| anyhow!("Failed to lock logger: {}", e))?;
    Ok(f(&mut logger))
}

pub fn modify_all_groups(mut f: impl FnMut(LineRange<&'_ mut Group>)) -> Result {
    modify_logger(|logger| for group in logger.groups.nonempty_mut() { f(group); })
}

pub fn modify_group<T>(
    selector: impl GroupSelector,
    f: impl FnOnce(LineRange<&'_ mut Group>) -> T
) -> Result<T> {
    modify_logger(|l| l.group_mut(selector).map(f))?
}

pub fn push_line(selector: impl GroupSelector, log: Log) -> Result {
    modify_logger(|l| l.push_line(selector, log))?
}

pub fn set_group_header(selector: impl GroupSelector, s: impl Into<String>) -> Result {
    modify_group_header(selector, |h| *h = s.into())
}

pub fn modify_group_header<T>
(selector: impl GroupSelector, f: impl FnOnce(&mut String) -> T) -> Result<T> {
    modify_group(selector, |mut g| f(&mut g.header))
}

pub fn modify_group_footer<T>
(selector: impl GroupSelector, f: impl FnOnce(&mut String) -> T) -> Result<T> {
    modify_group(selector, |mut g| f(&mut g.footer))
}

pub fn set_group_footer(selector: impl GroupSelector, s: impl Into<String>) -> Result {
    modify_group_footer(selector, |h| *h = s.into())
}

pub fn modify_group_collapsed<T>
(selector: impl GroupSelector, f: impl FnOnce(&mut Option<bool>) -> T) -> Result<T> {
    modify_group(selector, |mut g| f(&mut g.collapsed))
}

pub fn collapse_group(selector: impl GroupSelector) -> Result {
    modify_group_collapsed(selector, |b| *b = Some(true))
}

pub fn expand_group(selector: impl GroupSelector) -> Result {
    modify_group_collapsed(selector, |b| *b = Some(false))
}

pub fn shift_selection(shift: isize) -> Result {
    modify_logger(|l| l.shift_selection(shift))
}

pub fn shift_history(shift: isize) -> Result {
    modify_logger(|l| l.shift_history(shift))
}

pub fn scroll(group_index: group::Id, offset: isize) -> Result {
    modify_logger(|l| l.scroll(group_index, offset))?
}

pub fn line_to_group_id(line_ix: framebuffer::LineIndex) -> Result<Option<group::Id>> {
    modify_logger(|logger| logger.frame_buffer.line_to_group(line_ix))
}

pub fn group_to_lines
(group_ix: group::Id) -> Result<Option<(framebuffer::LineIndex, framebuffer::LineIndex)>> {
    modify_logger(|logger| logger.frame_buffer.group_to_lines(group_ix))
}

// =====================================
// === Simplified API for common use ===
// =====================================

fn report_errors<T>(result: Result<T>) {
    if let Err(error) = result {
        modify_logger(|logger| {
            logger.debug_lines.push(format!("Error: {error}"));
        }).ok();
    }
}

pub fn push_log_helper(selector: impl GroupStringSelector, log: Log) -> Result {
    selector.with_selector(|sel|
        modify_logger(|l| {
            l.create_group(sel);
            l.push_line(sel, log)
        })?
    )
}

pub fn log_helper2(selector: &[String], status: Option<Status>, log: String) -> Result {
    let last_log_status =
        modify_logger(|l| {
            l.create_group(selector);
            l.get_last_line(selector).map(|t| t.map(|s| s.status))
        })??;
    let status = status.or_else(|| last_log_status).unwrap_or_default();
    push_log(selector, Log { status, content: log.into() });
    Ok(())
}

pub fn set_header_helper(selector: impl GroupStringSelector, s: impl Into<String>) -> Result {
    selector.with_selector(|sel| {
        modify_logger(|l| l.create_group(sel))?;
        modify_group_header(sel, |h| *h = s.into())
    })
}

pub fn debug(log: impl Into<String>) {
    report_errors(modify_logger(|logger| logger.debug_lines.push(log.into())))
}

pub fn log(selector: impl GroupStringSelector, status: impl Into<Option<Status>>, log: impl Into<String>) {
    selector.with_selector(|sel| report_errors(log_helper2(sel, status.into(), log.into())))
}

pub fn push_log(selector: impl GroupStringSelector, log: Log) {
    report_errors(push_log_helper(selector, log))
}

pub fn set_header(selector: impl GroupStringSelector, s: impl Into<String>) {
    report_errors(set_header_helper(selector, s))
}

#[macro_export]
macro_rules! log {
    ($sel:expr, $msg:literal $($ts:tt)*) => {
        $crate::log($sel, None, format!($msg $($ts)*))
    };
    ($sel:expr, $status:expr, $msg:literal $($ts:tt)*) => {
        $crate::log($sel, $status, format!($msg $($ts)*))
    };
}

// ============
// === Main ===
// ============

pub fn main() -> Result {
    let error: Arc<Mutex<Option<String>>> = default();
    let error2 = error.clone();
    std::panic::set_hook(Box::new(move |info| {
        let mut err = String::new();
        if let Some(location) = info.location() {
            let file = location.file();
            let line = location.line();
            let column = location.column();
            err.push_str(&format!("At: {file}:{line}:{column}\n"));
        }

        err.push_str("Message: ");
        if let Some(msg) = info.payload().downcast_ref::<&'static str>() {
            err.push_str(&format!("{msg}\n"));
        } else if let Some(msg) = info.payload().downcast_ref::<String>() {
            err.push_str(&format!("{msg}\n"));
        } else {
            err.push_str("<non-string panic payload>\n");
        }
        if let Ok(mut t) = error2.lock() {
            *t = Some(err);
        }
    }));

    terminal::capture()?;
    let result = std::panic::catch_unwind(run);
    terminal::cleanup()?;

    result.unwrap_or_else(move |_| {
        let locked_err = error.lock();
        let msg = locked_err
            .as_ref()
            .map(|t| t.as_ref().map(|t| t.as_str()))
            .ok()
            .flatten()
            .unwrap_or("unknown panic (no message captured)");
        Err(anyhow!("Panic occurred: {msg}"))
    })
}

pub fn run() -> Result {
    let mut stdout = std::io::stdout();
    let mut prev_size = terminal::Size::default();

    loop {
        match on_frame(&mut stdout, &mut prev_size) {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => {
                modify_logger(|logger| {
                    logger.debug_lines.push(format!("Error: {error}"));
                })?;
            }
        }
    }
    Ok(())
}

fn history_tile(char: char, tag: group::StatusTag, active: bool) -> String {
    match (active, tag) {
        (true,  group::StatusTag::Success) => char.black().on_green(),
        (true,  group::StatusTag::Error)   => char.black().on_red(),
        (false, group::StatusTag::Success) => char.dark_green().on_green(),
        (false, group::StatusTag::Error)   => char.dark_red().on_red(),
    }.to_string()
}

fn history_tile_active((char, tag): (char, group::StatusTag)) -> String {
    history_tile(char, tag, true)
}

fn history_tile_non_active((char, tag): (char, group::StatusTag)) -> String {
    history_tile(char, tag, false)
}

fn on_frame(stdout: &mut std::io::Stdout, prev_size: &mut terminal::Size) -> Result<bool> {
    let size = terminal::Size::current();
    let bottom_menu_rows = 3;
    let header_and_footer_rows = 2;
    let default_debug_rows = 5;
    let no_menu_rows = size.rows.saturating_sub(bottom_menu_rows);

    modify_logger(|logger| {
        let mut writer = framebuffer::Writer::new(&mut logger.frame_buffer);
        if size != *prev_size {
            writer.clear();
            *prev_size = size;
        }

        let debug_rows_if_any = default_debug_rows.min(no_menu_rows);
        let debug_rows = if logger.debug_lines.is_empty() { 0 } else { debug_rows_if_any };
        let content_rows = no_menu_rows - debug_rows;

        let groups = logger.groups.nonempty();
        let style = &mut logger.style;

        let collapsed_count = groups.iter().filter(|g| g.is_collapsed()).count();
        let expanded_count = groups.len() - collapsed_count;
        let expanded_rows = content_rows.saturating_sub(collapsed_count);
        let (lines_per_group, mut lines_left) = if expanded_count == 0 { (0, 0) } else {
            ((expanded_rows / expanded_count), (expanded_rows % expanded_count))
        };

        for (group_ix, group) in groups.iter().enumerate().map(|t| (group::Id(t.0), t.1)) {
            let new_line = style.header(group, group_ix, &group.header);
            writer.line(Some(group_ix), None, new_line);
            if !group.is_collapsed() {
                let extra_line = if lines_left == 0 { 0 } else {
                    lines_left -= 1;
                    1
                };
                let height = lines_per_group + extra_line;
                let space = height.saturating_sub(header_and_footer_rows);
                let state = group.state();
                let lines = state.view_lines();
                let (scrolled, start_line) = if let Some(scroll) = group.scroll {
                    (true, scroll)
                } else {
                    (false, lines.len().saturating_sub(space))
                };
                for line_index_rel in 0 .. space {
                    let is_last_line = line_index_rel == space - 1;
                    let line_ix = group::LineIndex(start_line + line_index_rel);
                    let content = if scrolled && is_last_line {
                        "..."
                    } else {
                        lines.get(*line_ix).map_or_else(default, |t| t.log.content.as_str())
                    };
                    let new_line = style.log_line(group, group_ix, content);
                    writer.line(Some(group_ix), Some(line_ix), new_line);
                }
                let new_line = style.footer(group, group_ix, &group.footer);
                writer.line(Some(group_ix), None, new_line);
            }
        }
        for _ in writer.line.0 .. content_rows {
            writer.line(None, None, "".to_string());
        }

        // === Scroll Bar ===

        {
            let line_count = *logger.next_line_id;
            let len_f = if line_count == 0 { 1.0 } else {
                (size.cols as f32 / line_count as f32).max(1.0)
            };
            let len = len_f.ceil() as usize;
            let visible_line_count = logger.groups.next_line;
            let shift = visible_line_count.map(|t| *t as f32 / line_count as f32).unwrap_or(1.0);
            let left_space_count = ((size.cols - len) as f32 * shift) as usize;
            let left_space = " ".repeat(left_space_count);
            let bar = "▂".repeat(len).bold().dark_green();
            writer.line(None, None, format!("{left_space}{bar}"))
        };

        // === History ===

        {
            let padding = 1;
            let cols = size.cols.saturating_sub(2 * padding);
            let all_count = logger.history.len();
            let view_count = logger.groups.next_line.map(|t| *t).unwrap_or(all_count);
            let rhs_count = all_count - view_count;
            let max_shift = view_count.saturating_sub(cols/2);
            let shift = rhs_count.min(cols/2).min(max_shift);
            let start_ix = view_count.saturating_sub(cols) + shift;
            let end_ix_succ = (start_ix + cols).min(logger.history.len());
            let is_lhs_clipped = start_ix > 0;
            let is_rhs_clipped = rhs_count > cols/2;
            let visible_count = view_count.saturating_sub(start_ix);
            let history = logger.history[start_ix..end_ix_succ].iter()
                .map(|t| t.map0(|s| index_to_group_char_opt(*s)))
                .collect::<Vec<_>>();
            let (before, current) = visible_count.checked_sub(1).map(|current_ix| {
                let before_start = if is_lhs_clipped { 1 } else { 0 };
                let current = history.get(current_ix).copied().map(history_tile_active)
                    .unwrap_or_default();
                let before = history.get(before_start..current_ix).map(
                    |t| t.iter().copied().map(history_tile_active).collect::<String>()
                ).unwrap_or_default();
                (before, current)
            }).unwrap_or_default();
            let after_end = if is_rhs_clipped { history.len() - 1 } else { history.len() };
            let dots1 = if is_lhs_clipped { "…" } else { "" }.black().on_green();
            let dots2 = if is_rhs_clipped { "…" } else { "" }.dark_green().on_green();
            let after: String = history.get(visible_count .. after_end).map(
                |t| t.iter().copied().map(history_tile_non_active).collect()
            ).unwrap_or_default();
            let pad_str = " ".repeat(padding).on_green();
            let history_str = format!("{pad_str}{dots1}{before}{current}{after}{dots2}{pad_str}");
            let rhs_spaces = " ".repeat(cols.saturating_sub(visible_count)).on_green();
            let new_line = format!("{history_str}{rhs_spaces}");
            writer.line(None, None, new_line)
        };

        // === Menu ===

        let menu_no_selection: &[(&str, &str)] = &[
            ("Help", "?"),
            ("Quit", "q"),
            ("Select", "1-9 a-z ↑↓"),
            ("Inverse Selection", "0"),
            ("Deselect", "Esc"),
            ("History", "←→")
        ];
        let menu_selection: &[(&str, &str)] = &[("Help", "?"), ("Collapse", "Enter")];
        let menu_button = if groups.iter().any(|g| g.selected) {
            menu_selection
        } else {
            menu_no_selection
        };

        let new_line = menu_button.iter().map(|(label, shortcut)| {
            let left = format!(" {label}");
            let right = format!(" {shortcut} ").green().bold();
            format!("{left}{right}")
        }).collect::<Vec<_>>().join("");
        writer.line(None, None, new_line);

        // === Debug Panel ===

        let debug_lines_start = logger.debug_lines.len().saturating_sub(debug_rows);
        let debug_lines_count = logger.debug_lines.len().saturating_sub(debug_lines_start);
        for line in &logger.debug_lines[debug_lines_start..] {
            let fill = " ".repeat(size.cols.saturating_sub(line.len()));
            writer.line(None, None, format!("{line}{fill}").black().on_blue().to_string());
        }
        for _ in debug_lines_count .. debug_rows {
            writer.line(None, None, " ".repeat(size.cols).on_blue().to_string());
        }

        // === Draw ===

        for (i, line) in writer.lines.iter_mut().enumerate() {
            if line.changed {
                crossterm::queue!(
                        stdout,
                        crossterm::cursor::MoveTo(0, i as u16),
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine),
                        crossterm::style::Print(&line.content)
                    )?;
                line.changed = false;
            }
        }
        std::io::Write::flush(stdout)?;
        Result::<(), Error>::Ok(())
    })??;

    use crossterm::event;
    if event::poll(std::time::Duration::from_millis(16))? {
        match event::read()? {
            event::Event::Key(event) => {
                if event.code == event::KeyCode::Char('q') ||
                    event.code == event::KeyCode::Char('c')
                        && event.modifiers.contains(event::KeyModifiers::CONTROL) {
                    return Ok(false);
                }

                match event.code {
                    event::KeyCode::Char(char) => {
                        match char {
                            '0' => modify_all_groups(|mut g| g.selected = !g.selected),
                            _ => {
                                if let Some(index) = group_char_to_index(char).map(group::Id) {
                                    modify_group(index, |mut g| g.selected = !g.selected).ok();
                                }
                                Ok(())
                            }
                        }
                    }
                    event::KeyCode::Enter => modify_all_groups(|mut g| if g.selected {
                        g.collapsed = Some(!g.as_ref().is_collapsed())
                    }),
                    event::KeyCode::Esc => modify_all_groups(|mut g| g.selected = false),
                    event::KeyCode::Down => shift_selection(1),
                    event::KeyCode::Up => shift_selection(-1),
                    event::KeyCode::Left => {
                        let mult = if event.modifiers.contains(event::KeyModifiers::SHIFT) {
                            10
                        } else {
                            1
                        };
                        shift_history(-mult)
                    },
                    event::KeyCode::Right => {
                        let mult = if event.modifiers.contains(event::KeyModifiers::SHIFT) {
                            10
                        } else {
                            1
                        };
                        shift_history(mult)
                    },
                    _ => { Ok (()) }
                }?
            }
            event::Event::Mouse(event) => {
                let row = framebuffer::LineIndex(event.row as usize);
                let column = event.column as usize;
                match event.kind {
                    event::MouseEventKind::ScrollUp => {
                        if let Some(group_id) = line_to_group_id(row)? {
                            scroll(group_id, -1)?;
                        }
                    }
                    event::MouseEventKind::ScrollDown => {
                        if let Some(group_id) = line_to_group_id(row)? {
                            scroll(group_id, 1)?;
                        }
                    }
                    event::MouseEventKind::Down(_) => {
                        if let Some(group_id) = line_to_group_id(row)? {
                            let first_line = group_to_lines(group_id)?.unwrap_or_default().0;
                            if row == first_line && column < 4 {
                                modify_group(group_id, |mut g|
                                    g.collapsed = Some(!g.as_ref().is_collapsed())
                                )?;
                            } else {
                                modify_all_groups(|mut g| g.selected = false)?;
                                modify_group(group_id, |mut g| g.selected = true)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(true)
}

// We start naming from 1, as `0` has a special meaning.
fn group_char_to_index(c: char) -> Option<usize> {
    match c {
        '1'..='9' => Some(c as usize - '0' as usize),
        'a'..='z' => Some(c as usize - 'a' as usize + 10),
        _ => None,
    }.map(|i| i - 1)
}

// We start naming from 1, as `0` has a special meaning.
fn index_to_group_char(d: usize) -> Option<char> {
    match d {
        0..=8 => Some((d as u8 + b'1') as char),
        9..=34 => Some((d as u8 - 9 + b'a') as char),
        _ => None
    }
}

fn index_to_group_char_opt(d: usize) -> char {
    index_to_group_char(d).unwrap_or('?')
}
