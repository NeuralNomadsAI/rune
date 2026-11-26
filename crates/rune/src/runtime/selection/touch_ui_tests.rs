use super::{Ends, Handle};
use crate::{Cx, El, Env, Runtime, ScrollSpec, TextEdit, TextStyle, TouchPhase, col, el, text, text_input};

const TEXT: &str = "hello brave world";
const FIELD: &str = "alpha beta gamma";

struct S { edit: TextEdit }

fn view(s: &S, _: &mut Cx<S>) -> El {
    col().size(400.0, 300.0).scroll(ScrollSpec { stick_bottom: false })
        .child(text(TEXT).key("t").font(TextStyle::new(14.0, 400.0)).selectable(true))
        .child(text_input(&s.edit, TextStyle::new(14.0, 400.0)).key("f").on_edit(|s: &mut S| &mut s.edit).size(300.0, 32.0))
        .child(text("more").key("plain").font(TextStyle::new(14.0, 400.0)))
        .child(el().w_full().h(600.0))
}

/// A runtime whose host shows text menus and selection handles itself, as Android's does.
fn rig() -> Runtime<S> {
    let mut rt = Runtime::new(S { edit: TextEdit::new(FIELD) }, view, (400.0, 300.0), Env::default());
    rt.core_mut().host.text_menus = true;
    rt.frame(0.0);
    rt
}

fn long_press(rt: &mut Runtime<S>, x: f32, y: f32) {
    rt.touch(1, TouchPhase::Started, x, y, 0.0);
    rt.frame(0.6);
    rt.touch(1, TouchPhase::Ended, x, y, 0.6);
    rt.frame(0.0);
}

fn selected(rt: &Runtime<S>) -> (usize, usize) {
    let (_, r) = rt.text_selection().expect("a text selection");
    (r.start, r.end)
}

fn range(rt: &Runtime<S>) -> (End2, End2) {
    match rt.selection_ends() {
        Some(Ends::Range { start, end }) => ((start.x, (start.top + start.bottom) / 2.0), (end.x, (end.top + end.bottom) / 2.0)),
        other => panic!("no selected range: {other:?}"),
    }
}
type End2 = (f32, f32);

#[test]
fn a_long_press_on_text_selects_its_word_and_asks_the_host_for_the_menu() {
    let mut rt = rig();
    let y = rt.find("t").unwrap().1;
    long_press(&mut rt, 60.0, y);
    let (a, b) = selected(&rt);
    assert!(["hello", "brave", "world"].contains(&&TEXT[a..b]), "{:?}", &TEXT[a..b]);
    let ((sx, _), (ex, _)) = range(&rt);
    assert!(sx < 60.0 && 60.0 < ex, "{sx} {ex}");
    let show = rt.take_text_menu().expect("the Copy menu");
    assert!(show.menu.find("copy").unwrap().is_enabled());
    assert_eq!(rt.take_text_menu(), None, "one menu at a time");

    let board = crate::clipboard::memory();
    rt.use_clipboard(&board);
    rt.resolve_popup(Some("copy")).unwrap();
    rt.frame(0.0);
    assert_eq!(board.text().as_ref().map(|t| t.as_str()), Some(&TEXT[a..b]));
    assert_eq!(selected(&rt), (a, b), "copying keeps the selection");
}

#[test]
fn a_long_press_in_a_field_selects_its_word_and_the_host_gets_the_field_menu() {
    let mut rt = rig();
    let (cx, y) = rt.find("f").unwrap();
    long_press(&mut rt, cx - 150.0 + 14.0, y);
    assert!(rt.state.edit.focused);
    let (a, b) = rt.state.edit.range();
    assert_eq!(&FIELD[a..b], "alpha");
    let show = rt.take_text_menu().expect("the field's menu");
    for id in ["cut", "copy", "paste", "select-all"] {
        assert!(show.menu.find(id).unwrap().is_enabled(), "{id}");
    }
    rt.resolve_popup(Some("select-all")).unwrap();
    rt.frame(0.0);
    assert_eq!(rt.state.edit.range(), (0, FIELD.len()));
}

#[test]
fn an_app_menu_is_answered_none_and_the_text_menu_behind_it_opens() {
    let mut rt = rig();
    rt.update(|_, tx| tx.popup_menu(crate::PopupMenu::picker([("a", "A")], "a"), (10.0, 10.0), |s: &mut S, picked, _| {
        s.edit = TextEdit::new(if picked.is_none() { "refused" } else { "picked" });
    }));
    let y = rt.find("t").unwrap().1;
    long_press(&mut rt, 60.0, y);
    assert!(rt.take_text_menu().is_some());
    rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "refused");
}

#[test]
fn the_end_handle_extends_the_selection_and_the_start_handle_pulls_it_back() {
    let mut rt = rig();
    let y = rt.find("t").unwrap().1;
    long_press(&mut rt, 60.0, y);
    rt.take_text_menu();
    rt.resolve_popup(None).unwrap();
    let (a, b) = selected(&rt);
    let (_, (ex, ey)) = range(&rt);

    assert!(rt.grab_handle(Handle::End));
    rt.drag_handle(ex, ey);
    assert_eq!(selected(&rt), (a, b), "a grab alone changes nothing");
    rt.drag_handle(390.0, ey);
    rt.release_handle();
    rt.frame(0.0);
    assert_eq!(selected(&rt), (a, TEXT.len()));

    assert!(rt.grab_handle(Handle::Start));
    rt.drag_handle(0.0, ey);
    rt.release_handle();
    rt.frame(0.0);
    assert_eq!(selected(&rt), (0, TEXT.len()));
}

