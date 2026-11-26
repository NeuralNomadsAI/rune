//! What a touch host shows over selected text, as Android's text does: a handle on each end of
//! the selection (or one under a field's caret), which a finger drags to move that end, and the
//! text's own menu as the platform's selection toolbar. The host sets `host.text_menus`, reads
//! [`RuntimeCore::selection_ends`] after a frame, and takes the menu with
//! `RuntimeCore::take_text_menu`.
#![cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]

use super::*;

/// One end of the selection, in window points: the line it is on, from `top` to `bottom` at `x`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct End {
    pub(crate) x: f32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    /// Inside what clips its text: a handle on an end scrolled out of sight would float over
    /// what covers it.
    pub(crate) shown: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Ends {
    /// The focused field's caret, with nothing selected.
    Caret(End),
    Range { start: End, end: End },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handle {
    Start,
    End,
    Caret,
}

/// What a held handle moves.
#[derive(Clone, Debug)]
pub(in crate::runtime) enum Grab {
    /// The focused field's selection from byte `fixed`, or its caret.
    Field { path: String, fixed: Option<usize> },
    /// The selectable text's selection, held as a press at its other end.
    Text,
}

impl RuntimeCore {
    fn focused_field(&self) -> Option<usize> {
        self.focused_index().filter(|&i| self.tree.nodes[i].input.is_some() && !self.subtree_exiting(i))
    }

    /// Where the selection's handles hang, as last laid out.
    pub(crate) fn selection_ends(&self) -> Option<Ends> {
        let shown = |i: usize, x: f32, y: f32| self.shown_box(i).is_some_and(|[x0, y0, x1, y1]| x >= x0 - 1.0 && x <= x1 + 1.0 && y >= y0 && y <= y1);
        let end = |i: usize, x: f32, [_, y, _, h]: [f32; 4]| End { x, top: y, bottom: y + h, shown: shown(i, x, y + h / 2.0) };
        if let Some([(i, first), (j, last)]) = self.selection_end_boxes() {
            return Some(Ends::Range { start: end(i, first[0], first), end: end(j, last[0] + last[2], last) });
        }
        let i = self.focused_field()?;
        let n = &self.tree.nodes[i];
        if n.input.as_ref().is_some_and(|inp| inp.edit.preedit().is_some()) || self.node_disabled(i) { return None; }
        let caret = self.caret_rect(&self.tree.nodes, i)?;
        Some(Ends::Caret(end(i, caret[0], caret)))
    }

    /// A finger took `handle`: its end follows [`Self::drag_handle`] until
    /// [`Self::release_handle`]. False when there is no such end to move.
    pub(crate) fn grab_handle(&mut self, handle: Handle) -> bool {
        let grab = if let Some(i) = self.focused_field() {
            let Some(inp) = self.tree.nodes[i].input.as_deref() else { return false };
            let (a, b) = inp.edit.range();
            let fixed = match handle {
                Handle::Caret if a == b => None,
                Handle::Start if a < b => Some(b),
                Handle::End if a < b => Some(a),
                _ => return false,
            };
            Grab::Field { path: self.tree.nodes[i].path.clone(), fixed }
        } else {
            // A selection whose far part scrolled out of a virtual list keeps its ends.
            if self.selection_spans_unmounted() { return false; }
            let ranges = self.selection_ranges();
            let (Some(&(first, a, _)), Some(&(last, _, b))) = (ranges.first(), ranges.last()) else { return false };
            let (i, at) = match handle {
                Handle::Start => (last, b),
                Handle::End => (first, a),
                Handle::Caret => return false,
            };
            let (path, content) = (self.tree.nodes[i].path.clone(), self.content_of(i).to_owned());
            self.edit.selectable.presses.clear();
            self.select_step(phase::Event::Press { path, content, unit: Unit::Char, origin: (at, at) });
            Grab::Text
        };
        self.edit.selectable.grab = Some(grab);
        true
    }

    /// The held handle's end goes to the text under window point (x, y).
    pub(crate) fn drag_handle(&mut self, app: &mut dyn AppDriver, x: f32, y: f32) {
        match self.edit.selectable.grab.clone() {
            Some(Grab::Text) => self.selection_drag(x, y),
            Some(Grab::Field { path, fixed }) => self.drag_field_end(app, &path, fixed, x, y),
            None => {}
        }
    }

    pub(crate) fn release_handle(&mut self) {
        if matches!(self.edit.selectable.grab.take(), Some(Grab::Text)) { self.select_step(phase::Event::Release); }
    }

    fn drag_field_end(&mut self, app: &mut dyn AppDriver, path: &str, fixed: Option<usize>, x: f32, y: f32) {
        let Some(i) = self.live(path) else { return };
        let Some(input) = self.tree.nodes[i].input.as_deref() else { return };
        let (Some(access), single, selection_change) = (input.access.clone(), input.single_line, input.selection_change.clone()) else { return };
        let (Some(layout), Some([tx, ty])) = (self.tree.nodes[i].text_layout.clone(), self.input_text_point(i, x, y)) else { return };
        let s = self.env.scale;
        let mut one = self.transaction(app);
        let e = access(app.states());
        let before = (e.anchor, e.focus);
        let caret = crate::runtime::input_click::hit(e, layout.view(), tx, ty, s, single).caret;
        match fixed {
            Some(fixed) => e.set_selection(fixed, caret),
            None => e.set_caret(caret),
        }
        if (e.anchor, e.focus) == before { return; }
        if let Some(selection_change) = selection_change { selection_change(app.states(), &mut one); }
        self.latch_requests(app, &mut one);
        merge(&mut self.frame.pending, one);
        self.commit.dirty = true;
    }

    /// Asks again for the menu of what is selected, or of the focused field's caret, leaving
    /// both as they are: after a handle moved, or was tapped. False with neither.
    pub(crate) fn open_text_menu(&mut self, app: &dyn ViewDriver) -> bool {
        let Some(at) = self.selection_ends().map(|ends| match ends {
            Ends::Caret(end) | Ends::Range { start: end, .. } => (end.x, end.bottom),
        }) else { return false };
        if let Some(i) = self.focused_field() {
            self.frame.queue.push(Action::FieldMenu(self.tree.nodes[i].path.clone(), at, None));
            return true;
        }
        let Some(path) = self.selection_holder().map(|i| self.tree.nodes[i].path.clone()) else { return false };
        self.push_selection_menu(app, &path, at.0, at.1);
        true
    }
}

impl<S: 'static> Runtime<S> {
    pub(crate) fn selection_ends(&self) -> Option<Ends> { self.core().selection_ends() }
    pub(crate) fn grab_handle(&mut self, handle: Handle) -> bool { self.core_mut().grab_handle(handle) }
    pub(crate) fn drag_handle(&mut self, x: f32, y: f32) {
        let (core, mut app) = self.split_mut();
        core.drag_handle(&mut app, x, y);
    }
    pub(crate) fn release_handle(&mut self) { self.core_mut().release_handle(); }
    pub(crate) fn open_text_menu(&mut self) -> bool {
        let (core, app) = self.split_mut();
        core.open_text_menu(&app)
    }
    pub(crate) fn take_text_menu(&mut self) -> Option<crate::popup_menu::Show> {
        let (core, mut app) = self.split_mut();
        core.take_text_menu(&mut app)
    }
}

#[cfg(test)]
#[path = "touch_ui_tests.rs"]
mod tests;
