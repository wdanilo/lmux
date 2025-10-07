use crate::prelude::*;

use std::time::SystemTime;
use crate::LineRange;

// ==============
// === Status ===
// ==============

#[derive(Clone, Copy, Debug, Default)]
pub struct Status {
    pub progress: Option<f32>,
    pub finished: bool,
    pub tag: StatusTag,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StatusTag {
    #[default]
    Success,
    Error,
}

impl Status {
    pub const fn ok() -> Self {
        let progress = None;
        let finished = false;
        let tag = StatusTag::Success;
        Self { progress, finished, tag }
    }

    pub const fn error() -> Self {
        let progress = None;
        let finished = false;
        let tag = StatusTag::Error;
        Self { progress, finished, tag }
    }

    pub fn progress(self, progress: impl Into<Option<f32>>) -> Self {
        Self { progress: progress.into(), ..self }
    }

    pub const fn finished(self) -> Self {
        Self { finished: true, ..self }
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn is_error(&self) -> bool {
        self.tag == StatusTag::Error
    }
}

// =============
// === Group ===
// =============

#[derive(Clone, Copy, Debug, Deref, Default, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct LineIndex(pub usize);

#[derive(Clone, Copy, Debug, Deref, Default, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Id(pub usize);

#[derive(Debug)]
pub struct Line {
    pub log: Log,
    pub timestamp: crate::LineId,
    pub time: SystemTime,
}

#[derive(Debug)]
pub struct Log {
    pub content: String,
    pub status: Status,
}

#[derive(Debug, Deref, DerefMut)]
pub struct Group {
    #[deref]
    #[deref_mut]
    pub state: State,
    pub auto_collapse: AutoCollapse,
}

#[derive(Debug)]
pub struct State {
    pub id: Id,
    pub header: String,
    pub footer: String,
    pub lines: Vec<Line>,
    pub collapsed: Option<bool>,
    pub selected: bool,
    pub scroll: Option<usize>,
}

impl State {
    pub fn new(id: Id) -> Self {
        let header = default();
        let footer = default();
        let lines = default();
        let collapsed = None;
        let selected = false;
        let scroll = None;
        Self { id, header, footer, lines, collapsed, selected, scroll }
    }
}

impl Group {
    pub fn new(id: Id) -> Self {
        let state = State::new(id);
        let auto_collapse = default();
        Self { state, auto_collapse }
    }
}

// ====================
// === AutoCollapse ===
// ====================

#[derive(Clone)]
pub struct AutoCollapse {
    pub filter: Arc<dyn Fn(LineRange<&State>) -> bool + Send + Sync>
}

impl AutoCollapse {
    pub fn collapse_on_success() -> Self {
        Self { 
            filter: Arc::new(|group: LineRange<&State>| {
                group.lines.last().is_some_and(|line|
                    line.log.status.finished && line.log.status.tag == StatusTag::Success
                )
            })
        }
    }

    pub fn expand_on_error() -> Self {
        Self {
            filter: Arc::new(|group: LineRange<&State>| {
                group.view_lines().last().is_none_or(|line|
                    !(line.log.status.finished && line.log.status.tag == StatusTag::Error)
                )
            })
        }
    }

    pub fn expand_selected() -> Self {
        Self {
            filter: Arc::new(|group: LineRange<&State>| !group.selected)
        }
    }
}

impl Default for AutoCollapse {
    fn default() -> Self {
        Self::expand_on_error()
    }
}

impl Debug for AutoCollapse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutoCollapse").finish()
    }
}

// ============
// === View ===
// ============

impl<'t> LineRange<&'t Group> {
    pub fn is_collapsed(&self) -> bool {
        self.collapsed.unwrap_or_else(||
            (self.auto_collapse.filter)(self.map(|t| &t.state))
        )
    }

    pub fn state(&self) -> LineRange<&'t State> {
        self.map(|t| &t.state)
    }
}

impl LineRange<&State> {
    pub fn view_lines(&self) -> &[Line] {
        if let Some(view_range) = self.next_line {
            let end = self.data.lines.iter().enumerate()
                .find(|l| l.1.timestamp >= view_range)
                .map_or_else(|| self.data.lines.len(), |t| t.0);
            &self.data.lines[..end]
        } else {
            &self.data.lines
        }
    }
}
