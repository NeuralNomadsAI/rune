//! JNI entry points for `RuneView`'s selection handles and toolbar, on the UI thread.

#![allow(unsafe_code)]

use std::ptr::null_mut;

use jni_sys::{JNIEnv, jfloat, jfloatArray, jint, jlong, jobject, jstring};

use super::host::selection::Toolbar;
use super::jni::{handle, next};
use crate::runtime::touch_ui::{End, Ends, Handle};

/// Between an item's id and its title, and between items, in `nativeTakeToolbar`'s string.
const UNIT: char = '\u{1f}';
const RECORD: char = '\u{1e}';

/// The handles' new place when it changed since the last call, else null: the kind (0 none,
/// 1 a caret, 2 a range), then each end's x, top, bottom (device pixels) and whether it shows.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeTakeHandles(env: *mut JNIEnv, _: jobject, h: jlong) -> jfloatArray {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(ends) = (unsafe { handle(h) }).and_then(|h| h.view.take_handles()) else { return null_mut() };
    let end = |e: End| [e.x, e.top, e.bottom, f32::from(u8::from(e.shown))];
    let (kind, a, b) = match ends {
        None => (0.0, [0.0; 4], [0.0; 4]),
        Some(Ends::Caret(at)) => (1.0, end(at), end(at)),
        Some(Ends::Range { start, end: last }) => (2.0, end(start), end(last)),
    };
    let values = [kind, a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]];
    // SAFETY: a new array of `values.len()` floats, filled from a buffer of that length.
    unsafe {
        let array = call!(env, NewFloatArray(values.len() as i32));
        if !array.is_null() { call!(env, SetFloatArrayRegion(array, 0, values.len() as i32, values.as_ptr())) }
        array
    }
}

/// What the toolbar should do when that changed since the last call, else null: an empty
/// string to shut it, or its items to open it, as `id UNIT title` joined by `RECORD`.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeTakeToolbar(env: *mut JNIEnv, _: jobject, h: jlong) -> jstring {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(toolbar) = (unsafe { handle(h) }).and_then(|h| h.view.take_toolbar()) else { return null_mut() };
    let clean = |s: &str| s.replace([UNIT, RECORD], " ");
    let text = match toolbar {
        Toolbar::Shut => String::new(),
        Toolbar::Open(items) => items.iter().map(|(id, title)| format!("{}{UNIT}{}", clean(id), clean(title))).collect::<Vec<_>>().join(&RECORD.to_string()),
    };
    let units: Vec<u16> = text.encode_utf16().collect();
    // SAFETY: `units` is a live UTF-16 buffer of the given length.
    unsafe { call!(env, NewString(units.as_ptr(), units.len() as i32)) }
}

/// A handle's finger: `which` 0 the start, 1 the end, 2 the caret; `phase` 0 down, 1 moved,
/// 2 up, 3 up with no toolbar to bring; (x, y) where its end should go and `finger_y` the
/// finger's height, in the view's device pixels.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeHandle(_: *mut JNIEnv, _: jobject, h: jlong, which: jint, phase: jint, x: jfloat, y: jfloat, finger_y: jfloat) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    let which = match which {
        0 => Handle::Start,
        1 => Handle::End,
        _ => Handle::Caret,
    };
    next(h.view.handle(which, phase, x, y, finger_y))
}

/// The toolbar closed by itself: the item at `index` was picked, or none (negative).
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeToolbarPicked(_: *mut JNIEnv, _: jobject, h: jlong, index: jint) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    next(h.view.toolbar_picked(index))
}
