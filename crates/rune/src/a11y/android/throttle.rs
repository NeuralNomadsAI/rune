//! How often the accessibility tree's changes cross JNI to TalkBack. At rest every frame's
//! changes go at once. While a viewport scrolls (a finger, a fling), each frame moves every
//! visible node, and diffing the tree and raising its events at 120 Hz would take the UI
//! thread from the scroll: changes go at most every [`SCROLL_INTERVAL`], and at once on the
//! frame the scroll rests.

use crate::machine::Effects;

/// Android's own interval for recurring accessibility events
/// (`ViewConfiguration.getSendRecurringAccessibilityEventsInterval()`), in seconds.
pub(crate) const SCROLL_INTERVAL: f64 = 0.1;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Throttle {
    /// Nothing held: the next frame syncs.
    #[default]
    Open,
    /// A scroll synced lately: frames before `due` send nothing while it keeps moving.
    Held { due: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Event {
    /// A frame ended at `now` (seconds on the host's clock), with a viewport `scrolling` or not.
    Frame { now: f64, scrolling: bool },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Effect {
    /// Send the tree's changes now.
    Sync,
    /// Run a frame at this time, so held changes go even if the scroll stops drawing.
    WakeAt(f64),
}

pub(crate) fn step(state: &mut Throttle, event: Event) -> Effects<Effect> {
    let Event::Frame { now, scrolling } = event;
    match (*state, scrolling) {
        (_, false) => {
            *state = Throttle::Open;
            Effects::one(Effect::Sync)
        }
        (Throttle::Held { due }, true) if now < due => Effects::none(),
        (_, true) => {
            let due = now + SCROLL_INTERVAL;
            *state = Throttle::Held { due };
            Effects::two(Effect::Sync, Effect::WakeAt(due))
        }
    }
}
