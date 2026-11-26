//! What `RuneView` shows over selected text: a handle on each end and the text's own menu as
//! the floating toolbar. The toolbar is shut by what would leave it stale: a finger on the view,
//! a held handle, the selection collapsing. It stays while its selection only moves (the
//! keyboard coming up under it), and a released handle asks for it again, as does a finger
//! lifting from a scroll that shut it.

use crate::machine::Effects;
use crate::runtime::touch_ui::Ends;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct State {
    /// The ends `RuneView` was last told.
    shown: Option<Ends>,
    toolbar: bool,
    /// A finger on the view shut the toolbar, and may bring it back when it lifts.
    parted: bool,
}

impl State {
    /// The toolbar is open.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub(crate) fn toolbar(&self) -> bool { self.toolbar }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Event {
    /// A frame ended with the selection's ends at `ends`; `menu` when a text menu was taken to
    /// show as the toolbar.
    Frame { ends: Option<Ends>, menu: bool },
    /// A finger went down on the view, then lifted: `panned` when it scrolled, so was no tap.
    Touch,
    Lift { panned: bool },
    /// A finger took a handle, then let it go.
    Grab,
    Release,
    /// The toolbar closed by itself: an item was picked, or the system shut it.
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Effect {
    /// Place the handles, or hide them.
    Handles(Option<Ends>),
    /// Shut the toolbar and answer its menu with nothing picked.
    Shut,
    /// Ask the runtime for the selection's menu.
    Ask,
}

pub(crate) fn step(state: &mut State, event: Event) -> Effects<Effect> {
    let mut out = Effects::none();
    let shut = |state: &mut State, out: &mut Effects<Effect>| {
        if std::mem::take(&mut state.toolbar) { out.push(Effect::Shut) }
    };
    match event {
        Event::Frame { ends, menu } => {
            let collapsed = state.shown.map(|e| std::mem::discriminant(&e)) != ends.map(|e| std::mem::discriminant(&e));
            if state.shown != ends {
                state.shown = ends;
                out.push(Effect::Handles(ends));
            }
            if menu { state.toolbar = true } else if collapsed { shut(state, &mut out) }
        }
        Event::Touch => {
            state.parted = state.toolbar;
            shut(state, &mut out);
        }
        // A scroll under the selection hid its toolbar for as long as it lasted.
        Event::Lift { panned } => {
            if std::mem::take(&mut state.parted) && panned { out.push(Effect::Ask) }
        }
        Event::Grab => shut(state, &mut out),
        Event::Release => out.push(Effect::Ask),
        Event::Closed => state.toolbar = false,
    }
    out
}
