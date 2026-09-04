//! Key specifications, as a config file writes them: `"ctrl-r"`, `"alt-left"`,
//! `"ctrl-alt-f5"`, `"x"`, ...
//!
//! Modifiers first, in any order, then exactly one key.

use reedline::{KeyCode, KeyModifiers};

/// Parse `"ctrl-alt-x"` into the modifiers and key it names.
pub fn parse(spec: &str) -> Result<(KeyModifiers, KeyCode), String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty key".to_string());
    }

    // "-" and "ctrl--" bind the dash itself.
    let mut parts: Vec<&str> = spec.split('-').collect();
    let mut key = parts.pop().unwrap_or_default();
    if key.is_empty() {
        key = "-";
        if parts.last() == Some(&"") {
            parts.pop();
        }
    }

    let mut modifiers = KeyModifiers::NONE;
    for part in parts {
        modifiers |= match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => KeyModifiers::CONTROL,
            "alt" | "meta" => KeyModifiers::ALT,
            "shift" => KeyModifiers::SHIFT,
            "super" | "cmd" => KeyModifiers::SUPER,
            other => return Err(format!("{other:?} is not a modifier (in {spec:?})")),
        };
    }

    let mut code = keycode(key, spec)?;
    // shift-x is reported as "X"
    if let KeyCode::Char(c) = code
        && c.is_ascii_uppercase()
    {
        if modifiers == KeyModifiers::NONE {
            modifiers = KeyModifiers::SHIFT;
        }
        code = KeyCode::Char(c.to_ascii_lowercase());
    }

    Ok((modifiers, code))
}

fn keycode(name: &str, spec: &str) -> Result<KeyCode, String> {
    let lowered = name.to_ascii_lowercase();
    let name = lowered.as_str();
    // Function keys first, since "f1" would otherwise be two characters.
    if let Some(number) = name.strip_prefix('f')
        && let Ok(number) = number.parse::<u8>()
        && (1..=20).contains(&number)
    {
        return Ok(KeyCode::F(number));
    }

    Ok(match name {
        "enter" | "return" | "cr" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "backspace" | "bs" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "insert" | "ins" => KeyCode::Insert,
        "esc" | "escape" => KeyCode::Esc,
        "space" => KeyCode::Char(' '),
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        _ => {
            let mut chars = spec_key(spec).chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return Err(format!("{name:?} is not a key (in {spec:?})")),
            }
        }
    })
}

/// The key part of `spec`, as written.
fn spec_key(spec: &str) -> &str {
    match spec.rsplit_once('-') {
        Some((_, "")) => "-",
        Some((_, key)) => key,
        None => spec,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_character_has_no_modifiers() {
        assert_eq!(
            parse("x").unwrap(),
            (KeyModifiers::NONE, KeyCode::Char('x'))
        );
    }

    #[test]
    fn modifiers_combine_in_any_order() {
        let expected = (
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            KeyCode::Char('x'),
        );
        assert_eq!(parse("ctrl-alt-x").unwrap(), expected);
        assert_eq!(parse("alt-ctrl-x").unwrap(), expected);
        assert_eq!(parse("control-meta-x").unwrap(), expected);
    }

    #[test]
    fn named_keys_are_recognised() {
        for (spec, expected) in [
            ("tab", KeyCode::Tab),
            ("backtab", KeyCode::BackTab),
            ("enter", KeyCode::Enter),
            ("esc", KeyCode::Esc),
            ("backspace", KeyCode::Backspace),
            ("left", KeyCode::Left),
            ("pageup", KeyCode::PageUp),
            ("space", KeyCode::Char(' ')),
            ("f5", KeyCode::F(5)),
            ("f12", KeyCode::F(12)),
        ] {
            assert_eq!(parse(spec).unwrap().1, expected, "{spec}");
        }
    }

    #[test]
    fn case_does_not_matter() {
        assert_eq!(parse("CTRL-R").unwrap(), parse("ctrl-r").unwrap());
        assert_eq!(parse("Shift-Tab").unwrap(), parse("shift-tab").unwrap());
    }

    #[test]
    fn the_separator_can_also_be_the_key() {
        assert_eq!(
            parse("-").unwrap(),
            (KeyModifiers::NONE, KeyCode::Char('-'))
        );
        assert_eq!(
            parse("ctrl--").unwrap(),
            (KeyModifiers::CONTROL, KeyCode::Char('-'))
        );
    }

    #[test]
    fn an_unknown_modifier_or_key_says_which_word_was_wrong() {
        let err = parse("hyper-x").unwrap_err();
        assert!(err.contains("hyper"), "{err}");
        let err = parse("ctrl-nope").unwrap_err();
        assert!(err.contains("nope"), "{err}");
        assert!(parse("").is_err());
    }

    #[test]
    fn a_function_key_out_of_range_is_not_silently_a_character() {
        assert!(parse("f99").is_err());
        assert_eq!(parse("f").unwrap().1, KeyCode::Char('f'));
    }

    #[test]
    fn an_uppercase_letter_is_the_shifted_lowercase_one() {
        // The terminal reports Shift+X as `X` with SHIFT.
        let shifted = (KeyModifiers::SHIFT, KeyCode::Char('x'));
        assert_eq!(parse("X").unwrap(), shifted);
        assert_eq!(parse("shift-x").unwrap(), shifted);
        assert_eq!(parse("Ctrl-X").unwrap(), parse("ctrl-x").unwrap());
    }
}
