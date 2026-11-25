//! JNI entry points for `ly.anoma.rune.RuneView`, all called on the UI thread. Each returns
//! when to call `nativeFrame` next: -1 never (until something changes), 0 on the next
//! `Choreographer` frame, or that many milliseconds from now.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr::{NonNull, null_mut};
use std::sync::{Arc, Mutex, PoisonError};

use jni_sys::{JNIEnv, JavaVM, jboolean, jfloat, jfloatArray, jint, jintArray, jlong, jmethodID, jobject};

use super::host::{Next, View};
use super::lifecycle::{Event, Insets};

/// Calls an entry of a JNI function table, which the VM always fills.
macro_rules! call {
    ($table:expr, $f:ident($($arg:expr),*)) => {
        match (**$table).$f {
            Some(f) => f($table $(, $arg)*),
            None => unreachable!(concat!("JNI table without ", stringify!($f))),
        }
    };
}

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
        let view = match View::new(density, f64::from(long_press_ms) / 1000.0, dark != 0, waker.clone()) {
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
