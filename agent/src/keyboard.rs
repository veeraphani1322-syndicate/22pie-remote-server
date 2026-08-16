use serde::Deserialize;

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum KeyboardMessage {
    KeyDown {
        session_id: String,
        code: String,
        key: String,
        repeat: bool,
        modifiers: Modifiers,
        sequence: u64,
        timestamp: f64,
    },
    KeyUp {
        session_id: String,
        code: String,
        key: String,
        modifiers: Modifiers,
        sequence: u64,
        timestamp: f64,
    },
    TextInput {
        session_id: String,
        text: String,
        sequence: u64,
        timestamp: f64,
    },
}

impl KeyboardMessage {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let message: Self = serde_json::from_slice(data).ok()?;
        let valid = match &message {
            Self::KeyDown {
                code,
                key,
                timestamp,
                ..
            }
            | Self::KeyUp {
                code,
                key,
                timestamp,
                ..
            } => timestamp.is_finite() && key.chars().count() <= 32 && virtual_key(code).is_some(),
            Self::TextInput {
                text, timestamp, ..
            } => {
                timestamp.is_finite()
                    && !text.is_empty()
                    && text.chars().count() <= 256
                    && !text.contains('\0')
            }
        };
        valid.then_some(message)
    }
    pub fn session_id(&self) -> &str {
        match self {
            Self::KeyDown { session_id, .. }
            | Self::KeyUp { session_id, .. }
            | Self::TextInput { session_id, .. } => session_id,
        }
    }
}

fn virtual_key(code: &str) -> Option<u16> {
    let value = match code {
        "Backspace" => 0x08,
        "Tab" => 0x09,
        "Enter" => 0x0D,
        "ShiftLeft" => 0xA0,
        "ShiftRight" => 0xA1,
        "ControlLeft" => 0xA2,
        "ControlRight" => 0xA3,
        "AltLeft" => 0xA4,
        "AltRight" => 0xA5,
        "Pause" => 0x13,
        "CapsLock" => 0x14,
        "Escape" => 0x1B,
        "Space" => 0x20,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        "End" => 0x23,
        "Home" => 0x24,
        "ArrowLeft" => 0x25,
        "ArrowUp" => 0x26,
        "ArrowRight" => 0x27,
        "ArrowDown" => 0x28,
        "Insert" => 0x2D,
        "Delete" => 0x2E,
        "MetaLeft" => 0x5B,
        "MetaRight" => 0x5C,
        "ContextMenu" => 0x5D,
        "NumLock" => 0x90,
        "ScrollLock" => 0x91,
        "Semicolon" => 0xBA,
        "Equal" => 0xBB,
        "Comma" => 0xBC,
        "Minus" => 0xBD,
        "Period" => 0xBE,
        "Slash" => 0xBF,
        "Backquote" => 0xC0,
        "BracketLeft" => 0xDB,
        "Backslash" => 0xDC,
        "BracketRight" => 0xDD,
        "Quote" => 0xDE,
        _ if code.len() == 4 && code.starts_with("Key") => {
            code.as_bytes()
                .get(3)
                .copied()
                .filter(u8::is_ascii_uppercase)? as u16
        }
        _ if code.len() == 6 && code.starts_with("Digit") => {
            code.as_bytes().get(5).copied().filter(u8::is_ascii_digit)? as u16
        }
        _ if code.starts_with('F') => {
            let n = code[1..].parse::<u16>().ok()?;
            if !(1..=24).contains(&n) {
                return None;
            }
            0x6F + n
        }
        _ => return None,
    };
    Some(value)
}

#[cfg(windows)]
pub struct KeyboardController {
    held: std::collections::HashSet<u16>,
}

#[cfg(windows)]
impl KeyboardController {
    pub fn new() -> Self {
        Self {
            held: Default::default(),
        }
    }
    pub fn execute(&mut self, message: KeyboardMessage) -> bool {
        match message {
            KeyboardMessage::KeyDown { code, repeat, .. } => {
                let vk = virtual_key(&code).unwrap();
                if vk == 0x2E
                    && self.held.iter().any(|v| matches!(v, 0xA2 | 0xA3))
                    && self.held.iter().any(|v| matches!(v, 0xA4 | 0xA5))
                {
                    return false;
                }
                if !repeat {
                    self.held.insert(vk);
                }
                send_key(vk, false, false)
            }
            KeyboardMessage::KeyUp { code, .. } => {
                let vk = virtual_key(&code).unwrap();
                self.held.remove(&vk);
                send_key(vk, true, false)
            }
            KeyboardMessage::TextInput { text, .. } => text
                .encode_utf16()
                .all(|unit| send_key(unit, false, true) && send_key(unit, true, true)),
        }
    }
    pub fn release_all(&mut self) -> usize {
        let keys = self.held.drain().collect::<Vec<_>>();
        for key in &keys {
            let _ = send_key(*key, true, false);
        }
        keys.len()
    }
}

#[cfg(windows)]
fn send_key(value: u16, up: bool, unicode: bool) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let flags =
        (if up { KEYEVENTF_KEYUP } else { 0 }) | (if unicode { KEYEVENTF_UNICODE } else { 0 });
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: if unicode { 0 } else { value },
                wScan: if unicode { value } else { 0 },
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) == 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_protocol_and_limits() {
        assert!(KeyboardMessage::parse(br#"{"type":"key_down","session_id":"s","code":"KeyA","key":"a","repeat":false,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false},"sequence":1,"timestamp":1}"#).is_some());
        assert!(KeyboardMessage::parse(br#"{"type":"key_down","session_id":"s","code":"NoSuchKey","key":"a","repeat":false,"modifiers":{"alt":false,"ctrl":false,"meta":false,"shift":false},"sequence":1,"timestamp":1}"#).is_none());
        let long = "x".repeat(257);
        let json = format!(
            r#"{{"type":"text_input","session_id":"s","text":"{long}","sequence":1,"timestamp":1}}"#
        );
        assert!(KeyboardMessage::parse(json.as_bytes()).is_none());
    }
}
