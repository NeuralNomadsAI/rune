//! The Android view's lifecycle as one state machine. The window (`SurfaceHolder`'s surface)
//! comes and goes on its own; the activity is resumed or paused on its own; frames run only
//! while both a window exists and the activity is resumed. The runtime outlives every window,
//! so a pause, a rotation or a trip to the launcher keeps the app's state.

use crate::machine::Effects;

/// Whether the activity is in the foreground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Visibility {
    Resumed,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// No window: before the first `surfaceCreated`, or after `surfaceDestroyed`.
    NoWindow { visibility: Visibility },
    /// A window of `px` device pixels the host draws into.
    Window { px: (u32, u32), visibility: Visibility },
}

impl Default for State {
    fn default() -> Self { Self::NoWindow { visibility: Visibility::Resumed } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// `surfaceCreated` followed by its first `surfaceChanged`.
    WindowCreated { px: (u32, u32) },
    /// `surfaceChanged`: a rotation, a split-screen resize or a fold.
    WindowChanged { px: (u32, u32) },
    /// `surfaceDestroyed`: the window must be released before the callback returns.
    WindowDestroyed,
    Resumed,
    Paused,
    /// A `Choreographer` frame, or new data from another thread asking for one.
    Frame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Create the surface for the current window (and the runtime, the first time).
    Attach { px: (u32, u32) },
    Resize { px: (u32, u32) },
    /// Drop the surface; the runtime stays.
    Detach,
    /// Step and present one frame.
    Draw,
}

pub(crate) fn step(state: &mut State, event: Event) -> Effects<Effect> {
    use Visibility::{Paused, Resumed};
    match (*state, event) {
        (State::NoWindow { visibility }, Event::WindowCreated { px }) | (State::Window { visibility, .. }, Event::WindowCreated { px }) => {
            *state = State::Window { px, visibility };
            let attach = Effect::Attach { px };
            if visibility == Resumed { Effects::two(attach, Effect::Draw) } else { Effects::one(attach) }
        }
        (State::Window { px: old, visibility }, Event::WindowChanged { px }) => {
            *state = State::Window { px, visibility };
            match (old == px, visibility) {
                (true, _) => Effects::none(),
                (false, Resumed) => Effects::two(Effect::Resize { px }, Effect::Draw),
                (false, Paused) => Effects::one(Effect::Resize { px }),
            }
        }
        (State::Window { visibility, .. }, Event::WindowDestroyed) => {
            *state = State::NoWindow { visibility };
            Effects::one(Effect::Detach)
        }
        (State::NoWindow { .. }, Event::Resumed) => {
            *state = State::NoWindow { visibility: Resumed };
            Effects::none()
        }
        (State::NoWindow { .. }, Event::Paused) => {
            *state = State::NoWindow { visibility: Paused };
            Effects::none()
        }
        (State::Window { px, visibility: Paused }, Event::Resumed) => {
            *state = State::Window { px, visibility: Resumed };
            Effects::one(Effect::Draw)
        }
        (State::Window { px, .. }, Event::Paused) => {
            *state = State::Window { px, visibility: Paused };
            Effects::none()
        }
        (State::Window { visibility: Resumed, .. }, Event::Frame) => Effects::one(Effect::Draw),
        // A frame with nothing to draw into, a resize of no window, or a repeated resume.
        (_, Event::Frame | Event::WindowChanged { .. } | Event::WindowDestroyed | Event::Resumed) => Effects::none(),
    }
}

/// Window insets in device pixels, `[top, right, bottom, left]`: the system bars and display
/// cutout, and the on-screen keyboard (`WindowInsets.Type.ime()`), kept apart.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Insets {
    pub(crate) bars: [f32; 4],
    pub(crate) ime: [f32; 4],
}

/// The safe area in points. The keyboard covers the bottom of the window without resizing it
/// (the activity asks for `adjustNothing`), so a keyboard opening in portrait never reads as a
/// wide, short window: it only raises the bottom inset.
pub(crate) fn safe_area(insets: Insets, density: f32) -> [f32; 4] {
    let density = if density.is_finite() && density > 0.0 { density } else { 1.0 };
    std::array::from_fn(|i| insets.bars[i].max(insets.ime[i]).max(0.0) / density)
}
