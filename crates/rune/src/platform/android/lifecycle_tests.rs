use super::lifecycle::{Effect, Event, Insets, KEYBOARD_GAP, State, Visibility, safe_area, step, visible_band};

fn run(state: &mut State, events: &[Event]) -> Vec<Effect> {
    events.iter().flat_map(|&e| step(state, e)).collect()
}

const PORTRAIT: (u32, u32) = (1080, 2400);
const LANDSCAPE: (u32, u32) = (2400, 1080);

#[test]
fn first_window_attaches_then_draws() {
    let mut s = State::default();
    assert_eq!(run(&mut s, &[Event::Resumed, Event::WindowCreated { px: PORTRAIT }]), [Effect::Attach { px: PORTRAIT }, Effect::Draw]);
    assert_eq!(run(&mut s, &[Event::Frame]), [Effect::Draw]);
}

#[test]
fn no_frame_without_a_window() {
    let mut s = State::default();
    assert!(run(&mut s, &[Event::Frame, Event::WindowChanged { px: PORTRAIT }, Event::WindowDestroyed]).is_empty());
}

#[test]
fn twenty_trips_to_the_background_detach_and_reattach_each_time() {
    let mut s = State::default();
    run(&mut s, &[Event::Resumed, Event::WindowCreated { px: PORTRAIT }]);
    for _ in 0..20 {
        // Home: onPause, then surfaceDestroyed. Back: onResume, then surfaceCreated.
        assert_eq!(run(&mut s, &[Event::Paused, Event::WindowDestroyed, Event::Frame]), [Effect::Detach]);
        assert_eq!(s, State::NoWindow { visibility: Visibility::Paused });
        assert_eq!(run(&mut s, &[Event::Resumed, Event::WindowCreated { px: PORTRAIT }]), [Effect::Attach { px: PORTRAIT }, Effect::Draw]);
    }
}

#[test]
fn paused_with_its_window_kept_stops_frames_until_resumed() {
    // Split screen or a dialog over the activity: paused, the window stays.
    let mut s = State::default();
    run(&mut s, &[Event::Resumed, Event::WindowCreated { px: PORTRAIT }]);
    assert!(run(&mut s, &[Event::Paused, Event::Frame, Event::Frame]).is_empty());
    assert_eq!(run(&mut s, &[Event::WindowChanged { px: LANDSCAPE }]), [Effect::Resize { px: LANDSCAPE }]);
    assert_eq!(run(&mut s, &[Event::Resumed]), [Effect::Draw]);
}

#[test]
fn a_window_created_while_paused_draws_on_resume() {
    let mut s = State::default();
    assert_eq!(run(&mut s, &[Event::Paused, Event::WindowCreated { px: PORTRAIT }]), [Effect::Attach { px: PORTRAIT }]);
    assert_eq!(run(&mut s, &[Event::Resumed]), [Effect::Draw]);
}

#[test]
fn rotation_resizes_once_and_a_repeated_size_does_nothing() {
    let mut s = State::default();
    run(&mut s, &[Event::Resumed, Event::WindowCreated { px: PORTRAIT }]);
    assert_eq!(run(&mut s, &[Event::WindowChanged { px: LANDSCAPE }]), [Effect::Resize { px: LANDSCAPE }, Effect::Draw]);
    assert!(run(&mut s, &[Event::WindowChanged { px: LANDSCAPE }]).is_empty());
}

#[test]
fn a_replacement_window_without_destroy_reattaches() {
    let mut s = State::default();
    run(&mut s, &[Event::Resumed, Event::WindowCreated { px: PORTRAIT }]);
    assert_eq!(run(&mut s, &[Event::WindowCreated { px: LANDSCAPE }]), [Effect::Attach { px: LANDSCAPE }, Effect::Draw]);
}

#[test]
fn the_keyboard_raises_the_bottom_inset_without_resizing() {
    let bars = [96.0, 0.0, 48.0, 0.0];
    let closed = safe_area(Insets { bars, ime: [0.0; 4] }, 2.0);
    let open = safe_area(Insets { bars, ime: [0.0, 0.0, 900.0, 0.0] }, 2.0);
    assert_eq!(closed, [48.0, 0.0, 24.0, 0.0]);
    assert_eq!(open, [48.0, 0.0, 450.0, 0.0]);
    // The window keeps its portrait size while the keyboard shows: nothing to resize.
    let mut s = State::default();
    run(&mut s, &[Event::Resumed, Event::WindowCreated { px: PORTRAIT }]);
    assert!(run(&mut s, &[Event::WindowChanged { px: PORTRAIT }]).is_empty());
}

#[test]
fn safe_area_ignores_a_bad_density_and_negative_insets() {
    assert_eq!(safe_area(Insets { bars: [10.0, -4.0, 0.0, 0.0], ime: [0.0; 4] }, 0.0), [10.0, 0.0, 0.0, 0.0]);
    assert_eq!(safe_area(Insets { bars: [10.0, 0.0, 0.0, 0.0], ime: [0.0; 4] }, f32::NAN), [10.0, 0.0, 0.0, 0.0]);
}

#[test]
fn the_keyboard_leaves_a_band_of_the_window_in_sight() {
    let bars = [96.0, 0.0, 48.0, 0.0];
    let open = Insets { bars, ime: [0.0, 0.0, 900.0, 0.0] };
    // It ends a little above the keyboard, for the field's own padding and border to show.
    assert_eq!(visible_band(open, PORTRAIT, 3.0), Some((0.0, 500.0 - KEYBOARD_GAP)));
    // Closed, or no taller than the navigation bar it replaces: the whole window shows.
    assert_eq!(visible_band(Insets { bars, ime: [0.0; 4] }, PORTRAIT, 3.0), None);
    assert_eq!(visible_band(Insets { bars, ime: [0.0, 0.0, 48.0, 0.0] }, PORTRAIT, 3.0), None);
    // A keyboard taller than the window, or a density the system never reports.
    assert_eq!(visible_band(Insets { bars, ime: [0.0, 0.0, 5000.0, 0.0] }, PORTRAIT, 3.0), None);
    assert_eq!(visible_band(open, PORTRAIT, 0.0), Some((0.0, 1500.0 - KEYBOARD_GAP)));
}

#[test]
fn the_window_size_is_known_while_there_is_a_window() {
    let mut s = State::default();
    assert_eq!(s.px(), None);
    run(&mut s, &[Event::WindowCreated { px: PORTRAIT }, Event::WindowChanged { px: LANDSCAPE }]);
    assert_eq!(s.px(), Some(LANDSCAPE));
    run(&mut s, &[Event::WindowDestroyed]);
    assert_eq!(s.px(), None);
}
