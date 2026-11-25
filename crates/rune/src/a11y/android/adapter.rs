//! One `RuneView`'s accessibility adapter. Every call comes from the UI thread: the provider's
//! callbacks with their own `JNIEnv`, and `service` after a frame.

#![allow(unsafe_code)]

use accesskit_android::jni::objects::{GlobalRef, JObject};
use accesskit_android::jni::{JNIEnv, JavaVM};
use accesskit_android::{Adapter as Inner, PlatformAction, QueuedEvents};

use super::super::{AccessibilityLink, Actions, Activation};
use super::throttle::{self, Effect, Event, Throttle};
use crate::runtime::RuntimeCore;

pub(crate) struct Adapter {
    link: AccessibilityLink,
    inner: Inner,
    activation: Activation,
    actions: Actions,
    throttle: Throttle,
    epoch: crate::time::Instant,
    vm: JavaVM,
    /// The `RuneView` events are raised on.
    host: GlobalRef,
}

impl Adapter {
    /// `None` when the VM refuses a reference to `host`.
    ///
    /// # Safety
    /// `env` is the calling thread's live `JNIEnv` and `host` a live `android.view.View`.
    pub(crate) unsafe fn new(env: *mut jni_sys::JNIEnv, host: jni_sys::jobject, wake: crate::Waker) -> Option<Self> {
        // SAFETY: per this function's contract.
        let (env, host) = unsafe { (JNIEnv::from_raw(env).ok()?, JObject::from_raw(host)) };
        let link = AccessibilityLink::with_wake(wake);
        Some(Self {
            activation: Activation(link.shared.clone()),
            actions: Actions(link.shared.clone()),
            link,
            inner: Inner::default(),
            throttle: Throttle::default(),
            epoch: crate::time::now(),
            vm: env.get_java_vm().ok()?,
            host: env.new_global_ref(&host).ok()?,
        })
    }

    fn raise(&self, events: Option<QueuedEvents>) {
        let Some(events) = events else { return };
        // The UI thread is attached: this only looks its environment up.
        if let Ok(mut env) = self.vm.get_env() { events.raise(&mut env, self.host.as_obj()) }
    }

    /// After a frame: runs what TalkBack asked (activation, actions), and sends the tree's
    /// changes unless a scroll holds them. Returns whether an action needs a frame, and in how
    /// many seconds held changes are due.
    pub(crate) fn service(&mut self, rt: &mut RuntimeCore, name: &str) -> (bool, Option<f64>) {
        let queued = self.link.receive(rt, name);
        let now = self.epoch.elapsed().as_secs_f64();
        let mut wake = None;
        for effect in throttle::step(&mut self.throttle, Event::Frame { now, scrolling: rt.scrolling() }) {
            match effect {
                Effect::Sync => {
                    if let Some(update) = self.link.update(rt) {
                        let events = self.inner.update_if_active(|| update);
                        self.raise(events);
                    }
                }
                Effect::WakeAt(due) => wake = Some((due - now).max(0.0)),
            }
        }
        (queued, wake)
    }

    /// `AccessibilityNodeProvider.createAccessibilityNodeInfo`; null for no such node.
    ///
    /// # Safety
    /// `env` is the calling thread's live `JNIEnv`.
    pub(crate) unsafe fn node_info(&mut self, env: *mut jni_sys::JNIEnv, id: i32) -> jni_sys::jobject {
        // SAFETY: per this function's contract.
        let Ok(mut env) = (unsafe { JNIEnv::from_raw(env) }) else { return std::ptr::null_mut() };
        self.inner.create_accessibility_node_info(&mut self.activation, &mut env, self.host.as_obj(), id).into_raw()
    }

    /// `AccessibilityNodeProvider.findFocus`.
    ///
    /// # Safety
    /// `env` is the calling thread's live `JNIEnv`.
    pub(crate) unsafe fn find_focus(&mut self, env: *mut jni_sys::JNIEnv, focus_type: i32) -> jni_sys::jobject {
        // SAFETY: per this function's contract.
        let Ok(mut env) = (unsafe { JNIEnv::from_raw(env) }) else { return std::ptr::null_mut() };
        self.inner.find_focus(&mut self.activation, &mut env, self.host.as_obj(), focus_type).into_raw()
    }

    /// `AccessibilityNodeProvider.performAction`: queues the action for the next frame.
    ///
    /// # Safety
    /// `env` is the calling thread's live `JNIEnv` and `arguments` a `Bundle` or null.
    pub(crate) unsafe fn perform_action(&mut self, env: *mut jni_sys::JNIEnv, id: i32, action: i32, arguments: jni_sys::jobject) -> bool {
        // SAFETY: per this function's contract.
        let Ok(mut env) = (unsafe { JNIEnv::from_raw(env) }) else { return false };
        // SAFETY: as above; the reference belongs to the caller's frame.
        let arguments = unsafe { JObject::from_raw(arguments) };
        let Some(action) = PlatformAction::from_java(&mut env, action, &arguments) else { return false };
        let Some(events) = self.inner.perform_action(&mut self.actions, id, &action) else { return false };
        events.raise(&mut env, self.host.as_obj());
        true
    }

    /// A hover `MotionEvent` (TalkBack's explore by touch), in the view's pixels.
    pub(crate) fn hover(&mut self, action: i32, x: f32, y: f32) -> bool {
        let events = self.inner.on_hover_event(&mut self.activation, action, x, y);
        let handled = events.is_some();
        self.raise(events);
        handled
    }
}
