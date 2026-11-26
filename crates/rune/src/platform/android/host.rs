//! One `RuneView`'s Rust side: the app registered by `run`, the lifecycle machine, and the
//! [`LayerHost`] it drives once the first window arrives.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;

pub(super) mod ime;
pub(super) mod selection;

use super::lifecycle::{self, Effect, Event, Insets};
use crate::app::App;
use crate::embed::{EmbedFrame, LayerHost};
use ime::{ImeBridge, Keyboard};
use selection::{SelectionBridge, Toolbar};
use crate::popup_menu::Show;
use crate::runtime::touch_ui::{Ends, Handle};
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
    /// The selection's handles and toolbar (`runtime/selection/touch_ui.rs`).
    fn selection_ends(&self) -> Option<Ends>;
    fn take_text_menu(&mut self) -> Option<Show>;
    fn open_text_menu(&mut self) -> bool;
    fn resolve_menu(&mut self, picked: Option<&str>);
    fn grab_handle(&mut self, handle: Handle) -> bool;
    fn drag_handle(&mut self, x: f32, y: f32, finger_y: f32);
    fn release_handle(&mut self);
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
    fn selection_ends(&self) -> Option<Ends> { self.runtime().selection_ends() }
    fn take_text_menu(&mut self) -> Option<Show> { self.runtime_mut().take_text_menu() }
    fn open_text_menu(&mut self) -> bool { self.runtime_mut().open_text_menu() }
    fn resolve_menu(&mut self, picked: Option<&str>) { self.runtime_mut().resolve_popup(picked).ok(); }
    fn grab_handle(&mut self, handle: Handle) -> bool { self.runtime_mut().grab_handle(handle) }
    fn drag_handle(&mut self, x: f32, y: f32, finger_y: f32) { self.runtime_mut().drag_handle(x, y, finger_y); }
    fn release_handle(&mut self) { self.runtime_mut().release_handle(); }
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

impl Next {
    /// This, or sooner: the next frame when `now`, else within `secs` when given.
    #[cfg_attr(not(feature = "accessibility"), allow(dead_code))]
    fn sooner(self, now: bool, secs: Option<f64>) -> Self {
        let ms = |secs: f64| (secs * 1000.0).ceil().clamp(1.0, u32::MAX as f64) as u32;
        match (self, now, secs) {
            (Self::VSync, ..) | (_, true, _) => Self::VSync,
            (Self::AfterMs(a), false, Some(s)) => Self::AfterMs(a.min(ms(s))),
            (Self::Idle, false, Some(s)) => Self::AfterMs(ms(s)),
            (other, false, None) => other,
        }
    }
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
    /// ViewConfiguration.getLongPressTimeout(), in seconds.
    long_press: f64,
    dark: bool,
    insets: Insets,
    wake: Waker,
    ime: ImeBridge,
    selection: SelectionBridge,
    clipboard: crate::clipboard::Client,
    #[cfg(feature = "accessibility")]
    a11y: Option<crate::a11y::android::Adapter>,
}

