//! JNI entry points for `ly.anoma.rune.RuneView`, all called on the UI thread. Each returns
//! when to call `nativeFrame` next: -1 never (until something changes), 0 on the next
//! `Choreographer` frame, or that many milliseconds from now.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr::{NonNull, null_mut};
use std::sync::{Arc, Mutex, PoisonError};

use jni_sys::{JNIEnv, JavaVM, jboolean, jbyteArray, jfloat, jfloatArray, jint, jintArray, jlong, jmethodID, jobject, jstring};

use super::host::{Next, View};
use super::host_ime::ImeBridge;
use super::lifecycle::{Event, Insets};

unsafe extern "C" {
    /// Defined by the app's `android_main!`.
    fn rune_android_main();
}

/// `RuneView.requestFrame()`, callable from any thread until the view is dropped.
struct WakeTarget {
    vm: *mut JavaVM,
    view: Mutex<jobject>,
    method: jmethodID,
}

// SAFETY: a `JavaVM` pointer, a global reference and a method ID are valid on every thread.
unsafe impl Send for WakeTarget {}
// SAFETY: as above; the global reference is only used and deleted under its mutex.
unsafe impl Sync for WakeTarget {}

impl WakeTarget {
    fn wake(&self) {
        let view = self.view.lock().unwrap_or_else(PoisonError::into_inner);
        if view.is_null() { return }
        let mut env: *mut JNIEnv = null_mut();
        // SAFETY: `vm` is the process's VM; a daemon attachment needs no detach before exit.
        unsafe {
            if call!(self.vm, AttachCurrentThreadAsDaemon((&raw mut env).cast(), null_mut())) != 0 || env.is_null() { return }
            call!(env, CallVoidMethodA(*view, self.method, std::ptr::null()));
            if call!(env, ExceptionCheck()) != 0 { call!(env, ExceptionClear()) }
        }
    }

    fn release(&self, env: *mut JNIEnv) {
        let mut view = self.view.lock().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: `view` is this target's own global reference, deleted once.
        if !view.is_null() { unsafe { call!(env, DeleteGlobalRef(*view)) } }
        *view = null_mut();
    }
}

struct Handle {
    view: View,
    window: Option<NonNull<ndk_sys::ANativeWindow>>,
    wake: Arc<WakeTarget>,
}

impl Handle {
    fn release_window(&mut self) {
        // SAFETY: the reference `ANativeWindow_fromSurface` gave this handle, released once.
        if let Some(w) = self.window.take() { unsafe { ndk_sys::ANativeWindow_release(w.as_ptr()) } }
    }
}

fn next(n: Next) -> jlong {
    match n { Next::VSync => 0, Next::AfterMs(ms) => jlong::from(ms.max(1)), Next::Idle => -1 }
}

