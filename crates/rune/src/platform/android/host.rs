//! One `RuneView`'s Rust side: the app registered by `run`, the lifecycle machine, and the
//! [`LayerHost`] it drives once the first window arrives.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;

use super::lifecycle::{self, Effect, Event, Insets};
use crate::app::App;
use crate::embed::{EmbedFrame, LayerHost};
use crate::runtime::Waker;
use crate::{Key, KeyEvent};

/// What `RuneView` needs from a running app, with its state type erased.
pub(super) trait Host {
    /// # Safety
    /// `window` is a live `ANativeWindow` the caller holds until [`Self::detach`].
    unsafe fn attach(&mut self, window: NonNull<c_void>, viewport: (f32, f32), scale: f32) -> Result<(), String>;
    fn detach(&mut self);
    fn resize(&mut self, viewport: (f32, f32), scale: f32);
    fn frame(&mut self) -> EmbedFrame;
    fn set_safe_area(&mut self, insets: [f32; 4]);
    fn set_dark(&mut self, dark: bool);
    /// The system back gesture: Escape when something has focus (closing what's open, or
    /// leaving a field), false to let the activity go back.
    fn back(&mut self) -> bool;
}

impl<S: 'static> Host for LayerHost<S> {
    unsafe fn attach(&mut self, window: NonNull<c_void>, viewport: (f32, f32), scale: f32) -> Result<(), String> {
        // SAFETY: forwarded from the trait method's contract.
        unsafe { self.attach_android_window(window, viewport, scale) }
    }
    fn detach(&mut self) { self.detach_surface(); }
    fn resize(&mut self, viewport: (f32, f32), scale: f32) { LayerHost::resize(self, viewport, scale); }
    fn frame(&mut self) -> EmbedFrame { LayerHost::frame(self) }
    fn set_safe_area(&mut self, insets: [f32; 4]) { self.runtime_mut().set_safe_area(insets); }
    fn set_dark(&mut self, dark: bool) { self.runtime_mut().set_system_dark(dark); }
    fn back(&mut self) -> bool {
        let rt = self.runtime_mut();
        if !rt.has_focus() { return false }
        rt.key(KeyEvent { key: Key::Escape, mods: Default::default() });
        true
    }
}

type Start = Box<dyn FnOnce(NonNull<c_void>, (f32, f32), f32, bool, Waker) -> Result<Box<dyn Host>, String>>;

thread_local! {
    static REGISTERED: RefCell<Option<Start>> = const { RefCell::new(None) };
}

/// `App::try_run` on Android: keeps the app until `RuneView` has a window to start it in.
pub(crate) fn register<S: 'static>(app: App<S>) -> Result<(), String> {
    let spec = app.spec;
    let start: Start = Box::new(move |window, viewport, scale, dark, wake| {
        // SAFETY: `View` passes the window it holds until `detach`, on the UI thread.
        let host = unsafe { LayerHost::from_android_window(window, &spec, (spec.init)(), viewport, scale, dark, wake) }?;
        Ok(Box::new(host) as Box<dyn Host>)
    });
    REGISTERED.with(|r| r.borrow_mut().replace(start));
    Ok(())
}

fn take_registered() -> Option<Start> {
    REGISTERED.with(|r| r.borrow_mut().take())
}

/// How soon `RuneView` should call [`View::frame`] again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Next {
    VSync,
    AfterMs(u32),
    Idle,
}

impl From<EmbedFrame> for Next {
    fn from(f: EmbedFrame) -> Self {
        match (f.animating, f.wake_after_secs) {
            (true, _) => Self::VSync,
            (false, Some(secs)) => Self::AfterMs((secs * 1000.0).ceil().clamp(0.0, u32::MAX as f64) as u32),
            (false, None) => Self::Idle,
        }
    }
}

pub(super) struct View {
    lifecycle: lifecycle::State,
    start: Option<Start>,
    host: Option<Box<dyn Host>>,
    density: f32,
    dark: bool,
    insets: Insets,
    wake: Waker,
}

impl View {
    /// `None` when no app called `run` from its `android_main!` function.
    pub(super) fn new(density: f32, dark: bool, wake: Waker) -> Option<Self> {
        let start = take_registered()?;
        Some(Self { lifecycle: lifecycle::State::default(), start: Some(start), host: None, density, dark, insets: Insets::default(), wake })
    }

    fn viewport(&self, px: (u32, u32)) -> (f32, f32) {
        (px.0 as f32 / self.density, px.1 as f32 / self.density)
    }

    /// Steps the lifecycle; `window` is the current `ANativeWindow` for `Attach`.
    pub(super) fn step(&mut self, event: Event, window: Option<NonNull<c_void>>) -> Next {
        let mut next = Next::Idle;
        for effect in lifecycle::step(&mut self.lifecycle, event) {
            match effect {
                Effect::Attach { px } => {
                    let Some(window) = window else { continue };
                    if let Err(error) = self.attach(window, px) {
                        eprintln!("rune: Android window refused: {error}");
                    }
                }
                Effect::Resize { px } => {
                    let viewport = self.viewport(px);
                    if let Some(host) = &mut self.host { host.resize(viewport, self.density) }
                }
                Effect::Detach => if let Some(host) = &mut self.host { host.detach() },
                Effect::Draw => if let Some(host) = &mut self.host { next = host.frame().into() },
            }
        }
        next
    }

    fn attach(&mut self, window: NonNull<c_void>, px: (u32, u32)) -> Result<(), String> {
        let viewport = self.viewport(px);
        if let Some(host) = &mut self.host {
            // SAFETY: the JNI layer holds `window` until it steps `WindowDestroyed`.
            return unsafe { host.attach(window, viewport, self.density) };
        }
        let start = self.start.take().ok_or("rune: the Android app already failed to start")?;
        let mut host = start(window, viewport, self.density, self.dark, self.wake.clone())?;
        host.set_safe_area(lifecycle::safe_area(self.insets, self.density));
        self.host = Some(host);
        Ok(())
    }

    pub(super) fn set_insets(&mut self, insets: Insets) -> Next {
        self.insets = insets;
        let area = lifecycle::safe_area(insets, self.density);
        if let Some(host) = &mut self.host { host.set_safe_area(area) }
        self.step(Event::Frame, None)
    }

    pub(super) fn set_dark(&mut self, dark: bool) -> Next {
        self.dark = dark;
        if let Some(host) = &mut self.host { host.set_dark(dark) }
        self.step(Event::Frame, None)
    }

    pub(super) fn back(&mut self) -> bool {
        self.host.as_mut().is_some_and(|host| host.back())
    }
}
