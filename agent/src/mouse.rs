use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum MouseMessage {
    MouseMove {
        session_id: String,
        x: f64,
        y: f64,
        sequence: u64,
        timestamp: f64,
    },
    MouseDown {
        session_id: String,
        button: MouseButton,
        sequence: u64,
        timestamp: f64,
    },
    MouseUp {
        session_id: String,
        button: MouseButton,
        sequence: u64,
        timestamp: f64,
    },
    MouseScroll {
        session_id: String,
        dx: i32,
        dy: i32,
        sequence: u64,
        timestamp: f64,
    },
}

impl MouseMessage {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let message: Self = serde_json::from_slice(data).ok()?;
        let valid = match &message {
            Self::MouseMove {
                x, y, timestamp, ..
            } => {
                x.is_finite()
                    && y.is_finite()
                    && timestamp.is_finite()
                    && (0.0..=1.0).contains(x)
                    && (0.0..=1.0).contains(y)
            }
            Self::MouseScroll {
                dx, dy, timestamp, ..
            } => timestamp.is_finite() && dx.abs() <= 1200 && dy.abs() <= 1200,
            Self::MouseDown { timestamp, .. } | Self::MouseUp { timestamp, .. } => {
                timestamp.is_finite()
            }
        };
        valid.then_some(message)
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::MouseMove { session_id, .. }
            | Self::MouseDown { session_id, .. }
            | Self::MouseUp { session_id, .. }
            | Self::MouseScroll { session_id, .. } => session_id,
        }
    }
}

#[cfg(windows)]
pub struct MouseController {
    held: std::collections::HashSet<MouseButton>,
}

#[cfg(windows)]
impl MouseController {
    pub fn new() -> Self {
        Self {
            held: Default::default(),
        }
    }
    pub fn execute(&mut self, message: MouseMessage) -> bool {
        match message {
            MouseMessage::MouseMove { x, y, .. } => self.send(
                (x * 65535.0).round() as i32,
                (y * 65535.0).round() as i32,
                0,
                flags::MOVE | flags::ABSOLUTE,
            ),
            MouseMessage::MouseDown { button, .. } => {
                self.held.insert(button);
                self.send(0, 0, 0, down_flag(button))
            }
            MouseMessage::MouseUp { button, .. } => {
                self.held.remove(&button);
                self.send(0, 0, 0, up_flag(button))
            }
            MouseMessage::MouseScroll { dx, dy, .. } => {
                let vertical = dy == 0 || self.send(0, 0, dy as u32, flags::WHEEL);
                let horizontal = dx == 0 || self.send(0, 0, dx as u32, flags::HWHEEL);
                vertical && horizontal
            }
        }
    }
    pub fn release_all(&mut self) {
        for button in self.held.drain().collect::<Vec<_>>() {
            let _ = send_raw(0, 0, 0, up_flag(button));
        }
    }
    fn send(&self, x: i32, y: i32, data: u32, flags: u32) -> bool {
        send_raw(x, y, data, flags)
    }
}

#[cfg(windows)]
mod flags {
    pub use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_ABSOLUTE as ABSOLUTE, MOUSEEVENTF_HWHEEL as HWHEEL, MOUSEEVENTF_MOVE as MOVE,
        MOUSEEVENTF_WHEEL as WHEEL,
    };
}

#[cfg(windows)]
fn down_flag(button: MouseButton) -> u32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    match button {
        MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
        MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
        MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
    }
}
#[cfg(windows)]
fn up_flag(button: MouseButton) -> u32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    match button {
        MouseButton::Left => MOUSEEVENTF_LEFTUP,
        MouseButton::Right => MOUSEEVENTF_RIGHTUP,
        MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
    }
}

#[cfg(windows)]
fn send_raw(x: i32, y: i32, data: u32, flags: u32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT,
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: x,
                dy: y,
                mouseData: data,
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
    fn validates_mouse_protocol() {
        assert!(MouseMessage::parse(
            br#"{"type":"mouse_move","session_id":"s","x":0.5,"y":1.0,"sequence":1,"timestamp":1}"#
        )
        .is_some());
        assert!(MouseMessage::parse(br#"{"type":"mouse_move","session_id":"s","x":-0.1,"y":0.5,"sequence":1,"timestamp":1}"#).is_none());
        assert!(MouseMessage::parse(br#"{"type":"mouse_scroll","session_id":"s","dx":0,"dy":9999,"sequence":1,"timestamp":1}"#).is_none());
        assert!(MouseMessage::parse(br#"{"type":"mouse_down","session_id":"s","button":"invalid","sequence":1,"timestamp":1}"#).is_none());
    }
}
