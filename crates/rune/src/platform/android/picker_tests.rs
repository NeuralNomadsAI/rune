use super::picker::{self, Pick};
use crate::{Env, FileDialogOptions, FileHandle, Runtime, el};

#[derive(Default)]
struct S {
    file: Option<Result<Option<FileHandle>, String>>,
    files: Option<Result<Option<Vec<FileHandle>>, String>>,
    path: Option<Result<Option<std::path::PathBuf>, String>>,
}

fn rig() -> Runtime<S> {
    let mut rt = Runtime::new(S::default(), |_, _| el().size(40.0, 20.0), (40.0, 20.0), Env::default());
    rt.core_mut().host.file_dialogs = true;
    rt.frame(0.0);
    rt
}

fn document(uri: &str, name: &str) -> (String, String) { (uri.to_owned(), name.to_owned()) }

#[test]
fn the_view_is_asked_for_documents_and_they_answer_the_dialog_as_handles() {
    let mut rt = rig();
    let options = FileDialogOptions::default().filter("Text", &["txt", "md"]).filter("Images", &["png"]);
    rt.update(|_, tx| tx.pick_files(options, |s: &mut S, result, _| s.files = Some(result.map_err(|e| e.to_string()))));
    assert_eq!(picker::take(rt.core_mut()), Some(Pick::Show("1\u{1f}1\u{1f}txt,md,png\u{1f}".into())), "the dialog's id, several documents, its filters' extensions");
    assert_eq!(picker::take(rt.core_mut()), None, "asked once");
    picker::answer(rt.core_mut(), 1, Ok(Some(vec![document("content://docs/1", "a.txt"), document("content://docs/2", "b.md")])));
    rt.frame(0.0);
    let files = rt.state.files.take().unwrap().unwrap().expect("two documents");
    assert_eq!(files.iter().map(|f| (f.name(), f.path())).collect::<Vec<_>>(), [("a.txt", None), ("b.md", None)]);
}

#[test]
fn one_document_a_cancel_a_failure_and_a_document_to_save() {
    let mut rt = rig();
    let ask = |rt: &mut Runtime<S>, save: bool| {
        let options = FileDialogOptions { file_name: save.then(|| "notes.txt".to_owned()), ..Default::default() };
        rt.update(move |_, tx| {
            let answer = |s: &mut S, result: crate::FileHandleResult, _: &mut crate::Tx| s.file = Some(result.map_err(|e| e.to_string()));
            if save { tx.pick_save_file(options, answer) } else { tx.pick_file(options, answer) }
        });
        match picker::take(rt.core_mut()) {
            Some(Pick::Show(ask)) => ask,
            other => panic!("no pick for the view: {other:?}"),
        }
    };
    assert_eq!(ask(&mut rt, false), "1\u{1f}0\u{1f}\u{1f}", "one document of any kind");
    picker::answer(rt.core_mut(), 1, Ok(Some(vec![document("content://docs/1", "a.txt")])));
    rt.frame(0.0);
    assert_eq!(rt.state.file.take().unwrap().unwrap().map(|f| f.name().to_owned()), Some("a.txt".into()));

    ask(&mut rt, false);
    picker::answer(rt.core_mut(), 2, Ok(None));
    rt.frame(0.0);
    assert!(matches!(rt.state.file.take(), Some(Ok(None))), "the user backed out of the picker");

    ask(&mut rt, false);
    picker::answer(rt.core_mut(), 3, Err("no activity to show the document picker".into()));
    rt.frame(0.0);
    assert!(matches!(rt.state.file.take(), Some(Err(why)) if why == "no activity to show the document picker"));

    assert_eq!(ask(&mut rt, true), "4\u{1f}2\u{1f}\u{1f}notes.txt", "a document to create, under the name asked for");
    picker::answer(rt.core_mut(), 4, Ok(Some(vec![document("content://docs/9", "notes.txt")])));
    rt.frame(0.0);
    assert_eq!(rt.state.file.take().unwrap().unwrap().map(|f| f.name().to_owned()), Some("notes.txt".into()));
}

#[test]
fn a_dialog_that_wants_a_path_and_a_folder_are_refused() {
    let mut rt = rig();
    rt.update(|_, tx| tx.open_file(FileDialogOptions::default(), |s: &mut S, result, _| s.path = Some(result.map_err(|e| e.to_string()))));
    assert_eq!(picker::take(rt.core_mut()), Some(Pick::Refused), "nothing for the view");
    rt.frame(0.0);
    assert!(matches!(&rt.state.path, Some(Err(why)) if why.contains("pick_file")), "{:?}", rt.state.path);

    rt.update(|_, tx| tx.open_folder(FileDialogOptions::default(), |s: &mut S, result, _| s.path = Some(result.map_err(|e| e.to_string()))));
    assert_eq!(picker::take(rt.core_mut()), Some(Pick::Refused));
    rt.frame(0.0);
    assert!(matches!(&rt.state.path, Some(Err(why)) if why.contains("Android")));
}