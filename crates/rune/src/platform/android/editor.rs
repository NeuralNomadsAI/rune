//! What Android's input method is told of the focused field (`EditorInfo`), so a password gets
//! no suggestions, a one-line field an action key in place of Return, and an e-mail address or
//! a number the keys for it.

use crate::runtime::RuntimeCore;
use crate::{Ime, Key, KeyEvent, InputMode, Mods};

/// The focused field's traits a keyboard lays itself out by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Editor {
    pub(crate) secure: bool,
    pub(crate) single_line: bool,
    pub(crate) keyboard: InputMode,
}

impl Editor {
    /// The focused field's, or none when no field takes text.
    pub(crate) fn focused(core: &RuntimeCore) -> Option<Self> {
        Some(Self { secure: core.has_secure_focus(), single_line: core.focused_input_single_line()?, keyboard: core.focused_input_mode() })
    }

    /// Packed for `RuneView.onCreateInputConnection`: bit 0 secure, bit 1 one line, and from
    /// bit 8 what the field holds.
    pub(crate) fn code(self) -> i32 {
        let keyboard = match self.keyboard {
            InputMode::Text => 0,
            InputMode::Email => 1,
            InputMode::Url => 2,
            InputMode::Numeric => 3,
            InputMode::Decimal => 4,
            InputMode::Tel => 5,
            InputMode::Search => 6,
            InputMode::None => 7,
        };
        i32::from(self.secure) | i32::from(self.single_line) << 1 | keyboard << 8
    }

    /// Whether the field wants the touch keyboard at all (`InputMode::None` has a keypad of
    /// the app's own).
    pub(crate) fn wants_keyboard(self) -> bool {
        self.keyboard != InputMode::None
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
