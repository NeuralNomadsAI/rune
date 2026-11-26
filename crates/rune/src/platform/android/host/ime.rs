//! The input method side of one `RuneView`: whether a field takes text, what the keyboard
//! should do after a frame, and the window of text the input method was last shown.

use super::super::editor::Editor;
use super::super::ime_window::ImeWindow;
use crate::input::host_events::{ImeSync, ime_sync_state};
use crate::runtime::RuntimeCore;
use crate::{ClipboardContent, Ime};

/// What the keyboard should do after a frame, for `RuneView` to carry out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::platform::android) enum Keyboard {
    #[default]
    Unchanged = 0,
    Show = 1,
    Hide = 2,
    /// A new field, kind of field or paragraph, or a reset composition:
    /// `InputMethodManager.restartInput`.
    Restart = 3,
    /// The selection or the text around it changed: `InputMethodManager.updateSelection`.
    Select = 4,
}

impl Keyboard {
    /// A command still waiting, then `next`: showing, hiding or restarting covers a selection.
    fn then(self, next: Self) -> Self {
        match (self, next) {
            (_, Self::Unchanged) => self,
            (Self::Unchanged | Self::Select, Self::Select) => Self::Select,
            (_, Self::Select) => self,
            (_, next) => next,
        }
    }
}

#[derive(Default)]
pub(in crate::platform::android) struct ImeBridge {
    sync: ImeSync,
    keyboard: Keyboard,
    /// The last window of text shown, kept through a composition (Rune shows none then).
    window: Option<ImeWindow>,
    /// What the keyboard was last told of the focused field.
    editor: Option<Editor>,
    /// A finger tapped a field that takes text: the keyboard comes back if the user had put
    /// it away, the field having kept its focus.
    reveal: bool,
}

impl ImeBridge {
    pub(in crate::platform::android) fn after_frame(&mut self, core: &mut RuntimeCore) {
        let (reset, area) = ime_sync_state(core);
        let editor = Editor::focused(core);
        // A secret field has no input method area, as it takes no composition; it takes the
        // keyboard all the same, which on a phone is the only one, told it is a password's.
        let area = area.or(editor.filter(|e| e.secure).map(|_| [0.0; 4]));
        let step = self.sync.step(reset, area, false, true, core.window_scale());
        let window = core.ime_text().map(ImeWindow::new);
        let next = match (step.set_allowed, step.cancel, &self.window, &window) {
            (Some(true), ..) => Keyboard::Show,
            (Some(false), ..) => Keyboard::Hide,
            (None, true, ..) => Keyboard::Restart,
            // Focus went from one kind of field to another with the keyboard up.
            (None, false, ..) if self.sync.is_enabled() && editor != self.editor => Keyboard::Restart,
            (None, false, Some(old), Some(new)) if !old.same_origin(new) => Keyboard::Restart,
            (None, false, Some(old), Some(new)) if old != new => Keyboard::Select,
            _ => Keyboard::Unchanged,
        };
        self.keyboard = self.keyboard.then(next);
        self.editor = editor;
        self.reveal |= core.take_field_tap() && self.sync.is_enabled();
        if !self.sync.is_enabled() {
            self.window = None;
        } else if window.is_some() {
            self.window = window;
        }
    }

    pub(in crate::platform::android) fn take_keyboard(&mut self) -> Keyboard {
        std::mem::take(&mut self.keyboard)
    }

    /// Whether to show the keyboard as it is (`InputMethodManager.showSoftInput`), once.
    pub(in crate::platform::android) fn take_reveal(&mut self) -> bool {
        std::mem::take(&mut self.reveal)
    }

    pub(in crate::platform::android) fn wants_text(&self) -> bool {
        self.sync.is_enabled()
    }

    /// The focused field's kind, for `EditorInfo`.
    pub(in crate::platform::android) fn editor(&self) -> Option<Editor> {
        self.editor
    }

    pub(in crate::platform::android) fn window(&self) -> Option<&ImeWindow> {
        self.window.as_ref()
    }

    /// `setComposingRegion(a, b)`: the text there becomes the composition the next
    /// `setComposingText` replaces. False when it leaves the window.
    pub(in crate::platform::android) fn compose_region(&self, core: &mut RuntimeCore, a: usize, b: usize) -> bool {
        let Some(range) = self.window.as_ref().and_then(|w| w.field_range(a, b)) else { return false };
        core.ime(Ime::Select(range));
        true
    }

    /// `deleteSurroundingText(before, after)` in UTF-16 units. False without a window, for the
    /// caller to fall back to Backspace and Delete keys.
    pub(in crate::platform::android) fn delete_surrounding(&self, core: &mut RuntimeCore, before: usize, after: usize) -> bool {
        let Some(window) = &self.window else { return false };
        for range in window.surrounding(before, after) {
            core.ime(Ime::Select(range));
            core.ime(Ime::Commit(String::new()));
        }
        true
    }

    /// `finishComposingText`: the composition becomes text as it stands. Not
    /// `RuntimeCore::finish_composition`, which also tells the input method to drop its copy:
    /// here the input method is the one finishing, and a reset would restart it in a loop.
    pub(in crate::platform::android) fn finish_composition(core: &mut RuntimeCore) -> bool {
        let Some(text) = core.focused_preedit().map(|(text, _)| text.to_owned()) else { return false };
        core.ime(Ime::Commit(text));
        true
    }

    /// `commitContent`: a keyboard's image (a sticker, a GIF's first frame) pasted as PNG.
    pub(in crate::platform::android) fn paste_png(core: &mut RuntimeCore, png: Vec<u8>) -> bool {
        let Some(image) = crate::clipboard::ClipboardImage::from_png(png) else { return false };
        core.paste_content(ClipboardContent::new(String::new()).with_image(image));
        true
    }
}
