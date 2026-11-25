//! Android system services reached through `ly.anoma.rune.RuneServices`: the clipboard and
//! links. Callable from any thread: each call attaches the thread to the VM as a daemon and
//! keeps its local references in a frame of their own.

#![allow(unsafe_code)]

use std::ptr::null_mut;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use jni_sys::{JNIEnv, JavaVM, jboolean, jbyteArray, jmethodID, jobject, jstring, jvalue};

use super::jni::java_string;

struct Services {
    vm: *mut JavaVM,
    /// A global reference to the app's `RuneServices`.
    object: jobject,
    clipboard_text: jmethodID,
    clipboard_png: jmethodID,
    set_clipboard_text: jmethodID,
    set_clipboard_png: jmethodID,
    clear_clipboard: jmethodID,
    open_url: jmethodID,
}

// SAFETY: a `JavaVM` pointer, a global reference and method IDs are valid on every thread.
unsafe impl Send for Services {}
// SAFETY: as above; nothing here is mutated after `install`.
unsafe impl Sync for Services {}

static SERVICES: OnceLock<Services> = OnceLock::new();
/// Bumped by `ClipboardManager`'s change listener: the clipboard's generation.
static CLIPBOARD_REVISION: AtomicU64 = AtomicU64::new(0);

pub(crate) fn clipboard_revision() -> u64 {
    CLIPBOARD_REVISION.load(Ordering::Acquire)
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneServices_nativeClipboardChanged(_: *mut JNIEnv, _: jobject) {
    CLIPBOARD_REVISION.fetch_add(1, Ordering::AcqRel);
}

#[unsafe(no_mangle)]
extern "system" fn Java_ly_anoma_rune_RuneServices_nativeInstall(env: *mut JNIEnv, this: jobject) {
    if SERVICES.get().is_some() { return }
    // SAFETY: JNI calls on a thread with a live `env` and `this`; every name is NUL-terminated.
    let services = unsafe {
        let mut vm: *mut JavaVM = null_mut();
        if call!(env, GetJavaVM(&mut vm)) != 0 { return }
        let class = call!(env, GetObjectClass(this));
        let method = |name: &std::ffi::CStr, sig: &std::ffi::CStr| call!(env, GetMethodID(class, name.as_ptr(), sig.as_ptr()));
        let services = Services {
            vm,
            object: null_mut(),
            clipboard_text: method(c"clipboardText", c"()Ljava/lang/String;"),
            clipboard_png: method(c"clipboardPng", c"()[B"),
            set_clipboard_text: method(c"setClipboardText", c"(Ljava/lang/String;Z)Z"),
            set_clipboard_png: method(c"setClipboardPng", c"([B)Z"),
            clear_clipboard: method(c"clearClipboard", c"()Z"),
            open_url: method(c"openUrl", c"(Ljava/lang/String;)Ljava/lang/String;"),
        };
        call!(env, DeleteLocalRef(class));
        let methods = [services.clipboard_text, services.clipboard_png, services.set_clipboard_text, services.set_clipboard_png, services.clear_clipboard, services.open_url];
        if methods.iter().any(|m| m.is_null()) {
            call!(env, ExceptionClear());
            eprintln!("rune: RuneServices is missing a method");
            return;
        }
        Services { object: call!(env, NewGlobalRef(this)), ..services }
    };
    SERVICES.set(services).ok();
}

/// Runs `f` with the services and this thread's `JNIEnv`, inside a local reference frame.
/// `None` before `RuneServices` is installed, or when the thread can't attach.
fn with<R>(f: impl FnOnce(&Services, *mut JNIEnv) -> R) -> Option<R> {
    let services = SERVICES.get()?;
    let mut env: *mut JNIEnv = null_mut();
    // SAFETY: `vm` is the process's VM; a daemon attachment needs no detach before exit, and
    // the pushed frame is popped before returning, freeing `f`'s local references.
    unsafe {
        if call!(services.vm, AttachCurrentThreadAsDaemon((&raw mut env).cast(), null_mut())) != 0 || env.is_null() { return None }
        if call!(env, PushLocalFrame(16)) != 0 { return None }
        let out = f(services, env);
        // A Java exception means the call didn't happen as asked: its null result isn't an answer.
        let threw = call!(env, ExceptionCheck()) != 0;
        if threw {
            call!(env, ExceptionDescribe());
            call!(env, ExceptionClear());
        }
        call!(env, PopLocalFrame(null_mut()));
        (!threw).then_some(out)
    }
}

/// # Safety
/// `env` is the calling thread's live `JNIEnv`.
unsafe fn new_string(env: *mut JNIEnv, s: &str) -> jstring {
    let units: Vec<u16> = s.encode_utf16().collect();
    // SAFETY: `units` is a live UTF-16 buffer of the given length.
    unsafe { call!(env, NewString(units.as_ptr(), units.len() as i32)) }
}

pub(crate) fn clipboard_text() -> Option<String> {
    with(|s, env| {
        // SAFETY: `object` and the method ID come from `install`; the result is a string or null.
        unsafe {
            let text = call!(env, CallObjectMethodA(s.object, s.clipboard_text, std::ptr::null()));
            (!text.is_null()).then(|| java_string(env, text))
        }
    })?
}

pub(crate) fn clipboard_png() -> Option<Vec<u8>> {
    with(|s, env| {
        // SAFETY: as above; the result is a byte array or null, copied whole.
        unsafe {
            let array: jbyteArray = call!(env, CallObjectMethodA(s.object, s.clipboard_png, std::ptr::null()));
            if array.is_null() { return None }
            let n = call!(env, GetArrayLength(array)).max(0) as usize;
            let mut bytes = vec![0u8; n];
            call!(env, GetByteArrayRegion(array, 0, n as i32, bytes.as_mut_ptr().cast()));
            Some(bytes)
        }
    })?
}

/// Puts `text` on the clipboard; `sensitive` keeps it out of the system's clipboard preview.
pub(crate) fn set_clipboard_text(text: &str, sensitive: bool) -> bool {
    with(|s, env| {
        // SAFETY: as above; the arguments match `(Ljava/lang/String;Z)Z`.
        unsafe {
            let args = [jvalue { l: new_string(env, text) }, jvalue { z: jboolean::from(sensitive) }];
            call!(env, CallBooleanMethodA(s.object, s.set_clipboard_text, args.as_ptr())) != 0
        }
    }).unwrap_or(false)
}

pub(crate) fn set_clipboard_png(png: &[u8]) -> bool {
    with(|s, env| {
        // SAFETY: as above; the array is as long as `png`, which is copied into it.
        unsafe {
            let array = call!(env, NewByteArray(png.len() as i32));
            if array.is_null() { return false }
            call!(env, SetByteArrayRegion(array, 0, png.len() as i32, png.as_ptr().cast()));
            call!(env, CallBooleanMethodA(s.object, s.set_clipboard_png, [jvalue { l: array }].as_ptr())) != 0
        }
    }).unwrap_or(false)
}

pub(crate) fn clear_clipboard() -> bool {
    // SAFETY: as above.
    with(|s, env| unsafe { call!(env, CallBooleanMethodA(s.object, s.clear_clipboard, std::ptr::null())) != 0 }).unwrap_or(false)
}

/// Opens `url` in the app the system picks; the error says why nothing opened.
pub(crate) fn open_url(url: &str) -> Result<(), String> {
    with(|s, env| {
        // SAFETY: as above; the result is an error message or null.
        unsafe {
            let error = call!(env, CallObjectMethodA(s.object, s.open_url, [jvalue { l: new_string(env, url) }].as_ptr()));
            if error.is_null() { Ok(()) } else { Err(java_string(env, error)) }
        }
    }).unwrap_or_else(|| Err("Android's services aren't reachable".into()))
}
