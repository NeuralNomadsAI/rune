//! What Android's input method reads of the focused field: Rune's window of text around the
//! selection (`ImeText`: the caret's paragraph, up to a kilobyte each side), in the UTF-16
//! offsets an `InputConnection` counts in. Offsets start at the window, so a caret moving to
//! another paragraph moves the window too, and the view restarts the input method.

use std::ops::Range;

use crate::edit::ImeText;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImeWindow {
    text: String,
    /// Where `text` starts in the field, in bytes.
    start: usize,
    /// The selection, a byte range of `text`.
    selection: Range<usize>,
}

/// The byte index of UTF-16 offset `units` in `s`, rounded `up` or down to a character.
fn byte_at(s: &str, units: usize, up: bool) -> usize {
    let mut seen = 0;
    for (i, c) in s.char_indices() {
        if seen >= units { return i }
        let next = seen + c.len_utf16();
        if next > units { return if up { i + c.len_utf8() } else { i } }
        seen = next;
    }
    s.len()
}

fn units(s: &str) -> usize {
    s.encode_utf16().count()
}

impl ImeWindow {
    pub(crate) fn new(t: ImeText) -> Self {
        Self { text: t.text.clone(), start: t.start, selection: t.selection.clone() }
    }

    /// Whether `other` starts where this one does, so offsets carry over: the text in it and
    /// the selection may have changed, but not the paragraph.
    pub(crate) fn same_origin(&self, other: &Self) -> bool {
        self.start == other.start
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// The selection in UTF-16 offsets from the window's start.
    pub(crate) fn selection_units(&self) -> (usize, usize) {
        (units(&self.text[..self.selection.start]), units(&self.text[..self.selection.end]))
    }

    /// UTF-16 offsets `a..b` of the window as a byte range of the field, widened to whole
    /// characters; `None` when they leave the window.
    pub(crate) fn field_range(&self, a: usize, b: usize) -> Option<Range<usize>> {
        let (a, b) = (a.min(b), a.max(b));
        if b > units(&self.text) { return None }
        Some(self.start + byte_at(&self.text, a, false)..self.start + byte_at(&self.text, b, true))
    }

    /// `deleteSurroundingText(before, after)` as field byte ranges to remove, the one after the
    /// selection first so the one before keeps its offsets. Empty ranges are left out.
    pub(crate) fn surrounding(&self, before: usize, after: usize) -> Vec<Range<usize>> {
        let (a, b) = self.selection_units();
        [self.field_range(b, b + after.min(units(&self.text) - b)), self.field_range(a.saturating_sub(before), a)]
            .into_iter()
            .flatten()
            .filter(|r| !r.is_empty())
            .collect()
    }
}
