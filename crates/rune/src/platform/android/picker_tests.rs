use std::path::PathBuf;

use super::picker::{self, Pick};
use crate::{Env, FileDialogOptions, Runtime, el};

#[derive(Default)]
struct S {
    file: Option<Result<Option<PathBuf>, String>>,
    files: Option<Result<Option<Vec<PathBuf>>, String>>,
}

fn rig() -> Runtime<S> {
    let mut rt = Runtime::new(S::default(), |_, _| el().size(40.0, 20.0), (40.0, 20.0), Env::default());
    rt.core_mut().host.file_dialogs = true;
    rt.frame(0.0);
    rt
}

fn text_files() -> FileDialogOptions {
    FileDialogOptions::default().filter("Text", &["txt", "md"]).filter("Images", &["png"])
}

#[test]
fn the_view_is_asked_for_documents_and_its_copies_answer_the_dialog() {
    let mut rt = rig();
    rt.update(|_, tx| tx.open_files(text_files(), |s: &mut S, result, _| s.files = Some(result.map_err(|e| e.to_string()))));
    let ask = picker::take(rt.core_mut());
    assert_eq!(ask, Some(Pick::Show("1\u{1f}1\u{1f}txt,md,png".into())), "the dialog's id, several documents, its filters' extensions");
    assert_eq!(picker::take(rt.core_mut()), None, "asked once");
    picker::answer(rt.core_mut(), 1, Ok(Some(vec!["/cache/a.txt".into(), "/cache/b.md".into()])));
    rt.frame(0.0);
    assert_eq!(rt.state.files, Some(Ok(Some(vec![PathBuf::from("/cache/a.txt"), PathBuf::from("/cache/b.md")]))));
}

#[test]
fn one_document_a_cancel_and_a_copy_that_failed() {
    let mut rt = rig();
    let ask = |rt: &mut Runtime<S>| {
        rt.update(|_, tx| tx.open_file(FileDialogOptions::default(), |s: &mut S, result, _| s.file = Some(result.map_err(|e| e.to_string()))));
        match picker::take(rt.core_mut()) {
            Some(Pick::Show(ask)) => ask,
            other => panic!("no pick for the view: {other:?}"),
        }
    };
    assert_eq!(ask(&mut rt), "1\u{1f}0\u{1f}", "one document of any kind");
    picker::answer(rt.core_mut(), 1, Ok(Some(vec!["/cache/a.txt".into()])));
    rt.frame(0.0);
    assert_eq!(rt.state.file, Some(Ok(Some(PathBuf::from("/cache/a.txt")))));

    assert_eq!(ask(&mut rt), "2\u{1f}0\u{1f}");
    picker::answer(rt.core_mut(), 2, Ok(None));
    rt.frame(0.0);
    assert_eq!(rt.state.file, Some(Ok(None)), "the user backed out of the picker");

    assert_eq!(ask(&mut rt), "3\u{1f}0\u{1f}");
    picker::answer(rt.core_mut(), 3, Err("the document could not be read".into()));
    rt.frame(0.0);
    assert_eq!(rt.state.file, Some(Err("the document could not be read".into())));
}

#[test]
fn a_dialog_the_picker_cannot_stand_in_for_is_refused() {
    let mut rt = rig();
    rt.update(|_, tx| tx.save_file(FileDialogOptions::default(), |s: &mut S, result, _| s.file = Some(result.map_err(|e| e.to_string()))));
    assert_eq!(picker::take(rt.core_mut()), Some(Pick::Refused), "nothing for the view");
    rt.frame(0.0);
    assert!(matches!(&rt.state.file, Some(Err(why)) if why.contains("Android")), "{:?}", rt.state.file);

    rt.update(|_, tx| tx.open_folder(FileDialogOptions::default(), |s: &mut S, result, _| s.file = Some(result.map_err(|e| e.to_string()))));
    assert_eq!(picker::take(rt.core_mut()), Some(Pick::Refused));
    rt.frame(0.0);
    assert!(matches!(&rt.state.file, Some(Err(why)) if why.contains("Android")));
}