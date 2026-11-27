//! A picked file as a handle: its name, its path where the platform has one, and a way to read
//! or write it. Android's document picker and a browser's file input give documents with no
//! path, so [`Tx::pick_file`](crate::Tx::pick_file) answers with these where
//! [`Tx::open_file`](crate::Tx::open_file) answers with a `PathBuf`.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// A file the user picked with [`Tx::pick_file`](crate::Tx::pick_file),
/// [`Tx::pick_files`](crate::Tx::pick_files) or [`Tx::pick_save_file`](crate::Tx::pick_save_file).
///
/// [`Self::read`] and [`Self::write`] take or give the whole file as futures, the shape every
/// platform can answer. [`Self::reader`] and [`Self::writer`] stream it where the platform has
/// files to open (not in a browser). All of them do their work where they are called or
/// awaited: off the UI thread, as from `rune::task::spawn`. The handle is `Send` and cheap to
/// clone.
///
/// ```no_run
/// # use std::io::Read;
/// # use rune::{FileDialogOptions, FileHandle, Tx, el};
/// # struct App { picked: Option<FileHandle> }
/// # let _ = el()
/// .on_press_tx(|_: &mut App, tx: &mut Tx| {
///     tx.pick_file(FileDialogOptions::default(), |s: &mut App, result, _| {
///         if let Ok(Some(file)) = result { s.picked = Some(file); }
///     });
/// })
/// # ;
/// # fn load(file: FileHandle) -> std::io::Result<Vec<u8>> {
/// let mut bytes = Vec::new();
/// file.reader()?.read_to_end(&mut bytes)?;
/// # Ok(bytes)
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHandle {
    name: String,
    source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    Path(PathBuf),
    /// A document its platform opens by URI: Android's `content://`.
    Document(String),
}

impl FileHandle {
    /// The file at `path`, as a desktop's dialog answers.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        Self { name, source: Source::Path(path) }
    }

    /// A document named `name` that its platform opens by `uri`.
    #[cfg_attr(not(any(target_os = "android", test)), allow(dead_code))]
    pub(crate) fn document(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self { name: name.into(), source: Source::Document(uri.into()) }
    }

    /// The file's name with its extension, for showing and for telling its type.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The file's path, where the platform's picker gives one: a desktop's does, Android's
    /// document picker does not.
    pub fn path(&self) -> Option<&Path> {
        match &self.source {
            Source::Path(path) => Some(path),
            Source::Document(_) => None,
        }
    }

    /// Reads the whole file. The shape every platform can answer, a browser's file included;
    /// on a desktop and on Android the read happens inside the future, as
    /// `rfd::FileHandle::read`'s does, so await it off the UI thread.
    pub async fn read(&self) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.reader()?.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Writes the file anew with `bytes`, replacing what it held. As [`Self::read`], for
    /// every platform.
    pub async fn write(&self, bytes: &[u8]) -> io::Result<()> {
        self.writer()?.write_all(bytes)
    }

    /// Opens the file to read from its start. Blocks.
    pub fn reader(&self) -> io::Result<Box<dyn Read + Send>> {
        match &self.source {
            Source::Path(path) => Ok(Box::new(std::fs::File::open(path)?)),
            Source::Document(uri) => open_document(uri, false).map(|file| Box::new(file) as Box<dyn Read + Send>),
        }
    }

    /// Opens the file to write it anew, as after [`Tx::pick_save_file`](crate::Tx::pick_save_file):
    /// what it held is replaced. Blocks.
    pub fn writer(&self) -> io::Result<Box<dyn Write + Send>> {
        match &self.source {
            Source::Path(path) => Ok(Box::new(std::fs::File::create(path)?)),
            Source::Document(uri) => open_document(uri, true).map(|file| Box::new(file) as Box<dyn Write + Send>),
        }
    }
}

#[cfg(target_os = "android")]
fn open_document(uri: &str, write: bool) -> io::Result<std::fs::File> {
    crate::platform::android::services::open_document(uri, write).map_err(io::Error::other)
}

#[cfg(not(target_os = "android"))]
fn open_document(_: &str, _: bool) -> io::Result<std::fs::File> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "this platform opens no documents by URI"))
}

#[cfg(test)]
#[path = "handle_tests.rs"]
mod tests;