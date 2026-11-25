//! Android: TalkBack reads the runtime's tree through `accesskit_android`'s `Adapter`, which
//! `RuneView`'s `AccessibilityNodeProvider` calls over JNI on the UI thread, where the runtime
//! lives. Tree changes are paced while a viewport scrolls (`throttle`).

#[cfg(all(feature = "accessibility", target_os = "android"))]
mod adapter;
pub(crate) mod throttle;
#[cfg(test)]
mod throttle_tests;

#[cfg(all(feature = "accessibility", target_os = "android"))]
pub(crate) use adapter::Adapter;
