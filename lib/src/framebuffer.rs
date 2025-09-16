use crate::prelude::*;

use std::collections::HashMap;
use crate::group;

// =================
// === LineIndex ===
// =================

#[derive(Clone, Copy, Debug, Deref, Default, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct LineIndex(pub usize);

impl LineIndex {
    pub fn inc_mut(&mut self) {
        self.0 += 1;
    }

    pub fn inc(self) -> Self {
        Self(self.0 + 1)
    }
}

// ============
// === Line ===
// ============

#[derive(Clone, Debug, Default)]
pub struct Line {
    pub changed: bool,
    pub content: String,
}

// ===================
// === Framebuffer ===
// ===================

#[derive(Clone, Debug, Default)]
pub struct Framebuffer {
    pub lines: Vec<Line>,
    pub line_to_group: HashMap<LineIndex, Option<group::Id>>,
    pub group_to_lines: HashMap<group::Id, (LineIndex, LineIndex)>,
    pub group_to_group_lines: HashMap<group::Id, (group::LineIndex, group::LineIndex)>
}

impl Framebuffer {
    fn set_line(
        &mut self,
        line_ix: LineIndex,
        group: Option<group::Id>,
        group_line_ix: Option<group::LineIndex>,
        content: String
    ) {
        self.line_to_group.insert(line_ix, group);
        if let Some(group_ix) = group {
            self.group_to_lines.entry(group_ix).or_insert((line_ix, line_ix)).1 = line_ix;
            if let Some(group_line_ix) = group_line_ix {
                let line_range = self.group_to_group_lines
                    .entry(group_ix)
                    .or_insert((group_line_ix, group_line_ix));
                line_range.1 = group_line_ix;
            }
        }
        if LineIndex(self.lines.len()) <= line_ix {
            self.lines.resize(line_ix.inc().0, default());
        }
        let line = &mut self.lines[line_ix.0];
        if line.content != content {
            line.content = content;
            line.changed = true;
        }
    }

    pub fn line_to_group(&self, index: LineIndex) -> Option<group::Id> {
        self.line_to_group.get(&index).copied().flatten()
    }

    pub fn group_to_lines(&self, group_index: group::Id) -> Option<(LineIndex, LineIndex)> {
        self.group_to_lines.get(&group_index).copied()
    }

    fn on_frame(&mut self) {
        self.group_to_lines.clear();
        self.group_to_group_lines.clear();
        self.line_to_group.clear();
    }

    pub fn clear(&mut self) {
        for line in &mut self.lines {
            line.content.clear();
            line.changed = true;
        }
    }
}

// ==============
// === Writer ===
// ==============

#[derive(Deref, DerefMut)]
pub struct Writer<'t> {
    #[deref]
    #[deref_mut]
    pub framebuffer: &'t mut Framebuffer,
    pub line: LineIndex,
}

impl<'t> Writer<'t> {
    pub fn new(framebuffer: &'t mut Framebuffer) -> Self {
        framebuffer.on_frame();
        let line = default();
        Self { framebuffer, line }
    }
    
    pub fn line(
        &mut self,
        group: Option<group::Id>,
        group_line: Option<group::LineIndex>,
        content: String
    ) {
        self.framebuffer.set_line(self.line, group, group_line, content);
        self.line.inc_mut();
    }
}
