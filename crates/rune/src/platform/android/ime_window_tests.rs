use super::ime_window::ImeWindow;
use crate::edit::ImeText;

fn window(text: &str, start: usize, selection: std::ops::Range<usize>) -> ImeWindow {
    ImeWindow::new(ImeText { text: text.into(), start, selection })
}

#[test]
fn ascii_counts_the_same_in_bytes_and_units() {
    let w = window("hello world", 10, 5..5);
    assert_eq!(w.selection_units(), (5, 5));
    assert_eq!(w.field_range(0, 5), Some(10..15));
}

#[test]
fn accents_and_ideographs_are_one_unit_and_several_bytes() {
    // "é" is 2 bytes and "語" 3, each one UTF-16 unit.
    let w = window("café 語", 0, 9..9);
    assert_eq!(w.selection_units(), (6, 6));
    assert_eq!(w.field_range(3, 4), Some(3..5));
}

#[test]
fn an_emoji_is_two_units_and_never_split() {
    // "😀" is 4 bytes and two UTF-16 units (a surrogate pair).
    let w = window("a😀b", 0, 5..5);
    assert_eq!(w.selection_units(), (3, 3));
    assert_eq!(w.field_range(1, 2), Some(1..5));
}

#[test]
fn the_selection_reads_as_itself() {
    let w = window("one two three", 0, 4..7);
    assert_eq!(w.selection_units(), (4, 7));
}

#[test]
fn surrounding_deletes_after_first_and_skips_empty_sides() {
    let w = window("ab😀cd", 100, 6..6);
    // One emoji before the caret (2 units), one letter after.
    assert_eq!(w.surrounding(2, 1), [106..107, 102..106]);
    assert_eq!(w.surrounding(0, 0), Vec::<std::ops::Range<usize>>::new());
    // Past either end of the window clamps to it.
    assert_eq!(w.surrounding(99, 99), [106..108, 100..106]);
}

#[test]
fn a_range_past_the_window_is_refused_and_the_origin_is_the_paragraph_start() {
    let w = window("abc", 0, 1..1);
    assert_eq!(w.field_range(1, 9), None);
    assert!(w.same_origin(&window("abcd", 0, 2..2)));
    assert!(!w.same_origin(&window("abc", 4, 1..1)));
}
