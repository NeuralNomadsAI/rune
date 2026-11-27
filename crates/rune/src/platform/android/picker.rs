//! `Tx::pick_file`, `Tx::pick_files` and `Tx::pick_save_file` through Android's document picker.
//! It answers with `content://` URIs, which have no path: each becomes a `FileHandle` whose
//! provider opens its document through the `ContentResolver` (`services::open_document`). A
//! dialog that wants a path (`Tx::open_file` and its kin) and a folder are refused.

use std::io::{self, Read, Write};

use crate::file_dialog::{DialogAnswer, FileDialogError, FileDialogKind, FileHandle, FileHandleProvider};
use crate::runtime::RuntimeCore;

/// A picked document, by its `content://` URI: opened anew for each read or write, which blocks.
struct Document(String);

impl Document {
    #[cfg(target_os = "android")]
    fn open(&self, write: bool) -> io::Result<std::fs::File> { super::services::open_document(&self.0, write).map_err(io::Error::other) }

    #[cfg(not(target_os = "android"))]
    fn open(&self, _write: bool) -> io::Result<std::fs::File> { Err(io::Error::new(io::ErrorKind::Unsupported, format!("{} opens on Android only", self.0))) }
}

impl FileHandleProvider for Document {
    fn reader(&self) -> io::Result<Box<dyn Read + Send>> { Ok(Box::new(self.open(false)?)) }

    fn writer(&self) -> io::Result<Box<dyn Write + Send>> { Ok(Box::new(self.open(true)?)) }
}

/// Between the fields of [`Pick::Show`].
const UNIT: char = '\u{1f}';

/// What became of a file dialog the app asked for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    /// For `RuneView` to show: the dialog's id; 0 for one document, 1 for several, 2 for one to
    /// create; its filters' extensions (none: any document); the name to create it under.
    Show(String),
    /// The picker can't stand in for it: answered with why, which the next frame delivers.
    Refused,
}

/// The dialog a frame asked for, once.
pub(crate) fn take(core: &mut RuntimeCore) -> Option<Pick> {
    let handles = core.file_dialog_handles();
    let (id, kind, multiple, options) = core.take_file_dialog()?;
    let refused = match (kind, handles) {
        (FileDialogKind::Folder, _) => Some("Android's document picker gives no path for a folder"),
        (_, false) => Some("Android's documents have no path: ask with Tx::pick_file, Tx::pick_files or Tx::pick_save_file"),
        (_, true) => None,
    };
    if let Some(why) = refused {
        core.answer_file_dialog(id, Err(FileDialogError(why.into())));
        return Some(Pick::Refused);
    }
    let mode = match (kind, multiple) {
        (FileDialogKind::Save, _) => 2,
        (_, true) => 1,
        (_, false) => 0,
    };
    let extensions: Vec<&str> = options.filters.iter().flat_map(|(_, extensions)| extensions.iter().map(String::as_str)).collect();
    let name = options.file_name.as_deref().unwrap_or_default().replace(UNIT, " ");
    Some(Pick::Show(format!("{id}{UNIT}{mode}{UNIT}{}{UNIT}{name}", extensions.join(","))))
}

/// The picker closed: each picked document's URI and name, `None` when the user backed out, or
/// why it could not show.
pub(crate) fn answer(core: &mut RuntimeCore, id: u64, picked: Result<Option<Vec<(String, String)>>, String>) {
    let handles = picked.map(|documents| documents.map(|list| list.into_iter().map(|(uri, name)| FileHandle::from_provider(name, Document(uri))).collect()));
    core.answer_file_dialog(id, handles.map(DialogAnswer::Handles).map_err(FileDialogError));
}