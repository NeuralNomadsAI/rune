//! `examples/hello` on Android: the same app, started by `RuneView` through `android_main!`.

// An app that forbids unsafe code can still name its entry.
#![forbid(unsafe_code)]

#[path = "../../hello/src/main.rs"]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod hello;

rune::android_main!(hello::main);
