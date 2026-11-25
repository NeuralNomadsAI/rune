//! Android `MotionEvent`s as Rune touch phases. One event carries every finger down; only the
//! finger its action names starts or lifts, and a move moves them all.

use crate::TouchPhase;

pub(crate) const ACTION_DOWN: i32 = 0;
pub(crate) const ACTION_UP: i32 = 1;
pub(crate) const ACTION_MOVE: i32 = 2;
pub(crate) const ACTION_CANCEL: i32 = 3;
pub(crate) const ACTION_POINTER_DOWN: i32 = 5;
pub(crate) const ACTION_POINTER_UP: i32 = 6;

/// The phase of each pointer index (`0..count`) in an event with `getActionMasked() == action`
/// and `getActionIndex() == index`.
pub(crate) fn phases(action: i32, index: usize, count: usize) -> impl Iterator<Item = (usize, TouchPhase)> {
    let (range, phase) = match action {
        ACTION_DOWN | ACTION_POINTER_DOWN if index < count => (index..index + 1, TouchPhase::Started),
        ACTION_UP | ACTION_POINTER_UP if index < count => (index..index + 1, TouchPhase::Ended),
        ACTION_MOVE => (0..count, TouchPhase::Moved),
        ACTION_CANCEL => (0..count, TouchPhase::Cancelled),
        _ => (0..0, TouchPhase::Moved),
    };
    range.map(move |i| (i, phase))
}
