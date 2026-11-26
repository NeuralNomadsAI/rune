//! Rune's haptics (`Tx::haptic`) for `RuneView` to play with `View.performHapticFeedback`,
//! which needs no `VIBRATE` permission and follows the system's touch feedback setting.

use crate::runtime::RuntimeCore;
use crate::{Haptic, HapticOutcome, ImpactStrength};

/// The code `RuneView.haptic` turns into the `HapticFeedbackConstants` value the device's API
/// level has for it.
pub(crate) fn code(haptic: Haptic) -> i32 {
    match haptic {
        Haptic::Selection => 0,
        Haptic::Impact(ImpactStrength::Light) => 1,
        Haptic::Impact(ImpactStrength::Medium) => 2,
        Haptic::Impact(ImpactStrength::Heavy) => 3,
        Haptic::Impact(ImpactStrength::Soft) => 4,
        Haptic::Impact(ImpactStrength::Rigid) => 5,
        Haptic::Notify(HapticOutcome::Success) => 6,
        Haptic::Notify(HapticOutcome::Warning) => 7,
        Haptic::Notify(HapticOutcome::Error) => 8,
    }
}

/// The haptics asked for since the last call, oldest first, as codes.
pub(crate) fn take(core: &mut RuntimeCore) -> Vec<i32> {
    core.host.haptics.take().into_iter().map(code).collect()
}