#[test]
fn a_field_handle_moves_its_end_and_the_caret_handle_moves_the_caret() {
    let mut rt = rig();
    let (cx, y) = rt.find("f").unwrap();
    long_press(&mut rt, cx - 150.0 + 14.0, y);
    rt.take_text_menu();
    rt.resolve_popup(None).unwrap();
    assert!(rt.grab_handle(Handle::End));
    rt.drag_handle(cx + 140.0, y);
    rt.release_handle();
    rt.frame(0.0);
    assert_eq!(rt.state.edit.range(), (0, FIELD.len()));

    rt.update(|s, _| s.edit.set_caret(0));
    rt.frame(0.0);
    let Some(Ends::Caret(at)) = rt.selection_ends() else { panic!("no caret: {:?}", rt.selection_ends()) };
    assert!(at.shown && at.bottom > at.top);
    assert!(rt.grab_handle(Handle::Caret));
    rt.drag_handle(cx + 140.0, y);
    rt.release_handle();
    rt.frame(0.0);
    assert_eq!(rt.state.edit.range(), (FIELD.len(), FIELD.len()));
}

#[test]
fn the_menu_opens_again_for_the_selection_as_it_is() {
    let mut rt = rig();
    let y = rt.find("t").unwrap().1;
    long_press(&mut rt, 60.0, y);
    rt.take_text_menu();
    rt.resolve_popup(None).unwrap();
    let before = selected(&rt);
    assert!(rt.open_text_menu());
    rt.frame(0.0);
    assert!(rt.take_text_menu().is_some_and(|show| show.menu.find("copy").is_some()));
    assert_eq!(selected(&rt), before);
    rt.resolve_popup(None).unwrap();

    // A field's: its caret stays where it is, with Paste to offer.
    rt.update(|s, _| { s.edit.focused = true; s.edit.set_caret(3); });
    rt.frame(0.0);
    assert!(rt.open_text_menu());
    rt.frame(0.0);
    let show = rt.take_text_menu().expect("the field's menu");
    assert!(show.menu.find("paste").unwrap().is_enabled() && !show.menu.find("copy").unwrap().is_enabled());
    assert_eq!(rt.state.edit.range(), (3, 3));
}

#[test]
fn nothing_selected_has_no_ends_and_no_menu() {
    let mut rt = rig();
    assert_eq!(rt.selection_ends(), None);
    assert!(!rt.open_text_menu());
    assert!(!rt.grab_handle(Handle::End));
    // Plain text is not selectable: a long press on it selects nothing.
    let (x, y) = rt.find("plain").unwrap();
    long_press(&mut rt, x, y);
    assert_eq!(rt.selection_ends(), None);
    assert_eq!(rt.take_text_menu(), None);
}

#[test]
fn without_a_touch_host_a_long_press_keeps_the_desktop_behaviour() {
    let mut rt = rig();
    rt.core_mut().host.text_menus = false;
    let (cx, y) = rt.find("f").unwrap();
    long_press(&mut rt, cx - 150.0 + 14.0, y);
    let (a, b) = rt.state.edit.range();
    assert_eq!(a, b, "the caret moves; no word is selected");
}

#[test]
fn a_tap_in_a_field_still_focuses_it_and_puts_the_caret_under_the_finger() {
    let mut rt = rig();
    let (cx, y) = rt.find("f").unwrap();
    rt.touch(1, TouchPhase::Started, cx - 150.0 + 14.0, y, 0.0);
    rt.touch(1, TouchPhase::Ended, cx - 150.0 + 14.0, y, 0.05);
    rt.frame(0.05);
    rt.frame(0.0);
    assert!(rt.state.edit.focused);
    let (a, b) = rt.state.edit.range();
    assert!(a == b && a < 5, "{a} {b}");
    assert!(matches!(rt.selection_ends(), Some(Ends::Caret(_))));
}

#[test]
fn a_scroll_under_selected_text_keeps_it_and_a_tap_elsewhere_clears_it() {
    let mut rt = rig();
    let y = rt.find("t").unwrap().1;
    long_press(&mut rt, 60.0, y);
    rt.take_text_menu();
    rt.resolve_popup(None).unwrap();
    let before = selected(&rt);
    // A finger lands away from the text and moves off as a pan.
    rt.touch(1, TouchPhase::Started, 200.0, 250.0, 1.0);
    assert_eq!(selected(&rt), before, "landing changes nothing");
    for i in 1..=5 { rt.touch(1, TouchPhase::Moved, 200.0, 250.0 - 10.0 * i as f32, 1.0 + 0.01 * f64::from(i)); }
    rt.touch(1, TouchPhase::Ended, 200.0, 200.0, 1.06);
    rt.frame(0.1);
    assert_eq!(selected(&rt), before);
    // A tap there clears it.
    rt.touch(1, TouchPhase::Started, 200.0, 250.0, 2.0);
    rt.touch(1, TouchPhase::Ended, 200.0, 250.0, 2.05);
    rt.frame(0.1);
    assert_eq!(rt.text_selection(), None);
    assert_eq!(rt.selection_ends(), None);
}
