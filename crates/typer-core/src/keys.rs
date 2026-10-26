//! Human-readable labels for evdev key codes, for the key overlay.

/// Held modifiers, as bit flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const CTRL: u8 = 1;
    pub const ALT: u8 = 2;
    pub const SHIFT: u8 = 4;
    pub const SUPER: u8 = 8;

    /// The modifier bit for a key code, if it is a modifier.
    pub fn bit(code: u16) -> Option<u8> {
        Some(match code {
            29 | 97 => Self::CTRL,    // KEY_LEFTCTRL, KEY_RIGHTCTRL
            56 | 100 => Self::ALT,    // KEY_LEFTALT, KEY_RIGHTALT
            42 | 54 => Self::SHIFT,   // KEY_LEFTSHIFT, KEY_RIGHTSHIFT
            125 | 126 => Self::SUPER, // KEY_LEFTMETA, KEY_RIGHTMETA
            _ => return None,
        })
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn names(self) -> impl Iterator<Item = &'static str> {
        [
            (Self::SUPER, "Super"),
            (Self::CTRL, "Ctrl"),
            (Self::ALT, "Alt"),
            (Self::SHIFT, "Shift"),
        ]
        .into_iter()
        .filter(move |(bit, _)| self.0 & bit != 0)
        .map(|(_, name)| name)
    }
}

/// Label for a modifier pressed and released on its own.
pub fn modifier_label(bit: u8) -> &'static str {
    Modifiers(bit).names().next().unwrap_or("")
}

/// Label for a non-modifier key, or `None` for keys not worth showing.
pub fn label(code: u16) -> Option<&'static str> {
    const LETTERS: [(u16, &str); 26] = [
        (30, "A"),
        (48, "B"),
        (46, "C"),
        (32, "D"),
        (18, "E"),
        (33, "F"),
        (34, "G"),
        (35, "H"),
        (23, "I"),
        (36, "J"),
        (37, "K"),
        (38, "L"),
        (50, "M"),
        (49, "N"),
        (24, "O"),
        (25, "P"),
        (16, "Q"),
        (19, "R"),
        (31, "S"),
        (20, "T"),
        (22, "U"),
        (47, "V"),
        (17, "W"),
        (45, "X"),
        (21, "Y"),
        (44, "Z"),
    ];
    if let Some((_, l)) = LETTERS.iter().find(|(c, _)| *c == code) {
        return Some(l);
    }
    Some(match code {
        2..=10 => ["1", "2", "3", "4", "5", "6", "7", "8", "9"][code as usize - 2],
        11 => "0",
        1 => "Esc",
        12 => "-",
        13 => "=",
        14 => "⌫",
        15 => "⇥",
        26 => "[",
        27 => "]",
        28 | 96 => "⏎",
        39 => ";",
        40 => "'",
        41 => "`",
        43 => "\\",
        51 => ",",
        52 => ".",
        53 => "/",
        57 => "␣",
        58 => "Caps",
        59..=68 => {
            ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10"][code as usize - 59]
        }
        87 => "F11",
        88 => "F12",
        86 => "\\",
        89 => "/", // KEY_RO (ABNT2 /?)
        99 => "PrtSc",
        102 => "Home",
        103 => "↑",
        104 => "PgUp",
        105 => "←",
        106 => "→",
        107 => "End",
        108 => "↓",
        109 => "PgDn",
        110 => "Ins",
        111 => "Del",
        _ => return None,
    })
}

/// "Ctrl + Shift + T". Shift alone with a printable key shows just the key, so
/// ordinary typing doesn't read "Shift + A".
pub fn combo(mods: Modifiers, key: &str) -> String {
    let printable = key.chars().count() == 1;
    let mods = if mods.0 == Modifiers::SHIFT && printable {
        Modifiers(0)
    } else {
        mods
    };
    let mut out = String::new();
    for name in mods.names() {
        out.push_str(name);
        out.push_str(" + ");
    }
    out.push_str(key);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(label(30), Some("A"));
        assert_eq!(label(11), Some("0"));
        assert_eq!(label(28), Some("⏎"));
        assert_eq!(label(88), Some("F12"));
        assert_eq!(label(29), None); // modifiers have no plain label
    }

    #[test]
    fn combos() {
        let ctrl_shift = Modifiers(Modifiers::CTRL | Modifiers::SHIFT);
        assert_eq!(combo(ctrl_shift, "T"), "Ctrl + Shift + T");
        assert_eq!(combo(Modifiers(Modifiers::SHIFT), "A"), "A");
        assert_eq!(combo(Modifiers(Modifiers::SHIFT), "⇥"), "⇥");
        assert_eq!(combo(Modifiers(Modifiers::SHIFT), "F5"), "Shift + F5");
        assert_eq!(combo(Modifiers(Modifiers::SUPER), "⏎"), "Super + ⏎");
        assert_eq!(combo(Modifiers::default(), "A"), "A");
    }

    #[test]
    fn modifier_bits() {
        assert_eq!(Modifiers::bit(97), Some(Modifiers::CTRL));
        assert_eq!(Modifiers::bit(30), None);
        assert_eq!(modifier_label(Modifiers::SUPER), "Super");
    }
}
