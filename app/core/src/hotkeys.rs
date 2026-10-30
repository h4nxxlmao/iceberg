use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: HotkeyKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyKey {
    Char(char),
    Named(NamedKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NamedKey {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Space,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Comma,
    Period,
    Slash,
    Semicolon,
    Quote,
    Minus,
    Equals,
    Function(u8),
}

impl HotkeyKey {
    pub fn virtual_key(self) -> u32 {
        match self {
            Self::Char(value) => {
                let upper = value.to_ascii_uppercase();
                if upper.is_ascii_alphabetic() || upper.is_ascii_digit() {
                    upper as u32
                } else {
                    0
                }
            }
            Self::Named(key) => match key {
                NamedKey::ArrowLeft => 0x25,
                NamedKey::ArrowUp => 0x26,
                NamedKey::ArrowRight => 0x27,
                NamedKey::ArrowDown => 0x28,
                NamedKey::Space => 0x20,
                NamedKey::Enter => 0x0D,
                NamedKey::Tab => 0x09,
                NamedKey::Escape => 0x1B,
                NamedKey::Backspace => 0x08,
                NamedKey::Delete => 0x2E,
                NamedKey::Home => 0x24,
                NamedKey::End => 0x23,
                NamedKey::PageUp => 0x21,
                NamedKey::PageDown => 0x22,
                NamedKey::Insert => 0x2D,
                NamedKey::Comma => 0xBC,
                NamedKey::Period => 0xBE,
                NamedKey::Slash => 0xBF,
                NamedKey::Semicolon => 0xBA,
                NamedKey::Quote => 0xDE,
                NamedKey::Minus => 0xBD,
                NamedKey::Equals => 0xBB,
                NamedKey::Function(number) => 0x70 + (number.saturating_sub(1) as u32),
            },
        }
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.win {
            parts.push("Win".to_string());
        }
        parts.push(match self.key {
            HotkeyKey::Char(value) => value.to_ascii_uppercase().to_string(),
            HotkeyKey::Named(key) => match key {
                NamedKey::ArrowLeft => "Left".to_string(),
                NamedKey::ArrowRight => "Right".to_string(),
                NamedKey::ArrowUp => "Up".to_string(),
                NamedKey::ArrowDown => "Down".to_string(),
                NamedKey::Space => "Space".to_string(),
                NamedKey::Enter => "Enter".to_string(),
                NamedKey::Tab => "Tab".to_string(),
                NamedKey::Escape => "Esc".to_string(),
                NamedKey::Backspace => "Backspace".to_string(),
                NamedKey::Delete => "Delete".to_string(),
                NamedKey::Home => "Home".to_string(),
                NamedKey::End => "End".to_string(),
                NamedKey::PageUp => "PageUp".to_string(),
                NamedKey::PageDown => "PageDown".to_string(),
                NamedKey::Insert => "Insert".to_string(),
                NamedKey::Comma => ",".to_string(),
                NamedKey::Period => ".".to_string(),
                NamedKey::Slash => "/".to_string(),
                NamedKey::Semicolon => ";".to_string(),
                NamedKey::Quote => "'".to_string(),
                NamedKey::Minus => "-".to_string(),
                NamedKey::Equals => "=".to_string(),
                NamedKey::Function(number) => format!("F{number}"),
            },
        });
        formatter.write_str(&parts.join("+"))
    }
}

pub fn parse(value: &str) -> Result<Hotkey, String> {
    let tokens: Vec<&str> = value
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return Err("Hotkey is empty".to_string());
    }
    let mut hotkey = Hotkey {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
        key: HotkeyKey::Char('I'),
    };
    let (key_token, modifiers) = tokens.split_last().ok_or("Hotkey is empty")?;
    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => hotkey.ctrl = true,
            "alt" => hotkey.alt = true,
            "shift" => hotkey.shift = true,
            "win" | "meta" | "super" => hotkey.win = true,
            other => return Err(format!("Unknown modifier '{other}'")),
        }
    }
    if !hotkey.ctrl && !hotkey.alt && !hotkey.shift && !hotkey.win {
        return Err("Use at least one modifier".to_string());
    }
    hotkey.key = parse_key(key_token)?;
    Ok(hotkey)
}

