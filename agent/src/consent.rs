#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

pub async fn request_screen_view(viewer_name: String, device_name: String) -> bool {
    tokio::task::spawn_blocking(move || prompt(&viewer_name, &device_name))
        .await
        .unwrap_or(false)
}

#[cfg(windows)]
fn prompt(viewer_name: &str, device_name: &str) -> bool {
    use std::ffi::OsStr;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONINFORMATION, MB_SETFOREGROUND, MB_YESNO,
    };
    let message = format!(
        "Remote screen viewing requested.\n\n{viewer_name} wants to view this computer ({device_name}).\n\nScreen capture starts only if you choose Allow.\nYou can stop sharing at any time.\n\nAllow this session?"
    );
    let title: Vec<u16> = OsStr::new("22Pie Remote")
        .encode_wide()
        .chain(Some(0))
        .collect();
    let message: Vec<u16> = OsStr::new(&message).encode_wide().chain(Some(0)).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_YESNO | MB_ICONINFORMATION | MB_DEFBUTTON2 | MB_SETFOREGROUND,
        ) == IDYES
    }
}

#[cfg(not(windows))]
fn prompt(_viewer_name: &str, _device_name: &str) -> bool {
    false
}

#[cfg(windows)]
pub fn show_sharing_indicator(stop: tokio::sync::oneshot::Sender<()>) {
    std::thread::spawn(move || {
        use std::ffi::OsStr;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND,
        };
        let title: Vec<u16> = OsStr::new("22Pie Remote — Screen sharing active")
            .encode_wide()
            .chain(Some(0))
            .collect();
        let message: Vec<u16> = OsStr::new(
            "Your screen is being shared.\n\nSelect Stop Sharing to end the session immediately.",
        )
        .encode_wide()
        .chain(Some(0))
        .collect();
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND,
            );
        }
        let _ = stop.send(());
    });
}

#[cfg(not(windows))]
pub fn show_sharing_indicator(stop: tokio::sync::oneshot::Sender<()>) {
    let _ = stop.send(());
}
