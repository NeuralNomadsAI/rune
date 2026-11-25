use super::touch::{ACTION_CANCEL, ACTION_DOWN, ACTION_MOVE, ACTION_POINTER_DOWN, ACTION_POINTER_UP, ACTION_UP, phases};
use crate::TouchPhase;

fn all(action: i32, index: usize, count: usize) -> Vec<(usize, TouchPhase)> {
    phases(action, index, count).collect()
}

#[test]
fn one_finger_down_move_up() {
    assert_eq!(all(ACTION_DOWN, 0, 1), [(0, TouchPhase::Started)]);
    assert_eq!(all(ACTION_MOVE, 0, 1), [(0, TouchPhase::Moved)]);
    assert_eq!(all(ACTION_UP, 0, 1), [(0, TouchPhase::Ended)]);
}

#[test]
fn a_second_finger_starts_and_lifts_alone_while_moves_carry_both() {
    assert_eq!(all(ACTION_POINTER_DOWN, 1, 2), [(1, TouchPhase::Started)]);
    assert_eq!(all(ACTION_MOVE, 0, 2), [(0, TouchPhase::Moved), (1, TouchPhase::Moved)]);
    assert_eq!(all(ACTION_POINTER_UP, 0, 2), [(0, TouchPhase::Ended)]);
}

#[test]
fn a_cancel_cancels_every_finger() {
    // The system took the gesture (a back swipe, a notification shade): nothing is clicked.
    assert_eq!(all(ACTION_CANCEL, 0, 2), [(0, TouchPhase::Cancelled), (1, TouchPhase::Cancelled)]);
}

#[test]
fn other_actions_and_a_bad_index_report_nothing() {
    // ACTION_OUTSIDE, hover and scroll actions aren't touches.
    for action in [4, 7, 8, 9, 10] {
        assert!(all(action, 0, 1).is_empty(), "{action}");
    }
    assert!(all(ACTION_POINTER_DOWN, 3, 2).is_empty());
}
