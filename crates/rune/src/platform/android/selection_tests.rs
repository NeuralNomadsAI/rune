use super::selection::{Effect, Event, State, step};
use crate::runtime::touch_ui::{End, Ends};

fn end(x: f32, y: f32) -> End { End { x, top: y, bottom: y + 20.0, shown: true } }
fn range(y: f32) -> Option<Ends> { Some(Ends::Range { start: end(10.0, y), end: end(60.0, y) }) }
fn caret(y: f32) -> Option<Ends> { Some(Ends::Caret(end(10.0, y))) }

fn run(state: &mut State, event: Event) -> Vec<Effect> { step(state, event).into_iter().collect() }
fn frame(state: &mut State, ends: Option<Ends>, menu: bool) -> Vec<Effect> { run(state, Event::Frame { ends, menu }) }

#[test]
fn handles_follow_the_ends_and_are_told_once() {
    let mut s = State::default();
    assert_eq!(frame(&mut s, None, false), []);
    assert_eq!(frame(&mut s, range(100.0), false), [Effect::Handles(range(100.0))]);
    assert_eq!(frame(&mut s, range(100.0), false), []);
    assert_eq!(frame(&mut s, range(80.0), false), [Effect::Handles(range(80.0))]);
    assert_eq!(frame(&mut s, None, false), [Effect::Handles(None)]);
}

#[test]
fn the_toolbar_stays_while_its_selection_moves_and_shuts_when_it_collapses() {
    let mut s = State::default();
    assert_eq!(frame(&mut s, range(100.0), true), [Effect::Handles(range(100.0))]);
    // The keyboard came up under a long press in a field: the page moved, the selection didn't.
    assert_eq!(frame(&mut s, range(40.0), false), [Effect::Handles(range(40.0))]);
    // Typing replaced the selection.
    assert_eq!(frame(&mut s, caret(40.0), false), [Effect::Handles(caret(40.0)), Effect::Shut]);
    assert_eq!(frame(&mut s, None, false), [Effect::Handles(None)], "shut once");
}

#[test]
fn a_touch_on_the_view_shuts_the_toolbar() {
    let mut s = State::default();
    assert_eq!(run(&mut s, Event::Touch), []);
    frame(&mut s, range(100.0), true);
    assert_eq!(run(&mut s, Event::Touch), [Effect::Shut]);
    assert_eq!(run(&mut s, Event::Touch), []);
}

#[test]
fn a_held_handle_shuts_the_toolbar_and_its_release_asks_for_it_again() {
    let mut s = State::default();
    frame(&mut s, range(100.0), true);
    assert_eq!(run(&mut s, Event::Grab), [Effect::Shut]);
    assert_eq!(frame(&mut s, range(120.0), false), [Effect::Handles(range(120.0))]);
    assert_eq!(run(&mut s, Event::Release), [Effect::Ask]);
    assert_eq!(frame(&mut s, range(120.0), true), []);
    // A handle tapped with the toolbar away asks too.
    assert_eq!(run(&mut s, Event::Closed), []);
    assert_eq!(run(&mut s, Event::Grab), []);
    assert_eq!(run(&mut s, Event::Release), [Effect::Ask]);
}

#[test]
fn a_toolbar_that_closed_itself_is_not_shut_again() {
    let mut s = State::default();
    frame(&mut s, range(100.0), true);
    assert_eq!(run(&mut s, Event::Closed), []);
    assert_eq!(frame(&mut s, None, false), [Effect::Handles(None)]);
    assert_eq!(run(&mut s, Event::Touch), []);
}

#[test]
fn a_scroll_brings_the_toolbar_back_and_a_tap_does_not() {
    let mut s = State::default();
    frame(&mut s, range(100.0), true);
    assert_eq!(run(&mut s, Event::Touch), [Effect::Shut]);
    assert_eq!(frame(&mut s, range(60.0), false), [Effect::Handles(range(60.0))]);
    assert_eq!(run(&mut s, Event::Lift { panned: true }), [Effect::Ask]);
    frame(&mut s, range(60.0), true);
    assert_eq!(run(&mut s, Event::Touch), [Effect::Shut]);
    assert_eq!(run(&mut s, Event::Lift { panned: false }), []);
    // With no toolbar to bring back, a scroll asks for none.
    assert_eq!(run(&mut s, Event::Touch), []);
    assert_eq!(run(&mut s, Event::Lift { panned: true }), []);
}

#[test]
fn a_selection_a_tap_made_gets_its_toolbar() {
    // A second tap selects its word.
    let mut s = State::default();
    assert_eq!(run(&mut s, Event::Touch), []);
    assert_eq!(run(&mut s, Event::Lift { panned: false }), []);
    assert_eq!(frame(&mut s, range(100.0), false), [Effect::Handles(range(100.0)), Effect::Ask]);
    assert_eq!(frame(&mut s, range(100.0), true), []);
    // A tap with that selection showing asks for nothing: on a control that keeps it, or the
    // lift of the long press that made it.
    assert_eq!(run(&mut s, Event::Touch), [Effect::Shut]);
    assert_eq!(run(&mut s, Event::Lift { panned: false }), []);
    assert_eq!(frame(&mut s, range(100.0), false), []);
    let mut s = State::default();
    run(&mut s, Event::Touch);
    assert_eq!(frame(&mut s, range(100.0), true), [Effect::Handles(range(100.0))]);
    run(&mut s, Event::Lift { panned: false });
    assert_eq!(frame(&mut s, range(100.0), false), []);
    // A frame that isn't a tap's (a selection the app made) asks for none either.
    let mut s = State::default();
    assert_eq!(frame(&mut s, range(100.0), false), [Effect::Handles(range(100.0))]);
}