/// # Safety
/// `h` is 0 or a handle `nativeNew` returned and `nativeDrop` hasn't freed.
unsafe fn handle<'a>(h: jlong) -> Option<&'a mut Handle> {
    // SAFETY: per this function's contract, a non-zero `h` is a live `Box<Handle>`.
    unsafe { (h as *mut Handle).as_mut() }
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeNew(env: *mut JNIEnv, this: jobject, density: jfloat, long_press_ms: jint, dark: jboolean) -> jlong {
    super::logcat::redirect_stdio();
    // SAFETY: JNI calls on the UI thread with a live `env` and `this`.
    unsafe {
        let mut vm: *mut JavaVM = null_mut();
        if call!(env, GetJavaVM(&mut vm)) != 0 { return 0 }
        let class = call!(env, GetObjectClass(this));
        let method = call!(env, GetMethodID(class, c"requestFrame".as_ptr(), c"()V".as_ptr()));
        call!(env, DeleteLocalRef(class));
        if method.is_null() { return 0 }
        let global = call!(env, NewGlobalRef(this));
        let wake = Arc::new(WakeTarget { vm, view: Mutex::new(global), method });
        let waker: crate::runtime::Waker = { let w = wake.clone(); Arc::new(move || w.wake()) };
        #[cfg(feature = "accessibility")]
        let a11y_wake = waker.clone();
        #[cfg_attr(not(feature = "accessibility"), allow(unused_mut))]
        let mut view = match View::new(density, f64::from(long_press_ms) / 1000.0, dark != 0, waker.clone()) {
            Some(view) => view,
            None => {
                rune_android_main();
                match View::new(density, f64::from(long_press_ms) / 1000.0, dark != 0, waker) {
                    Some(view) => view,
                    None => {
                        eprintln!("rune: android_main! didn't call rune::run");
                        wake.release(env);
                        return 0;
                    }
                }
            }
        };
        #[cfg(feature = "accessibility")]
        view.set_a11y(crate::a11y::android::Adapter::new(env, this, a11y_wake));
        Box::into_raw(Box::new(Handle { view, window: None, wake })) as jlong
    }
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeSurfaceCreated(env: *mut JNIEnv, _: jobject, h: jlong, surface: jobject, width: jint, height: jint) -> jlong {
    // SAFETY: `h` comes from `nativeNew`; `surface` is the holder's live `Surface`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    if h.window.is_some() {
        // A new surface without a destroy for the old one: let go of the old one first.
        h.view.step(Event::WindowDestroyed, None);
        h.release_window();
    }
    // SAFETY: as above; the returned reference is released in `release_window`.
    h.window = NonNull::new(unsafe { ndk_sys::ANativeWindow_fromSurface(env.cast(), surface.cast()) });
    let px = (width.max(1) as u32, height.max(1) as u32);
    let window = h.window.map(NonNull::cast::<c_void>);
    next(h.view.step(Event::WindowCreated { px }, window))
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeSurfaceChanged(_: *mut JNIEnv, _: jobject, h: jlong, width: jint, height: jint) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    next(h.view.step(Event::WindowChanged { px: (width.max(1) as u32, height.max(1) as u32) }, None))
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeSurfaceDestroyed(_: *mut JNIEnv, _: jobject, h: jlong) {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return };
    h.view.step(Event::WindowDestroyed, None);
    h.release_window();
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeResumed(_: *mut JNIEnv, _: jobject, h: jlong) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    next(h.view.step(Event::Resumed, None))
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativePaused(_: *mut JNIEnv, _: jobject, h: jlong) {
    // SAFETY: `h` comes from `nativeNew`.
    if let Some(h) = unsafe { handle(h) } { h.view.step(Event::Paused, None); }
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeFrame(_: *mut JNIEnv, _: jobject, h: jlong) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    next(h.view.step(Event::Frame, None))
}

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeInsets(
    _: *mut JNIEnv, _: jobject, h: jlong,
    bar_top: jfloat, bar_right: jfloat, bar_bottom: jfloat, bar_left: jfloat,
    ime_top: jfloat, ime_right: jfloat, ime_bottom: jfloat, ime_left: jfloat,
) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    let insets = Insets { bars: [bar_top, bar_right, bar_bottom, bar_left], ime: [ime_top, ime_right, ime_bottom, ime_left] };
    next(h.view.set_insets(insets))
}

/// A Java string as Rust's, through UTF-16 (JNI's UTF-8 is Java's modified form, which splits
/// characters outside the BMP, emoji among them, into two).
///
/// # Safety
/// `s` is a live `java.lang.String` reference or null.
pub(super) unsafe fn java_string(env: *mut JNIEnv, s: jstring) -> String {
    if s.is_null() { return String::new() }
    // SAFETY: per this function's contract; the characters are released before returning.
    unsafe {
        let len = call!(env, GetStringLength(s)).max(0) as usize;
        let chars = call!(env, GetStringChars(s, null_mut()));
        if chars.is_null() { return String::new() }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(chars, len));
        call!(env, ReleaseStringChars(s, chars));
        text
    }
}

/// `nativeKey`'s answer for a key Rune has no name for, which the view leaves to the system.
const UNHANDLED: jlong = -2;

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeKey(
    _: *mut JNIEnv, _: jobject, h: jlong, code: jint, unicode: jint, typed: jint, meta: jint, repeat: jboolean,
) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return UNHANDLED };
    let Some(ev) = super::keys::key(code, unicode as u32, typed as u32, meta) else { return UNHANDLED };
    next(h.view.key(ev, repeat != 0))
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeCommit(env: *mut JNIEnv, _: jobject, h: jlong, text: jstring) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    // SAFETY: `text` is the input method's live string.
    let text = unsafe { java_string(env, text) };
    next(h.view.ime(crate::Ime::Commit(text)))
}

