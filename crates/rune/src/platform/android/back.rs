//! The system back gesture. Rune hears it as Escape, which closes what is open or leaves a
//! field; when nothing takes the key, the activity goes back.

use crate::runtime::RuntimeCore;
use crate::{Key, KeyEvent};

/// Queues the gesture's Escape. The next frame routes it, and [`taken`] then says what came of it.
pub(crate) fn press(core: &mut RuntimeCore) {
    core.take_key_consumed();
    core.key(KeyEvent { key: Key::Escape, mods: Default::default() });
}

/// Whether something took the Escape of [`press`], once a frame has routed it.
pub(crate) fn taken(core: &mut RuntimeCore) -> bool {
    core.take_key_consumed()
}