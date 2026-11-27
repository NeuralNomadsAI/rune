//! `Tx::open_file` and `Tx::open_files` through Android's document picker. It answers with
//! `content://` URIs where Rune's dialogs answer with paths, so `RuneView` copies each picked
//! document into the app's cache and the copies' paths answer the dialog. A file to save and a
//! folder have no path to give, and are refused.

use std::path::PathBuf;

use crate::file_dialog::{DialogAnswer, FileDialogError, FileDialogKind};
use crate::runtime::RuntimeCore;

/// Between the fields of [`Pick::Show`].
const UNIT: char = '\u{1f}';

/// What became of a file dialog the app asked for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    /// For `RuneView` to show: the dialog's id, 1 when several documents may be picked, and
    /// its filters' extensions (none: any document).
    Show(String),
    /// The picker can't stand in for it: answered with why, which the next frame delivers.
    Refused,
}

/// The dialog a frame asked for, once.
pub(crate) fn take(core: &mut RuntimeCore) -> Option<Pick> {
    let (id, kind, multiple, options) = core.take_file_dialog()?;
    if kind != FileDialogKind::Open {
        let what = if kind == FileDialogKind::Save { "save" } else { "folder" };
        core.answer_file_dialog(id, Err(FileDialogError(format!("Android's document picker gives no path for a {what} dialog"))));
        return Some(Pick::Refused);
    }
    let extensions: Vec<&str> = options.filters.iter().flat_map(|(_, extensions)| extensions.iter().map(String::as_str)).collect();
    Some(Pick::Show(format!("{id}{UNIT}{}{UNIT}{}", u8::from(multiple), extensions.join(","))))
}

/// The picker closed: the copies of what was picked, `None` when the user backed out, or why
/// a document could not be copied.
pub(crate) fn answer(core: &mut RuntimeCore, id: u64, picked: Result<Option<Vec<PathBuf>>, String>) {
    core.answer_file_dialog(id, picked.map(DialogAnswer::Many).map_err(FileDialogError));
}