use std::io::{Read, Write};
use std::path::Path;

use super::super::{DialogAnswer, FileDialogKind, FileDialogOptions};
use super::FileHandle;
use crate::{Env, Runtime, el};

#[derive(Default)]
struct S {
    one: Option<Result<Option<FileHandle>, String>>,
    many: Option<Result<Option<Vec<FileHandle>>, String>>,
}

fn rig() -> Runtime<S> {
    let mut rt = Runtime::new(S::default(), |_, _| el().size(40.0, 20.0), (40.0, 20.0), Env::default());
    rt.frame(0.0);
    rt
}

#[test]
fn a_picked_file_is_a_handle_with_its_name_and_where_it_has_one_its_path() {
    let mut rt = rig();
    rt.update(|_, tx| tx.pick_file(FileDialogOptions::default(), |s: &mut S, result, _| s.one = Some(result.map_err(|e| e.0))));
    assert_eq!(rt.file_dialog().map(|(kind, _)| kind), Some(FileDialogKind::Open));
    assert!(rt.file_dialog_handles(), "the host may answer with a document that has no path");
    rt.resolve_file_dialog(Some("/fixture/notes.txt".into())).unwrap();
    rt.frame(0.0);
    let handle = rt.state.one.take().unwrap().unwrap().expect("a file");
    assert_eq!((handle.name(), handle.path()), ("notes.txt", Some(Path::new("/fixture/notes.txt"))));

    rt.update(|_, tx| tx.pick_file(FileDialogOptions::default(), |s: &mut S, result, _| s.one = Some(result.map_err(|e| e.0))));
    rt.resolve_file_dialog(None).unwrap();
    rt.frame(0.0);
    assert_eq!(rt.state.one, Some(Ok(None)), "cancelled");
}

#[test]
fn several_files_and_a_file_to_save() {
    let mut rt = rig();
    rt.update(|_, tx| tx.pick_files(FileDialogOptions::default(), |s: &mut S, result, _| s.many = Some(result.map_err(|e| e.0))));
    assert!(rt.file_dialog_multiple() && rt.file_dialog_handles());
    rt.resolve_file_dialog_files(Some(vec!["/fixture/a.csv".into(), "/fixture/b.csv".into()])).unwrap();
    rt.frame(0.0);
    let names: Vec<String> = rt.state.many.take().unwrap().unwrap().unwrap().iter().map(|h| h.name().to_owned()).collect();
    assert_eq!(names, ["a.csv", "b.csv"]);

    rt.update(|_, tx| tx.pick_save_file(FileDialogOptions::default(), |s: &mut S, result, _| s.one = Some(result.map_err(|e| e.0))));
    assert_eq!(rt.file_dialog().map(|(kind, _)| kind), Some(FileDialogKind::Save));
    rt.resolve_file_dialog(Some("/fixture/out.json".into())).unwrap();
    rt.frame(0.0);
    assert_eq!(rt.state.one.take().unwrap().unwrap().unwrap().name(), "out.json");
}

#[test]
fn a_dialog_asked_for_a_path_does_not_say_it_takes_handles() {
    let mut rt = rig();
    rt.update(|_, tx| tx.open_file(FileDialogOptions::default(), |_: &mut S, _, _| {}));
    assert!(rt.file_dialog().is_some() && !rt.file_dialog_handles());
}

#[test]
fn a_handle_writes_and_reads_its_file() {
    let path = std::env::temp_dir().join(format!("rune-file-handle-{}.txt", std::process::id()));
    let handle = FileHandle::from_path(&path);
    handle.writer().unwrap().write_all(b"picked and written").unwrap();
    let mut text = String::new();
    handle.reader().unwrap().read_to_string(&mut text).unwrap();
    std::fs::remove_file(&path).ok();
    assert_eq!(text, "picked and written");
}

#[test]
fn a_document_has_a_name_and_no_path() {
    let document = FileHandle::document("report.pdf", "content://docs/document/42");
    assert_eq!((document.name(), document.path()), ("report.pdf", None));
    // Only its platform opens it.
    #[cfg(not(target_os = "android"))]
    assert_eq!(document.reader().err().map(|e| e.kind()), Some(std::io::ErrorKind::Unsupported));
    // A caller that asked for paths gets none from it.
    assert_eq!(DialogAnswer::Handles(Some(vec![document.clone()])).into_single(), None);
    assert_eq!(DialogAnswer::Handles(Some(vec![document])).into_handle().map(|h| h.name().to_owned()), Some("report.pdf".into()));
}