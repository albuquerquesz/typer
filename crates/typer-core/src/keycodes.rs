//! Mechvibes packs key their sounds by libuiohook (iohook) keycodes. For the base
//! keyboard block those are PC scancode set 1 values, which match Linux evdev
//! `KEY_*` codes one-to-one. Extended (E0-prefixed) keys are encoded as `0x0Exx`
//! or `0xE0xx` and need translating.

/// Translate a Mechvibes/iohook keycode into a Linux evdev key code.
pub fn mechvibes_to_evdev(code: u32) -> Option<u16> {
    match code {
        1..=88 => Some(code as u16),
        _ if matches!(code & 0xFF00, 0x0E00 | 0xE000) => extended(code & 0xFF),
        _ => None,
    }
}

fn extended(scan: u32) -> Option<u16> {
    Some(match scan {
        0x1C => 96,  // KEY_KPENTER
        0x1D => 97,  // KEY_RIGHTCTRL
        0x35 => 98,  // KEY_KPSLASH
        0x37 => 99,  // KEY_SYSRQ (Print Screen)
        0x38 => 100, // KEY_RIGHTALT
        0x47 => 102, // KEY_HOME
        0x48 => 103, // KEY_UP
        0x49 => 104, // KEY_PAGEUP
        0x4B => 105, // KEY_LEFT
        0x4D => 106, // KEY_RIGHT
        0x4F => 107, // KEY_END
        0x50 => 108, // KEY_DOWN
        0x51 => 109, // KEY_PAGEDOWN
        0x52 => 110, // KEY_INSERT
        0x53 => 111, // KEY_DELETE
        0x5B => 125, // KEY_LEFTMETA
        0x5C => 126, // KEY_RIGHTMETA
        0x5D => 127, // KEY_COMPOSE (menu)
        _ => return None,
    })
}

pub const KEY_BACKSPACE: u16 = 14;
pub const KEY_TAB: u16 = 15;
pub const KEY_ENTER: u16 = 28;
pub const KEY_A: u16 = 30;
pub const KEY_SPACE: u16 = 57;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_block_is_identity() {
        assert_eq!(mechvibes_to_evdev(1), Some(1)); // Esc
        assert_eq!(mechvibes_to_evdev(14), Some(KEY_BACKSPACE));
        assert_eq!(mechvibes_to_evdev(28), Some(KEY_ENTER));
        assert_eq!(mechvibes_to_evdev(57), Some(KEY_SPACE));
        assert_eq!(mechvibes_to_evdev(88), Some(88)); // F12
    }

    #[test]
    fn extended_keys_both_encodings() {
        assert_eq!(mechvibes_to_evdev(57416), Some(103)); // 0xE048 Up
        assert_eq!(mechvibes_to_evdev(3656), Some(103)); // 0x0E48 Up
        assert_eq!(mechvibes_to_evdev(3667), Some(111)); // 0x0E53 Delete
        assert_eq!(mechvibes_to_evdev(3612), Some(96)); // 0x0E1C KP Enter
    }

    #[test]
    fn unknown_codes() {
        assert_eq!(mechvibes_to_evdev(0), None);
        assert_eq!(mechvibes_to_evdev(0x0E99), None);
        assert_eq!(mechvibes_to_evdev(500), None);
    }
}
