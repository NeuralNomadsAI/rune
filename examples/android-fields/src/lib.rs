//! A field of each kind, to check on a device which keyboard each one raises: several lines,
//! one line (its action key submits, with a haptic), a secret, and one for each
//! `InputMode`; and two buttons that pick documents to read and one to write (`Tx::pick_files`,
//! `Tx::pick_save_file`), read and written off the UI thread through their `FileHandle`s; and
//! two more that copy an image and say what the clipboard holds.
//! `scripts/android-run.sh --app fields`.


use rune::tok::{self, type_};
use std::io::{Read, Write};

use rune::{
    AppSpec, ClipboardContent, ClipboardImage, Cx, El, FileDialogOptions, FileHandle, Haptic, HapticOutcome, InputMode, Latest, ScrollSpec, TextEdit, Tx, col,
    edit_access, text, text_input,
};

/// One-line fields that say what they hold.
const KINDS: [(&str, InputMode); 7] = [
    ("E-mail", InputMode::Email),
    ("Website", InputMode::Url),
    ("Count", InputMode::Numeric),
    ("Amount", InputMode::Decimal),
    ("Phone", InputMode::Tel),
    ("Search", InputMode::Search),
    ("No keyboard", InputMode::None),
];

struct State {
    note: TextEdit,
    name: TextEdit,
    secret: TextEdit,
    pin: TextEdit,
    kinds: [TextEdit; KINDS.len()],
    submitted: String,
    /// What became of the documents last picked: a line each, or why there is none.
    picked: Vec<String>,
    /// The documents being read or written, off the UI thread.
    files: Latest<Vec<String>>,
}

/// "name: N bytes read", and whether the platform gave the document a path.
fn read(file: &FileHandle) -> String {
    let mut bytes = Vec::new();
    let read = file.reader().and_then(|mut reader| reader.read_to_end(&mut bytes));
    let place = if file.path().is_some() { "has a path" } else { "no path" };
    match read {
        Ok(n) => format!("{}: {n} bytes read, {place}", file.name()),
        Err(why) => format!("{}: {why}", file.name()),
    }
}

fn write(file: &FileHandle) -> String {
    let text = b"written by Rune through a FileHandle\n";
    match file.writer().and_then(|mut writer| writer.write_all(text)) {
        Ok(()) => format!("{}: {} bytes written", file.name(), text.len()),
        Err(why) => format!("{}: {why}", file.name()),
    }
}

fn button(label: &'static str, key: &'static str, t: &tok::Theme) -> El {
    text(label).key(key).role("button").focusable(true).font(type_::TITLE).color(t.text).padding(10.0).radius(8.0).fill(t.control_off)
}

impl Default for State {
    fn default() -> Self {
        Self {
            note: TextEdit::default(),
            name: TextEdit::default(),
            secret: TextEdit::default().secure(),
            pin: TextEdit::default().secure(),
            kinds: Default::default(),
            submitted: String::new(),
            picked: Vec::new(),
            files: Latest::default(),
        }
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
    let mut page = col().key("page").w_full().h_full().padding(20.0).gap(18.0).scroll(ScrollSpec { stick_bottom: false })
            .child(field("Note", text_input(&s.note, type_::BODY).key("note").on_edit(|s: &mut State| &mut s.note).min_h(64.0), &t))
            .child(field("Name", text_input(&s.name, type_::BODY).single_line().key("name").on_edit(|s: &mut State| &mut s.name)
                .on_submit(|s: &mut State, tx| {
                    s.submitted = s.name.text.clone();
                    tx.haptic(Haptic::Notify(HapticOutcome::Success));
                }), &t))
            .child(field("Password", text_input(&s.secret, type_::BODY).single_line().secure().key("secret").on_edit(|s: &mut State| &mut s.secret), &t))
            .child(field("PIN", text_input(&s.pin, type_::BODY).single_line().secure().input_mode(InputMode::Numeric).key("pin").on_edit(|s: &mut State| &mut s.pin), &t));
    for (i, (label, keyboard)) in KINDS.into_iter().enumerate() {
        let input = text_input(&s.kinds[i], type_::BODY).single_line().input_mode(keyboard).key(label).on_edit(edit_access(move |s: &mut State| &mut s.kinds[i]));
        page = page.child(field(label, input, &t));
    }
    let attach = button("Attach files", "attach", &t).on_press_tx(|_: &mut State, tx: &mut Tx| {
        tx.pick_files(FileDialogOptions::default(), |s: &mut State, result, _| match result {
            Ok(Some(files)) => s.files.request(files, |files| files.iter().map(read).collect()),
            Ok(None) => s.picked = vec!["Nothing picked".to_owned()],
            Err(why) => s.picked = vec![why.to_string()],
        });
    });
    let save = button("Save a note", "save", &t).on_press_tx(|_: &mut State, tx: &mut Tx| {
        let options = FileDialogOptions { file_name: Some("rune-note.txt".into()), ..Default::default() }.filter("Text", &["txt"]);
        tx.pick_save_file(options, |s: &mut State, result, _| match result {
            Ok(Some(file)) => s.files.request(file, |file| vec![write(&file)]),
            Ok(None) => s.picked = vec!["Nothing saved".to_owned()],
            Err(why) => s.picked = vec![why.to_string()],
        });
    });
    let copy = button("Copy an image", "copy-image", &t).on_press_tx(|s: &mut State, tx: &mut Tx| {
        // Two red pixels by two: enough for another app to paste and for this one to read back.
        let Some(image) = ClipboardImage::from_rgba(2, 2, &[255, 0, 0, 255].repeat(4)) else { return };
        tx.copy_rich(ClipboardContent::new("").with_image(image));
        s.picked = vec!["Image copied".to_owned()];
    });
    let read = button("Read the clipboard", "read-clipboard", &t).on_press_tx(|_: &mut State, tx: &mut Tx| {
        tx.read_clipboard(|s: &mut State, content, _| {
            s.picked = vec![match content.image() {
                Some(image) => format!("Clipboard: an image, {} by {}", image.size().0, image.size().1),
                None if content.text().is_empty() => "Clipboard: empty".to_owned(),
                None => format!("Clipboard: {} characters of text", content.text().chars().count()),
            }];
        });
    });
    let mut page = page.child(text(submitted).key("submitted").font(type_::BODY).color(t.text_secondary)).child(attach).child(save).child(copy).child(read);
    for (i, line) in s.picked.iter().enumerate() {
        page = page.child(text(line.clone()).key(format!("picked-{i}")).font(type_::BODY).color(t.text_secondary));
    }
    col().w_full().h_full().pt(top).pr(right).pb(bottom).pl(left).fill(t.bg).child(page)
}

/// Opens the scene: Android's entry point.
pub fn main() {
    rune::run(AppSpec::<State> {
        setup: Some(|s: &mut State, waker| s.files.set_waker(waker)),
        // True only when a read or a write finished, so an idle window draws nothing.
        poll: Some(|s: &mut State, _, tx: &mut Tx| {
            let Some(lines) = s.files.take() else { return false };
            s.picked = lines;
            tx.instant();
            true
        }),
        ..AppSpec::new("Fields", (360.0, 640.0), State::default, view)
    });
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
        assert_eq!(rt.take_haptics(), [Haptic::Notify(HapticOutcome::Success)]);
        assert!(rt.state.secret.is_secure());
    }
}