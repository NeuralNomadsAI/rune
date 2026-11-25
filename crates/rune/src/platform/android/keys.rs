//! Android `KeyEvent`s (a hardware keyboard, or keys an input method sends) as Rune keys.

use crate::{Key, KeyEvent, Mods};

const META_SHIFT_ON: i32 = 0x1;
const META_ALT_ON: i32 = 0x2;
const META_CTRL_ON: i32 = 0x1000;
const META_META_ON: i32 = 0x10000;

/// The key for `getKeyCode() == code`, where `unicode` is `getUnicodeChar(0)` (the character
/// without modifiers, for a Ctrl shortcut) and `typed` is `getUnicodeChar()` (what it types).
/// `None` for keys Rune has no name for, which the view leaves to the system.
pub(crate) fn key(code: i32, unicode: u32, typed: u32, meta: i32) -> Option<KeyEvent> {
    let ctrl = meta & META_CTRL_ON != 0;
    let mods = Mods {
        shift: meta & META_SHIFT_ON != 0,
        cmd: ctrl,
        alt: meta & META_ALT_ON != 0,
        ctrl,
        logo: meta & META_META_ON != 0,
    };
    let key = match code {
        66 | 160 => Key::Enter,
        67 => Key::Backspace,
        112 => Key::Delete,
        61 => Key::Tab,
        111 => Key::Escape,
        21 => Key::Left,
        22 => Key::Right,
        19 => Key::Up,
        20 => Key::Down,
        122 => Key::Home,
        123 => Key::End,
        92 => Key::PageUp,
        93 => Key::PageDown,
        124 => Key::Insert,
        82 => Key::ContextMenu,
        131..=142 => Key::Function((code - 130) as u8),
        _ if ctrl => Key::Command(char::from_u32(unicode).filter(|c| !c.is_control())?.to_ascii_lowercase()),
        _ => Key::Text(char::from_u32(typed).filter(|c| !c.is_control())?.to_string()),
    };
    Some(KeyEvent { key, mods })
}
