//! A field of each kind, to check on a device which keyboard each one raises: several lines,
//! one line (its action key submits) and a secret. `scripts/android-run.sh --app fields`.


use rune::tok::{self, type_};
use rune::{AppSpec, Cx, El, ScrollSpec, TextEdit, col, text, text_input};

struct State {
    note: TextEdit,
    name: TextEdit,
    secret: TextEdit,
    submitted: String,
}

impl Default for State {
    fn default() -> Self {
        Self { note: TextEdit::default(), name: TextEdit::default(), secret: TextEdit::default().secure(), submitted: String::new() }
    }
}

fn field(label: &'static str, input: El, t: &tok::Theme) -> El {
    col().gap(6.0).w_full()
        .child(text(label).font(type_::LABEL).color(t.text_secondary))
        .child(input.label(label).w_full().padding(10.0).radius(8.0).fill(t.card).border(1.0, t.card_border).color(t.text))
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let t = tok::theme(true);
    // Clear of a phone's status bar, cutout and keyboard; zero on a desktop.
    let [top, right, bottom, left] = cx.safe_area();
    let submitted = if s.submitted.is_empty() { "Nothing submitted".to_owned() } else { format!("Submitted: {}", s.submitted) };
    col().w_full().h_full().pt(top).pr(right).pb(bottom).pl(left).fill(t.bg).child(
        col().key("page").w_full().h_full().padding(20.0).gap(18.0).scroll(ScrollSpec { stick_bottom: false })
            .child(field("Note", text_input(&s.note, type_::BODY).key("note").on_edit(|s: &mut State| &mut s.note).min_h(64.0), &t))
            .child(field("Name", text_input(&s.name, type_::BODY).single_line().key("name").on_edit(|s: &mut State| &mut s.name)
                .on_submit(|s: &mut State, _| s.submitted = s.name.text.clone()), &t))
            .child(field("Password", text_input(&s.secret, type_::BODY).single_line().secure().key("secret").on_edit(|s: &mut State| &mut s.secret), &t))
            .child(text(submitted).key("submitted").font(type_::BODY).color(t.text_secondary)),
    )
}

/// Opens the scene: Android's entry point.
pub fn main() {
    rune::run(AppSpec::<State>::new("Fields", (360.0, 640.0), State::default, view));
}

rune::android_main!(main);

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime, TouchPhase};

    #[test]
    fn the_one_line_field_submits_and_the_secret_keeps_its_text_hidden() {
        let mut rt = Runtime::new(State::default(), view, (360.0, 640.0), Env::default());
        rt.frame(0.0);
        let (x, y) = rt.find("name").unwrap();
        rt.touch(1, TouchPhase::Started, x, y, 0.0);
        rt.touch(1, TouchPhase::Ended, x, y, 0.05);
        rt.frame(0.0);
        rt.key(rune::KeyEvent { key: rune::Key::Text("Ada".into()), mods: rune::Mods::default() });
        rt.key(rune::KeyEvent { key: rune::Key::Enter, mods: rune::Mods::default() });
        rt.frame(0.0);
        assert_eq!(rt.state.submitted, "Ada");
        assert!(rt.state.secret.is_secure());
    }
}