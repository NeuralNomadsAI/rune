//! Android's clipboard (`ClipboardManager`, through `platform::android::services`): text and
//! one image. HTML, RTF and files aren't written or read.

use super::*;

/// What one write puts on the clipboard: Android holds one clip.
#[derive(Debug, PartialEq)]
pub(super) enum Form<'a> {
    Png(&'a [u8]),
    /// `sensitive` keeps a secret out of the system's clipboard preview.
    Text { text: &'a str, sensitive: bool },
}

/// The image when the write has one and isn't a secret, else its text.
pub(super) fn form(copy: &ClipboardWrite) -> Form<'_> {
    match (copy.content.image(), copy.is_secret()) {
        (Some(image), false) => Form::Png(image.png()),
        _ => Form::Text { text: copy.content.text(), sensitive: copy.is_secret() },
    }
}

#[cfg(target_os = "android")]
pub(super) struct Board;

#[cfg(target_os = "android")]
impl Backend for Board {
    fn write(&mut self, copy: &ClipboardWrite) -> Result<(), String> {
        use crate::platform::android::services;
        let written = match form(copy) {
            Form::Png(png) => services::set_clipboard_png(png),
            Form::Text { text, sensitive } => services::set_clipboard_text(text, sensitive),
        };
        if written { Ok(()) } else { Err("the clipboard refused the write".into()) }
    }
    fn text(&mut self) -> Option<Zeroizing<String>> {
        crate::platform::android::services::clipboard_text().map(Zeroizing::new)
    }
    fn content(&mut self, all: bool) -> Option<ClipboardContent> {
        let text = self.text();
        let image = all.then(crate::platform::android::services::clipboard_png).flatten().and_then(ClipboardImage::from_png);
        if text.is_none() && image.is_none() { return None }
        let content = ClipboardContent::from(text.unwrap_or_default());
        Some(match image { Some(image) => content.with_image(image), None => content })
    }
    fn revision(&self) -> Option<u64> { Some(crate::platform::android::services::clipboard_revision()) }
    fn clear(&mut self) -> Result<(), String> {
        if crate::platform::android::services::clear_clipboard() { Ok(()) } else { Err("the clipboard refused to clear".into()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_write_copies_the_image_and_a_text_write_the_text() {
        let image = ClipboardImage::from_rgba(1, 1, &[1, 2, 3, 255]).unwrap();
        let with_image = ClipboardWrite::new(ClipboardContent::new("alt").with_image(image.clone()));
        assert_eq!(form(&with_image), Form::Png(image.png()));
        let text = ClipboardWrite::new(ClipboardContent::new("hello"));
        assert_eq!(form(&text), Form::Text { text: "hello", sensitive: false });
    }

    #[test]
    fn a_secret_is_text_marked_sensitive() {
        let (secret, _copy) = ClipboardWrite::secret("hunter2".into(), Duration::from_secs(30));
        assert_eq!(form(&secret), Form::Text { text: "hunter2", sensitive: true });
    }
}