/// The composition so far (`setComposingText`), with the cursor at its end; empty ends it.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeCompose(env: *mut JNIEnv, _: jobject, h: jlong, text: jstring) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    // SAFETY: `text` is the input method's live string.
    let text = unsafe { java_string(env, text) };
    let end = text.len();
    next(h.view.ime(crate::Ime::Preedit(text, Some((end, end)), Vec::new())))
}

/// `finishComposingText`: the composition becomes text as it stands.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeFinishCompose(_: *mut JNIEnv, _: jobject, h: jlong) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    next(h.view.with_ime(|_, core| ImeBridge::finish_composition(core)).unwrap_or(Next::Idle))
}

/// `setComposingRegion(a, b)` in UTF-16 units of the window `nativeImeText` returned.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeComposingRegion(_: *mut JNIEnv, _: jobject, h: jlong, a: jint, b: jint) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    let (a, b) = (a.max(0) as usize, b.max(0) as usize);
    next(h.view.with_ime(|ime, core| ime.compose_region(core, a, b)).unwrap_or(Next::Idle))
}

/// `deleteSurroundingText` in UTF-16 units, or `UNHANDLED` without a window of text, for the
/// view to send Backspace and Delete instead.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeDeleteSurrounding(_: *mut JNIEnv, _: jobject, h: jlong, before: jint, after: jint) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return UNHANDLED };
    let (before, after) = (before.max(0) as usize, after.max(0) as usize);
    h.view.with_ime(|ime, core| ime.delete_surrounding(core, before, after)).map_or(UNHANDLED, next)
}

/// The window of text around the focused field's selection, or null.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeImeText(env: *mut JNIEnv, _: jobject, h: jlong) -> jstring {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(text) = (unsafe { handle(h) }).and_then(|h| h.view.ime_bridge().window().map(|w| w.text().encode_utf16().collect::<Vec<u16>>())) else { return null_mut() };
    // SAFETY: `text` is a live UTF-16 buffer of the given length.
    unsafe { call!(env, NewString(text.as_ptr(), text.len() as i32)) }
}

/// The selection in that window as `start << 32 | end`, in UTF-16 units; -1 without one.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeImeSelection(_: *mut JNIEnv, _: jobject, h: jlong) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some((a, b)) = (unsafe { handle(h) }).and_then(|h| h.view.ime_bridge().window().map(|w| w.selection_units())) else { return -1 };
    ((a as jlong) << 32) | (b as jlong & 0xffff_ffff)
}

/// `commitContent`: a keyboard image the view read and encoded as PNG.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativePastePng(env: *mut JNIEnv, _: jobject, h: jlong, png: jbyteArray) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    // SAFETY: `png` is a live byte array; the copy writes exactly its length.
    let bytes = unsafe {
        let n = call!(env, GetArrayLength(png)).max(0) as usize;
        let mut bytes = vec![0u8; n];
        call!(env, GetByteArrayRegion(png, 0, n as i32, bytes.as_mut_ptr().cast()));
        bytes
    };
    next(h.view.with_ime(|_, core| ImeBridge::paste_png(core, bytes)).unwrap_or(Next::Idle))
}

/// What the keyboard should do now (`Keyboard` as an int), once.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeTakeKeyboard(_: *mut JNIEnv, _: jobject, h: jlong) -> jint {
    // SAFETY: `h` comes from `nativeNew`.
    unsafe { handle(h) }.map_or(0, |h| h.view.take_keyboard() as jint)
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeWantsText(_: *mut JNIEnv, _: jobject, h: jlong) -> jboolean {
    // SAFETY: `h` comes from `nativeNew`.
    unsafe { handle(h) }.map_or(0, |h| jboolean::from(h.view.wants_text()))
}

/// Whether this build has an accessibility adapter, for `RuneView` to offer a node provider.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeA11yAvailable(_: *mut JNIEnv, _: jobject, _h: jlong) -> jboolean {
    #[cfg(feature = "accessibility")]
    // SAFETY: `_h` comes from `nativeNew`.
    { jboolean::from(unsafe { handle(_h) }.is_some_and(|h| h.view.a11y().is_some())) }
    #[cfg(not(feature = "accessibility"))]
    { 0 }
}

