use super::throttle::{Effect, Event, SCROLL_INTERVAL, Throttle, step};

fn frame(state: &mut Throttle, now: f64, scrolling: bool) -> Vec<Effect> {
    step(state, Event::Frame { now, scrolling }).into_iter().collect()
}

#[test]
fn at_rest_every_frame_syncs() {
    let mut t = Throttle::default();
    for i in 0..5 {
        assert_eq!(frame(&mut t, f64::from(i) / 60.0, false), [Effect::Sync]);
        assert_eq!(t, Throttle::Open);
    }
}

#[test]
fn a_scroll_syncs_once_then_holds_until_the_interval() {
    let mut t = Throttle::default();
    assert_eq!(frame(&mut t, 1.0, true), [Effect::Sync, Effect::WakeAt(1.0 + SCROLL_INTERVAL)]);
    // 120 Hz frames inside the interval send nothing through JNI.
    let mut sent = 0;
    for i in 1..12 {
        sent += frame(&mut t, 1.0 + f64::from(i) / 120.0, true).len();
    }
    assert_eq!(sent, 0);
    // The first frame at or past the deadline syncs and holds again.
    let due = 1.0 + SCROLL_INTERVAL;
    assert_eq!(frame(&mut t, due, true), [Effect::Sync, Effect::WakeAt(due + SCROLL_INTERVAL)]);
}

#[test]
fn a_one_second_fling_syncs_about_ten_times_not_a_hundred_and_twenty() {
    let mut t = Throttle::default();
    let syncs = (0..120).filter(|i| frame(&mut t, f64::from(*i) / 120.0, true).contains(&Effect::Sync)).count();
    assert!((9..=11).contains(&syncs), "{syncs}");
}

#[test]
fn the_frame_a_scroll_rests_on_syncs_at_once() {
    let mut t = Throttle::default();
    frame(&mut t, 0.0, true);
    assert!(frame(&mut t, 0.01, true).is_empty());
    assert_eq!(frame(&mut t, 0.02, false), [Effect::Sync]);
    assert_eq!(t, Throttle::Open);
}

#[test]
fn the_wake_flushes_a_scroll_that_stopped_drawing() {
    // The last scroll frame was held; no frame follows until the wake it asked for.
    let mut t = Throttle::default();
    frame(&mut t, 0.0, true);
    assert!(frame(&mut t, 0.05, true).is_empty());
    assert_eq!(frame(&mut t, SCROLL_INTERVAL, false), [Effect::Sync]);
}