fn parse_key(value: &str) -> Result<HotkeyKey, String> {
    let token = value.trim();
    if token.len() == 1 {
        let character = token.chars().next().unwrap_or(' ');
        if character.is_ascii_alphanumeric() {
            return Ok(HotkeyKey::Char(character));
        }
    }
    if let Some(letter) = token.strip_prefix("Key") {
        if letter.len() == 1 {
            let character = letter.chars().next().unwrap_or(' ');
            if character.is_ascii_alphabetic() {
                return Ok(HotkeyKey::Char(character));
            }
        }
    }
    if let Some(digit) = token.strip_prefix("Digit") {
        if digit.len() == 1 && digit.chars().all(|character| character.is_ascii_digit()) {
            return Ok(HotkeyKey::Char(digit.chars().next().unwrap_or('0')));
        }
    }
    let named = match token.to_ascii_lowercase().as_str() {
        "left" | "arrowleft" => Some(NamedKey::ArrowLeft),
        "right" | "arrowright" => Some(NamedKey::ArrowRight),
        "up" | "arrowup" => Some(NamedKey::ArrowUp),
        "down" | "arrowdown" => Some(NamedKey::ArrowDown),
        "space" => Some(NamedKey::Space),
        "enter" | "return" => Some(NamedKey::Enter),
        "tab" => Some(NamedKey::Tab),
        "escape" | "esc" => Some(NamedKey::Escape),
        "backspace" => Some(NamedKey::Backspace),
        "delete" | "del" => Some(NamedKey::Delete),
        "home" => Some(NamedKey::Home),
        "end" => Some(NamedKey::End),
        "pageup" => Some(NamedKey::PageUp),
        "pagedown" => Some(NamedKey::PageDown),
        "insert" => Some(NamedKey::Insert),
        "comma" | "," => Some(NamedKey::Comma),
        "period" | "." => Some(NamedKey::Period),
        "slash" | "/" => Some(NamedKey::Slash),
        "semicolon" | ";" => Some(NamedKey::Semicolon),
        "quote" | "'" => Some(NamedKey::Quote),
        "minus" | "-" => Some(NamedKey::Minus),
        "equals" | "=" => Some(NamedKey::Equals),
        _ => None,
    };
    if let Some(named) = named {
        return Ok(HotkeyKey::Named(named));
    }
    if let Some(number) = token.strip_prefix('F').or_else(|| token.strip_prefix('f')) {
        if let Ok(number) = number.parse::<u8>() {
            if (1..=24).contains(&number) {
                return Ok(HotkeyKey::Named(NamedKey::Function(number)));
            }
        }
    }
    Err(format!("Unsupported key '{token}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_style_hotkeys() {
        let hotkey = parse("Ctrl+Alt+KeyI").expect("hotkey");
        assert!(hotkey.ctrl && hotkey.alt);
        assert_eq!(hotkey.key, HotkeyKey::Char('I'));
        assert_eq!(hotkey.to_string(), "Ctrl+Alt+I");
        assert_eq!(hotkey.key.virtual_key(), 'I' as u32);
    }

    #[test]
    fn parses_arrows_and_symbols() {
        assert_eq!(
            parse("Ctrl+Alt+ArrowRight").unwrap().key,
            HotkeyKey::Named(NamedKey::ArrowRight)
        );
        assert_eq!(
            parse("Ctrl+Alt+,").unwrap().key,
            HotkeyKey::Named(NamedKey::Comma)
        );
        assert_eq!(
            parse("Ctrl+Shift+F12").unwrap().key,
            HotkeyKey::Named(NamedKey::Function(12))
        );
    }

    #[test]
    fn rejects_modifierless_or_unknown_keys() {
        assert!(parse("I").is_err());
        assert!(parse("Ctrl+Alt+Nope").is_err());
    }
}