impl View {
    /// `None` when no app called `run` from its `android_main!` function.
    pub(super) fn new(density: f32, long_press: f64, dark: bool, wake: Waker) -> Option<Self> {
        let start = take_registered()?;
        let mut clipboard = crate::clipboard::Client::default();
        clipboard.connect(wake.clone());
        Some(Self { lifecycle: lifecycle::State::default(), start: Some(start), host: None, density, long_press, dark, insets: Insets::default(), wake, ime: ImeBridge::default(), selection: SelectionBridge::default(), clipboard, #[cfg(feature = "accessibility")] a11y: None })
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
                    if let Some(host) = &mut self.host {
                        // Clipboard replies land before the frame; what it copies or asks for leaves after.
                        self.clipboard.receive_core(host.core());
                        next = host.frame().into();
                        self.clipboard.flush_core(host.core());
                        if self.selection.after_frame(host.as_mut()) { next = Next::VSync }
                        #[cfg(feature = "accessibility")]
                        if let Some(a11y) = &mut self.a11y {
                            let (queued, wake) = a11y.service(host.core(), "");
                            next = next.sooner(queued, wake);
                        }
                    }
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
        // Links go to the system (Intent.ACTION_VIEW), with its answer.
        host.core().host.open_urls_natively = true;
        // `RuneView` shows text's menus as the selection toolbar, and handles on its ends.
        host.core().host.text_menus = true;
        self.host = Some(host);
        Ok(())
    }

    pub(super) fn set_insets(&mut self, insets: Insets) -> Next {
        self.insets = insets;
        let area = lifecycle::safe_area(insets, self.density);
        // The keyboard covers the window without resizing it: the runtime keeps the focused
        // field's caret in what shows above it, as it does for a browser's keyboard.
        let band = self.lifecycle.px().and_then(|px| lifecycle::visible_band(insets, px, self.density));
        if let Some(host) = &mut self.host {
            host.set_safe_area(area);
            let core = host.core();
            core.set_visual_viewport_y(band);
            if band.is_some() { core.request_caret_follow() }
        }
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
        if action == super::touch::ACTION_DOWN { self.selection.touch_down(host.as_mut()) }
        for (i, phase) in super::touch::phases(action, index, pointers.len()) {
            let (id, x, y) = pointers[i];
            host.touch(id as u64, phase, x / self.density, y / self.density, time);
        }
        if matches!(action, super::touch::ACTION_UP | super::touch::ACTION_CANCEL) { self.selection.touch_up(host.as_mut()) }
        self.step(Event::Frame, None)
    }

    /// A selection handle's finger and its `phase` (`SelectionBridge::handle`): its end goes to
    /// (x, y) and the finger is at height `finger_y`, in device pixels.
    pub(super) fn handle(&mut self, handle: Handle, phase: i32, x: f32, y: f32, finger_y: f32) -> Next {
        let Some(host) = &mut self.host else { return Next::Idle };
        self.selection.handle(host.as_mut(), handle, phase, x / self.density, y / self.density, finger_y / self.density);
        self.step(Event::Frame, None)
    }

    /// The selection toolbar closed by itself, with the item at `index` picked or none (negative).
    pub(super) fn toolbar_picked(&mut self, index: i32) -> Next {
        let Some(host) = &mut self.host else { return Next::Idle };
        self.selection.picked(host.as_mut(), index);
        self.step(Event::Frame, None)
    }

    /// The handles' new place in device pixels, when it changed since the last call.
    pub(super) fn take_handles(&mut self) -> Option<Option<Ends>> {
        let px = |e: crate::runtime::touch_ui::End| crate::runtime::touch_ui::End { x: e.x * self.density, top: e.top * self.density, bottom: e.bottom * self.density, shown: e.shown };
        Some(self.selection.take_handles()?.map(|ends| match ends {
            Ends::Caret(at) => Ends::Caret(px(at)),
            Ends::Range { start, end } => Ends::Range { start: px(start), end: px(end) },
        }))
    }

    pub(super) fn take_toolbar(&mut self) -> Option<Toolbar> {
        self.selection.take_toolbar()
    }

    fn sync_ime(&mut self) {
        if let Some(host) = &mut self.host { self.ime.after_frame(host.core()) }
    }

    pub(super) fn take_keyboard(&mut self) -> Keyboard {
        self.ime.take_keyboard()
    }

    /// Whether a focused field takes text, so `RuneView` offers an `InputConnection`.
    pub(super) fn wants_text(&self) -> bool {
        self.ime.wants_text()
    }

    /// `commitText`: text the keyboard typed or the user picked from its suggestions.
    pub(super) fn commit(&mut self, text: String) -> Next {
        let Some(host) = &mut self.host else { return Next::Idle };
        super::editor::commit(host.core(), text);
        self.step(Event::Frame, None)
    }

    pub(super) fn ime_bridge(&self) -> &ImeBridge {
        &self.ime
    }

    /// Runs `f` on the runtime with the IME bridge, then a frame when it says something changed.
    pub(super) fn with_ime(&mut self, f: impl FnOnce(&ImeBridge, &mut RuntimeCore) -> bool) -> Option<Next> {
        let host = self.host.as_mut()?;
        f(&self.ime, host.core()).then(|| self.step(Event::Frame, None))
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

    #[cfg(feature = "accessibility")]
    pub(super) fn set_a11y(&mut self, adapter: Option<crate::a11y::android::Adapter>) {
        self.a11y = adapter;
    }

    #[cfg(feature = "accessibility")]
    pub(super) fn a11y(&mut self) -> Option<&mut crate::a11y::android::Adapter> {
        self.a11y.as_mut()
    }

    pub(super) fn back(&mut self) -> bool {
        self.host.as_mut().is_some_and(|host| host.back())
    }
}
