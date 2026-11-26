use super::haptics::code;
use crate::{Env, Haptic, HapticOutcome, ImpactStrength, Runtime, el};

const ALL: [Haptic; 9] = [
    Haptic::Selection,
    Haptic::Impact(ImpactStrength::Light),
    Haptic::Impact(ImpactStrength::Medium),
    Haptic::Impact(ImpactStrength::Heavy),
    Haptic::Impact(ImpactStrength::Soft),
    Haptic::Impact(ImpactStrength::Rigid),
    Haptic::Notify(HapticOutcome::Success),
    Haptic::Notify(HapticOutcome::Warning),
    Haptic::Notify(HapticOutcome::Error),
];

#[test]
fn each_haptic_has_the_code_the_view_plays() {
    assert_eq!(ALL.map(code), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn what_a_handler_asks_for_reaches_the_view_once() {
    let mut rt = Runtime::new((), |_, _| el().size(10.0, 10.0), (10.0, 10.0), Env::default());
    rt.frame(0.0);
    rt.update(|_, tx| {
        tx.haptic(Haptic::Selection);
        tx.haptic(Haptic::Notify(HapticOutcome::Success));
    });
    assert_eq!(super::haptics::take(rt.core_mut()), [0, 6]);
    assert!(super::haptics::take(rt.core_mut()).is_empty());
}