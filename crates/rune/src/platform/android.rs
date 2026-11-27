//! Android host. A Kotlin `RuneView` (a `SurfaceView` paced by `Choreographer`, in
//! `crates/rune/android/`) calls the JNI entry points in `jni` on the UI thread, and Rune draws
//! into its `ANativeWindow` through [`LayerHost`](crate::embed::LayerHost). winit isn't used
//! on Android: an app's `cdylib` names the function that calls [`run`](crate::run) with
//! [`android_main!`](crate::android_main), and `RuneView` calls it when it's created.

/// Calls an entry of a JNI function table, which the VM always fills.
#[cfg(target_os = "android")]
macro_rules! call {
    ($table:expr, $f:ident($($arg:expr),*)) => {
        match (**$table).$f {
            Some(f) => f($table $(, $arg)*),
            None => unreachable!(concat!("JNI table without ", stringify!($f))),
        }
    };
}

pub(crate) mod editor;
#[cfg(test)]
mod editor_tests;
pub(crate) mod haptics;
#[cfg(test)]
mod haptics_tests;
#[cfg(target_os = "android")]
mod host;
pub(crate) mod ime_window;
#[cfg(test)]
mod ime_window_tests;
#[cfg(target_os = "android")]
mod jni;
#[cfg(target_os = "android")]
mod jni_selection;
pub(crate) mod keys;
#[cfg(test)]
mod keys_tests;
pub(crate) mod lifecycle;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(target_os = "android")]
mod logcat;
pub(crate) mod picker;
#[cfg(test)]
mod picker_tests;
pub(crate) mod selection;
#[cfg(test)]
mod selection_tests;
#[cfg(target_os = "android")]
pub(crate) mod services;
pub(crate) mod touch;
#[cfg(test)]
mod touch_tests;

#[cfg(target_os = "android")]
impl<S: 'static> crate::App<S> {
    /// On Android the activity owns the loop: `run` hands the app to `RuneView`, which starts
    /// it in its first window and returns at once.
    pub(crate) fn try_run_start(self) -> Result<(), crate::tooling::StartError> {
        host::register(self).map_err(crate::tooling::StartError::Window)
    }
}

/// winit's Android backend links its `NativeActivity` glue, which calls `android_main` when an
/// app is started through `android.app.NativeActivity`. Rune's host is `RuneView`, so that
/// glue never runs; this only satisfies its symbol.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(_: winit::platform::android::activity::AndroidApp) {
    eprintln!("rune: started through NativeActivity; Rune apps run in RuneView instead");
}
