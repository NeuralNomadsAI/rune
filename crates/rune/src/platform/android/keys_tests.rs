use super::keys::key;
use crate::Key;

const SHIFT: i32 = 0x1;
const CTRL: i32 = 0x1000;

#[test]
fn editing_and_navigation_keys_have_names() {
    for (code, want) in [(67, Key::Backspace), (112, Key::Delete), (66, Key::Enter), (160, Key::Enter), (61, Key::Tab), (111, Key::Escape),
        (21, Key::Left), (22, Key::Right), (19, Key::Up), (20, Key::Down), (122, Key::Home), (123, Key::End), (131, Key::Function(1)), (142, Key::Function(12))] {
        assert_eq!(key(code, 0, 0, 0).map(|k| k.key.clone()), Some(want), "{code}");
    }
}

#[test]
fn a_character_key_types_its_character_with_shift_applied() {
    // KEYCODE_A: 'a', and 'A' with Shift.
    assert_eq!(key(29, 'a' as u32, 'a' as u32, 0).map(|k| k.key.clone()), Some(Key::Text("a".into())));
    let shifted = key(29, 'a' as u32, 'A' as u32, SHIFT).unwrap();
    assert_eq!(shifted.key, Key::Text("A".into()));
    assert!(shifted.mods.shift);
}

#[test]
fn ctrl_makes_a_shortcut_of_the_unmodified_character() {
    let copy = key(31, 'c' as u32, 0, CTRL).unwrap();
    assert_eq!(copy.key, Key::Command('c'));
    assert!(copy.mods.ctrl && copy.mods.cmd);
}

#[test]
fn keys_without_a_name_or_character_are_left_to_the_system() {
    // Volume up, a bare Shift.
    assert_eq!(key(24, 0, 0, 0), None);
    assert_eq!(key(59, 0, 0, SHIFT), None);
}
