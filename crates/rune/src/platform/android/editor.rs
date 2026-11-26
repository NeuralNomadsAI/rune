//! What Android's input method is told of the focused field (`EditorInfo`), so a password gets
//! no suggestions and a one-line field an action key in place of Return.

use crate::runtime::RuntimeCore;
use crate::{Ime, Key, KeyEvent, Mods};

/// The focused field's traits a keyboard lays itself out by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Editor {
    pub(crate) secure: bool,
    pub(crate) single_line: bool,
}

impl Editor {
    /// The focused field's, or none when no field takes text.
    pub(crate) fn focused(core: &RuntimeCore) -> Option<Self> {
        Some(Self { secure: core.has_secure_focus(), single_line: core.focused_input_single_line()? })
    }

    /// Packed for `RuneView.onCreateInputConnection`: bit 0 secure, bit 1 one line.
    pub(crate) fn code(self) -> i32 {
        i32::from(self.secure) | i32::from(self.single_line) << 1
    }
}

/// `InputConnection.commitText`: text typed or picked from the suggestions. A secret field
/// takes no composition (its text stays out of the input method), so there it is typed keys.
pub(crate) fn commit(core: &mut RuntimeCore, text: String) {
    if core.has_secure_focus() {
        core.key(KeyEvent { key: Key::Text(text), mods: Mods::default() });
    } else {
        core.ime(Ime::Commit(text));
    }
}
