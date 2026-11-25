//! Android host. A Kotlin `RuneView` (a `SurfaceView` paced by `Choreographer`, in
//! `crates/rune/android/`) calls the JNI entry points in `jni` on the UI thread, and Rune draws
//! into its `ANativeWindow` through [`LayerHost`](crate::embed::LayerHost). winit isn't used
//! on Android: an app's `cdylib` names the function that calls [`run`](crate::run) with
//! [`android_main!`](crate::android_main), and `RuneView` calls it when it's created.

#[cfg(target_os = "android")]
mod host;
#[cfg(target_os = "android")]
mod jni;
pub(crate) mod keys;
#[cfg(test)]
mod keys_tests;
pub(crate) mod lifecycle;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(target_os = "android")]
mod logcat;
pub(crate) mod touch;
#[cfg(test)]
mod touch_tests;

#[cfg(target_os = "android")]
pub(crate) use host::register;

/// winit's Android backend links its `NativeActivity` glue, which calls `android_main` when an
/// app is started through `android.app.NativeActivity`. Rune's host is `RuneView`, so that
/// glue never runs; this only satisfies its symbol.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
fn android_main(_: winit::platform::android::activity::AndroidApp) {
    eprintln!("rune: started through NativeActivity; Rune apps run in RuneView instead");
}
