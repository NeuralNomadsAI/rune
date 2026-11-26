//! What Android's input method is told of the focused field (`EditorInfo`), so a password gets
//! no suggestions, a one-line field an action key in place of Return, and an e-mail address or
//! a number the keys for it.

use crate::runtime::RuntimeCore;
use crate::{Ime, Key, KeyEvent, KeyboardType, Mods};

/// The focused field's traits a keyboard lays itself out by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Editor {
    pub(crate) secure: bool,
    pub(crate) single_line: bool,
    pub(crate) keyboard: KeyboardType,
}

impl Editor {
    /// The focused field's, or none when no field takes text.
    pub(crate) fn focused(core: &RuntimeCore) -> Option<Self> {
        Some(Self { secure: core.has_secure_focus(), single_line: core.focused_input_single_line()?, keyboard: core.focused_keyboard_type() })
    }

    /// Packed for `RuneView.onCreateInputConnection`: bit 0 secure, bit 1 one line, and from
    /// bit 8 what the field holds.
    pub(crate) fn code(self) -> i32 {
        let keyboard = match self.keyboard {
            KeyboardType::Text => 0,
            KeyboardType::Email => 1,
            KeyboardType::Url => 2,
            KeyboardType::Number => 3,
            KeyboardType::Decimal => 4,
            KeyboardType::Phone => 5,
        };
        i32::from(self.secure) | i32::from(self.single_line) << 1 | keyboard << 8
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
