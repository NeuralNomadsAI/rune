use super::editor::Editor;
use crate::{Cx, El, Env, Runtime, TextEdit, TextStyle, TouchPhase, col, text, text_input};

struct S { note: TextEdit, name: TextEdit, secret: TextEdit }

fn view(s: &S, _: &mut Cx<S>) -> El {
    let style = TextStyle::new(14.0, 400.0);
    col().size(400.0, 300.0)
        .child(text_input(&s.note, style).key("note").on_edit(|s: &mut S| &mut s.note).size(300.0, 60.0))
        .child(text_input(&s.name, style).single_line().key("name").on_edit(|s: &mut S| &mut s.name).size(300.0, 32.0))
        .child(text_input(&s.secret, style).single_line().secure().key("secret").on_edit(|s: &mut S| &mut s.secret).size(300.0, 32.0))
        .child(text("a label").key("label").font(style))
}

fn rig() -> Runtime<S> {
    let mut rt = Runtime::new(S { note: TextEdit::new("a note"), name: TextEdit::new("Ada"), secret: TextEdit::new("hunter2") }, view, (400.0, 300.0), Env::default());
    rt.frame(0.0);
    rt
}

fn tap(rt: &mut Runtime<S>, key: &str) -> Option<Editor> {
    let (x, y) = rt.find(key).unwrap();
    rt.touch(1, TouchPhase::Started, x, y, 0.0);
    rt.touch(1, TouchPhase::Ended, x, y, 0.05);
    rt.frame(0.0);
    Editor::focused(rt.core_mut())
}

#[test]
fn the_keyboard_is_told_what_the_focused_field_is() {
    let mut rt = rig();
    assert_eq!(Editor::focused(rt.core_mut()), None, "no field has focus");
    assert_eq!(tap(&mut rt, "note"), Some(Editor { secure: false, single_line: false }));
    assert_eq!(tap(&mut rt, "name"), Some(Editor { secure: false, single_line: true }));
    assert_eq!(tap(&mut rt, "secret"), Some(Editor { secure: true, single_line: true }));
}

#[test]
fn each_kind_of_field_is_its_own_code_for_the_view() {
    let codes: Vec<i32> = [(false, false), (false, true), (true, false), (true, true)]
        .map(|(secure, single_line)| Editor { secure, single_line }.code()).to_vec();
    assert_eq!(codes, [0, 2, 1, 3]);
}

#[test]
fn a_keyboards_text_goes_into_a_secret_field_as_into_any_other() {
    let mut rt = rig();
    tap(&mut rt, "name");
    super::editor::commit(rt.core_mut(), " L".into());
    rt.frame(0.0);
    assert_eq!(rt.state.name.text, "Ada L");
    tap(&mut rt, "secret");
    super::editor::commit(rt.core_mut(), "!".into());
    rt.frame(0.0);
    assert_eq!(rt.state.secret.text, "hunter2!");
}
