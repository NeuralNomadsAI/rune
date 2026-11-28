use super::back;
use crate::{Cx, El, Env, Key, Runtime, TouchPhase, col, el};

#[derive(Default)]
struct S {
    panel: bool,
    edit: crate::TextEdit,
}

fn view(s: &S, _: &mut Cx<S>) -> El {
    col()
        .child(el().key("open").size(40.0, 20.0).focusable(true).on_press(|s: &mut S| s.panel = true))
        .child(crate::text_input(&s.edit, crate::tok::type_::BODY).key("field").on_edit(|s: &mut S| &mut s.edit))
        .children(s.panel.then(|| el().key("panel").size(40.0, 20.0)))
}

fn rig() -> Runtime<S> {
    let mut rt = Runtime::new(S::default(), view, (200.0, 200.0), Env::default());
    // The app closes its panel on Escape, and leaves every other key alone.
    rt.on_key = Some(|s, ev, _| ev.key == Key::Escape && std::mem::take(&mut s.panel));
    rt.frame(0.0);
    rt
}

/// One back gesture: whether Rune took it, so the activity stays.
fn back(rt: &mut Runtime<S>) -> bool {
    back::press(rt.core_mut());
    rt.frame(0.0);
    back::taken(rt.core_mut())
}

/// A finger taps the element keyed `key`.
fn tap(rt: &mut Runtime<S>, key: &str) {
    let (x, y) = rt.find(key).unwrap();
    rt.touch(1, TouchPhase::Started, x, y, 0.0);
    rt.frame(0.0);
    rt.touch(1, TouchPhase::Ended, x, y, 0.05);
    rt.frame(0.05);
    rt.frame(0.1);
}
#[test]
fn back_closes_what_is_open_then_leaves_the_app() {
    let mut rt = rig();
    assert!(!back(&mut rt), "nothing open and no focus: the activity goes back");
    rt.update(|s, _| s.panel = true);
    rt.frame(0.0);
    assert!(back(&mut rt), "the app closed its panel");
    assert!(!rt.state.panel);
    assert!(!back(&mut rt), "nothing left to close");
}

#[test]
fn a_tapped_button_keeping_focus_does_not_hold_the_user_in_the_app() {
    let mut rt = rig();
    tap(&mut rt, "open");
    assert!(rt.state.panel, "the tap opened the panel");
    assert!(rt.has_focus(), "and left the button focused");
    assert!(back(&mut rt), "the panel closes first");
    assert!(rt.has_focus(), "the button keeps its focus");
    assert!(!back(&mut rt), "Escape means nothing to a button: the activity goes back");
}
#[test]
fn back_leaves_a_field_before_it_leaves_the_app() {
    let mut rt = rig();
    tap(&mut rt, "field");
    assert!(rt.has_focus());
    assert!(back(&mut rt), "Escape leaves the field");
    assert!(!rt.has_focus());
    assert!(!back(&mut rt));
}