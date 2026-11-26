//! The selection side of one `RuneView`: what its handles and its toolbar should show after a
//! frame, kept until the view takes it, and what its handles and its toolbar tell the runtime.

use super::host::Host;
use super::selection::{self, Effect, Event, State};
use crate::PopupEntry;
use crate::popup_menu::Show;
use crate::runtime::touch_ui::{Ends, Handle};

/// What the toolbar should do, for `RuneView` to carry out.
pub(super) enum Toolbar {
    Shut,
    /// Each item's id and title, in order.
    Open(Vec<(String, String)>),
}

#[derive(Default)]
pub(super) struct SelectionBridge {
    state: State,
    /// The handles' new place, until the view takes it.
    handles: Option<Option<Ends>>,
    toolbar: Option<Toolbar>,
    /// The open toolbar's item ids, by the index the view answers with.
    ids: Vec<String>,
}

impl SelectionBridge {
    /// True when the runtime was asked for a menu, which its next frame brings.
    fn run(&mut self, host: &mut dyn Host, event: Event) -> bool {
        let mut asked = false;
        for effect in selection::step(&mut self.state, event) {
            match effect {
                Effect::Handles(ends) => self.handles = Some(ends),
                Effect::Shut => {
                    host.resolve_menu(None);
                    self.toolbar = Some(Toolbar::Shut);
                }
                Effect::Ask => asked |= host.open_text_menu(),
            }
        }
        asked
    }

    /// After a frame: the selection's ends as it left them, and the text menu it asked for.
    /// True when another frame is needed.
    pub(super) fn after_frame(&mut self, host: &mut dyn Host) -> bool {
        let show = host.take_text_menu();
        let ends = host.selection_ends();
        let asked = self.run(host, Event::Frame { ends, menu: show.is_some() });
        show.is_some_and(|show| self.open(host, show)) || asked
    }

    /// The menu's enabled items become the toolbar's. One with none is answered at once, and
    /// that needs a frame.
    fn open(&mut self, host: &mut dyn Host, show: Show) -> bool {
        let items: Vec<(String, String)> = show.menu.entries().iter().filter_map(|entry| match entry {
            PopupEntry::Item(item) if item.is_enabled() => Some((item.id().to_owned(), item.title().to_owned())),
            _ => None,
        }).collect();
        if items.is_empty() {
            host.resolve_menu(None);
            self.run(host, Event::Closed);
            return true;
        }
        self.ids = items.iter().map(|(id, _)| id.clone()).collect();
        self.toolbar = Some(Toolbar::Open(items));
        false
    }

    pub(super) fn touch_down(&mut self, host: &mut dyn Host) { self.run(host, Event::Touch); }
    pub(super) fn touch_up(&mut self, host: &mut dyn Host) {
        let panned = host.core().touch_panned();
        self.run(host, Event::Lift { panned });
    }

    /// A handle's finger: `phase` 0 down, 1 moved, 2 up, 3 up with no toolbar to bring. Its end
    /// goes to (x, y) and the finger is at height `finger_y`, in points.
    pub(super) fn handle(&mut self, host: &mut dyn Host, handle: Handle, phase: i32, x: f32, y: f32, finger_y: f32) {
        match phase {
            0 if host.grab_handle(handle) => {
                self.run(host, Event::Grab);
                host.drag_handle(x, y, finger_y);
            }
            1 => host.drag_handle(x, y, finger_y),
            2 => {
                host.release_handle();
                self.run(host, Event::Release);
            }
            3 => host.release_handle(),
            _ => {}
        }
    }

    /// The toolbar closed by itself, with the item at `index` picked or none (negative).
    pub(super) fn picked(&mut self, host: &mut dyn Host, index: i32) {
        if !self.state.toolbar() { return }
        let id = usize::try_from(index).ok().and_then(|i| self.ids.get(i));
        host.resolve_menu(id.map(String::as_str));
        self.run(host, Event::Closed);
    }

    pub(super) fn take_handles(&mut self) -> Option<Option<Ends>> { self.handles.take() }
    pub(super) fn take_toolbar(&mut self) -> Option<Toolbar> { self.toolbar.take() }
}