/// `AccessibilityNodeProvider.createAccessibilityNodeInfo`: an `AccessibilityNodeInfo` or null.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeA11yNodeInfo(_env: *mut JNIEnv, _: jobject, _h: jlong, _id: jint) -> jobject {
    #[cfg(feature = "accessibility")]
    // SAFETY: `_h` comes from `nativeNew`; `_env` is this call's environment.
    if let Some(a11y) = unsafe { handle(_h) }.and_then(|h| h.view.a11y()) { return unsafe { a11y.node_info(_env, _id) } }
    null_mut()
}

/// `AccessibilityNodeProvider.findFocus`: an `AccessibilityNodeInfo` or null.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeA11yFindFocus(_env: *mut JNIEnv, _: jobject, _h: jlong, _focus: jint) -> jobject {
    #[cfg(feature = "accessibility")]
    // SAFETY: `_h` comes from `nativeNew`; `_env` is this call's environment.
    if let Some(a11y) = unsafe { handle(_h) }.and_then(|h| h.view.a11y()) { return unsafe { a11y.find_focus(_env, _focus) } }
    null_mut()
}

/// `AccessibilityNodeProvider.performAction`; `arguments` is a `Bundle` or null.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeA11yPerformAction(_env: *mut JNIEnv, _: jobject, _h: jlong, _id: jint, _action: jint, _arguments: jobject) -> jboolean {
    #[cfg(feature = "accessibility")]
    // SAFETY: `_h` comes from `nativeNew`; `_env` and `_arguments` are this call's.
    if let Some(a11y) = unsafe { handle(_h) }.and_then(|h| h.view.a11y()) { return jboolean::from(unsafe { a11y.perform_action(_env, _id, _action, _arguments) }) }
    0
}

/// A hover `MotionEvent` (TalkBack's explore by touch); true when the tree took it.
#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeA11yHover(_: *mut JNIEnv, _: jobject, _h: jlong, _action: jint, _x: jfloat, _y: jfloat) -> jboolean {
    #[cfg(feature = "accessibility")]
    // SAFETY: `_h` comes from `nativeNew`.
    if let Some(a11y) = unsafe { handle(_h) }.and_then(|h| h.view.a11y()) { return jboolean::from(a11y.hover(_action, _x, _y)) }
    0
}

/// Pointers past this many in one event are dropped (Android reports at most ten).
const MAX_POINTERS: usize = 16;

#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeTouch(
    env: *mut JNIEnv, _: jobject, h: jlong, action: jint, index: jint,
    ids: jintArray, xs: jfloatArray, ys: jfloatArray, time_ms: jlong,
) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    let (mut id, mut x, mut y) = ([0; MAX_POINTERS], [0.0; MAX_POINTERS], [0.0; MAX_POINTERS]);
    // SAFETY: the three arrays are live and as long as `RuneView` made them; each copy writes
    // at most `n` elements into a buffer of `MAX_POINTERS`.
    let n = unsafe {
        let n = (call!(env, GetArrayLength(ids)).max(0) as usize).min(MAX_POINTERS);
        call!(env, GetIntArrayRegion(ids, 0, n as i32, id.as_mut_ptr()));
        call!(env, GetFloatArrayRegion(xs, 0, n as i32, x.as_mut_ptr()));
        call!(env, GetFloatArrayRegion(ys, 0, n as i32, y.as_mut_ptr()));
        n
    };
    let pointers: Vec<(i32, f32, f32)> = (0..n).map(|i| (id[i], x[i], y[i])).collect();
    next(h.view.touch(action, index.max(0) as usize, &pointers, time_ms as f64 / 1000.0))
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeDark(_: *mut JNIEnv, _: jobject, h: jlong, dark: jboolean) -> jlong {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return -1 };
    next(h.view.set_dark(dark != 0))
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeBack(_: *mut JNIEnv, _: jobject, h: jlong) -> jboolean {
    // SAFETY: `h` comes from `nativeNew`.
    let Some(h) = (unsafe { handle(h) }) else { return 0 };
    jboolean::from(h.view.back())
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneView_nativeDrop(env: *mut JNIEnv, _: jobject, h: jlong) {
    if h == 0 { return }
    // SAFETY: `h` is the `Box<Handle>` `nativeNew` leaked, freed here once.
    let mut h = unsafe { Box::from_raw(h as *mut Handle) };
    h.view.step(Event::WindowDestroyed, None);
    h.release_window();
    h.wake.release(env);
}
