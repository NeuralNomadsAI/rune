//! One `RuneView`'s Rust side: the app registered by `run`, the lifecycle machine, and the
//! [`LayerHost`] it drives once the first window arrives.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;

use super::lifecycle::{self, Effect, Event, Insets};
use crate::app::App;
use crate::embed::{EmbedFrame, LayerHost};
use crate::input::host_events::{ImeSync, ime_sync_step};
use crate::runtime::{RuntimeCore, Waker};
use crate::{Ime, Key, KeyEvent, TouchPhase};

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
    fn set_long_press(&mut self, secs: f64);
    /// One finger at (x, y) in points, `time` in seconds.
    fn touch(&mut self, id: u64, phase: TouchPhase, x: f32, y: f32, time: f64);
    /// The system back gesture: Escape when something has focus (closing what's open, or
    /// leaving a field), false to let the activity go back.
    fn back(&mut self) -> bool;
    fn core(&mut self) -> &mut RuntimeCore;
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
    fn set_long_press(&mut self, secs: f64) { self.runtime_mut().set_touch_long_press(secs); }
    fn touch(&mut self, id: u64, phase: TouchPhase, x: f32, y: f32, time: f64) { self.runtime_mut().touch(id, phase, x, y, time); }
    fn back(&mut self) -> bool {
        let rt = self.runtime_mut();
        if !rt.has_focus() { return false }
        rt.key(KeyEvent { key: Key::Escape, mods: Default::default() });
        true
    }
    fn core(&mut self) -> &mut RuntimeCore { self.runtime_mut().core_mut() }
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

/// What the on-screen keyboard should do after a frame, for `RuneView` to carry out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Keyboard {
    #[default]
    Unchanged = 0,
    Show = 1,
    Hide = 2,
    /// The field's text changed under the input method (a new field, a reset composition):
    /// `InputMethodManager.restartInput`.
    Restart = 3,
}

pub(super) struct View {
    lifecycle: lifecycle::State,
    start: Option<Start>,
    host: Option<Box<dyn Host>>,
    density: f32,
    /// ViewConfiguration.getLongPressTimeout(), in seconds.
    long_press: f64,
    dark: bool,
    insets: Insets,
    wake: Waker,
    ime: ImeSync,
    keyboard: Keyboard,
}

impl View {
    /// `None` when no app called `run` from its `android_main!` function.
    pub(super) fn new(density: f32, long_press: f64, dark: bool, wake: Waker) -> Option<Self> {
        let start = take_registered()?;
        Some(Self { lifecycle: lifecycle::State::default(), start: Some(start), host: None, density, long_press, dark, insets: Insets::default(), wake, ime: ImeSync::default(), keyboard: Keyboard::default() })
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
                Effect::Draw => {
                    if let Some(host) = &mut self.host { next = host.frame().into() }
                    self.sync_ime();
                }
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
        host.set_long_press(self.long_press);
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

    /// One `MotionEvent`: `pointers` holds each finger's id and position in device pixels, and
    /// `time` its `getEventTime()` in seconds.
    pub(super) fn touch(&mut self, action: i32, index: usize, pointers: &[(i32, f32, f32)], time: f64) -> Next {
        let Some(host) = &mut self.host else { return Next::Idle };
        for (i, phase) in super::touch::phases(action, index, pointers.len()) {
            let (id, x, y) = pointers[i];
            host.touch(id as u64, phase, x / self.density, y / self.density, time);
        }
        self.step(Event::Frame, None)
    }

    /// Turns the runtime's input method state (a focused field wants text, or not; its
    /// composition was reset) into what the keyboard should do.
    fn sync_ime(&mut self) {
        let Some(host) = &mut self.host else { return };
        let step = ime_sync_step(host.core(), &mut self.ime, true);
        self.keyboard = match (step.set_allowed, step.cancel) {
            (Some(true), _) => Keyboard::Show,
            (Some(false), _) => Keyboard::Hide,
            (None, true) => Keyboard::Restart,
            (None, false) => self.keyboard,
        };
    }

    pub(super) fn take_keyboard(&mut self) -> Keyboard {
        std::mem::take(&mut self.keyboard)
    }

    /// Whether a focused field takes text, so `RuneView` offers an `InputConnection`.
    pub(super) fn wants_text(&self) -> bool {
        self.ime.is_enabled()
    }

    pub(super) fn key(&mut self, ev: KeyEvent, repeat: bool) -> Next {
        let Some(host) = &mut self.host else { return Next::Idle };
        if repeat { host.core().repeat_key(ev) } else { host.core().key(ev) }
        self.step(Event::Frame, None)
    }

    pub(super) fn ime(&mut self, ev: Ime) -> Next {
        let Some(host) = &mut self.host else { return Next::Idle };
        host.core().ime(ev);
        self.step(Event::Frame, None)
    }

    /// The input method's `finishComposingText`: the composition becomes text as it stands. Not
    /// `RuntimeCore::finish_composition`, which also tells the input method to drop its copy:
    /// here the input method is the one finishing, and a reset would restart it in a loop.
    pub(super) fn finish_composition(&mut self) -> Next {
        let Some(host) = &mut self.host else { return Next::Idle };
        let core = host.core();
        let Some(text) = core.focused_preedit().map(|(text, _)| text.to_owned()) else { return Next::Idle };
        core.ime(Ime::Commit(text));
        self.step(Event::Frame, None)
    }

    pub(super) fn back(&mut self) -> bool {
        self.host.as_mut().is_some_and(|host| host.back())
    }
}
