//! `examples/hello` on Android: the same app, started by `RuneView` through `android_main!`.

#[path = "../../hello/src/main.rs"]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod hello;

rune::android_main!(hello::main);
