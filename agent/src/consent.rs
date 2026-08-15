use crate::trusted_access::LocalTrust;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentDecision {
    AllowOnce,
    TrustAccount,
    Deny,
    ExistingTrust,
}

const ALLOW_ONCE: i32 = 1001;
const TRUST_ACCOUNT: i32 = 1002;
const DENY: i32 = 1003;

fn decision_from_button(button: i32) -> ConsentDecision {
    match button {
        ALLOW_ONCE => ConsentDecision::AllowOnce,
        TRUST_ACCOUNT => ConsentDecision::TrustAccount,
        _ => ConsentDecision::Deny,
    }
}

pub async fn request_screen_view(viewer_name: String, device_name: String) -> ConsentDecision {
    tokio::task::spawn_blocking(move || prompt(&viewer_name, &device_name, false))
        .await
        .unwrap_or(ConsentDecision::Deny)
}

pub async fn request_mouse_control(viewer_name: String, device_name: String) -> ConsentDecision {
    tokio::task::spawn_blocking(move || prompt(&viewer_name, &device_name, true))
        .await
        .unwrap_or(ConsentDecision::Deny)
}

pub async fn review_trusted_access(grants: &[LocalTrust]) -> bool {
    let grants = grants.to_vec();
    tokio::task::spawn_blocking(move || review(&grants))
        .await
        .unwrap_or(false)
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(Some(0))
        .collect()
}

#[cfg(windows)]
fn prompt(viewer_name: &str, device_name: &str, mouse_control: bool) -> ConsentDecision {
    try_task_dialog(viewer_name, device_name, mouse_control)
        .unwrap_or_else(|| fallback_consent_window(viewer_name, device_name, mouse_control))
}

#[cfg(windows)]
fn try_task_dialog(
    viewer_name: &str,
    device_name: &str,
    mouse_control: bool,
) -> Option<ConsentDecision> {
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows_sys::Win32::UI::Controls::{
        TASKDIALOGCONFIG, TASKDIALOG_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION, TDF_USE_COMMAND_LINKS,
    };

    type TaskDialogIndirectFn =
        unsafe extern "system" fn(*const TASKDIALOGCONFIG, *mut i32, *mut i32, *mut i32) -> i32;

    let library_name = wide("comctl32.dll");
    let library = unsafe { LoadLibraryW(library_name.as_ptr()) };
    if library.is_null() {
        return None;
    }
    let address = unsafe { GetProcAddress(library, c"TaskDialogIndirect".as_ptr().cast()) };
    let Some(address) = address else {
        unsafe { FreeLibrary(library) };
        return None;
    };
    let task_dialog: TaskDialogIndirectFn = unsafe { std::mem::transmute(address) };

    let title = wide("22Pie Remote");
    let instruction = wide(&format!(
        "{viewer_name} is requesting {}.",
        if mouse_control {
            "remote mouse control"
        } else {
            "access to this computer"
        }
    ));
    let content = wide(&format!(
        "Computer: {device_name}\n\nRequested permissions:\nScreen viewing{}",
        if mouse_control { "\nMouse control" } else { "" }
    ));
    let allow = wide("Allow Once\nAllow screen viewing for only this session.");
    let trust = wide(if mouse_control {
        "Trust These Permissions\nAllow mouse control now and in future sessions until revoked."
    } else {
        "Trust This Account\nAllow now and future screen-view sessions until revoked."
    });
    let deny = wide("Deny\nDo not allow this session or save authorization.");
    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: ALLOW_ONCE,
            pszButtonText: allow.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: TRUST_ACCOUNT,
            pszButtonText: trust.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: DENY,
            pszButtonText: deny.as_ptr(),
        },
    ];
    let mut config: TASKDIALOGCONFIG = unsafe { std::mem::zeroed() };
    config.cbSize = std::mem::size_of::<TASKDIALOGCONFIG>() as u32;
    config.dwFlags = TDF_USE_COMMAND_LINKS | TDF_ALLOW_DIALOG_CANCELLATION;
    config.pszWindowTitle = title.as_ptr();
    config.pszMainInstruction = instruction.as_ptr();
    config.pszContent = content.as_ptr();
    config.cButtons = buttons.len() as u32;
    config.pButtons = buttons.as_ptr();
    config.nDefaultButton = DENY;
    let mut selected = DENY;
    let result = unsafe {
        task_dialog(
            &config,
            &mut selected,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    unsafe { FreeLibrary(library) };
    if result < 0 {
        return None;
    }
    Some(decision_from_button(selected))
}

#[cfg(windows)]
fn fallback_consent_window(
    viewer_name: &str,
    device_name: &str,
    mouse_control: bool,
) -> ConsentDecision {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::UpdateWindow;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        GetWindowLongPtrW, LoadCursorW, RegisterClassW, SetForegroundWindow, SetWindowLongPtrW,
        ShowWindow, TranslateMessage, BS_PUSHBUTTON, CW_USEDEFAULT, GWLP_USERDATA, IDC_ARROW, MSG,
        SW_SHOW, WM_CLOSE, WM_COMMAND, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_OVERLAPPED, WS_SYSMENU,
        WS_VISIBLE,
    };

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_COMMAND => {
                let button = (wparam & 0xffff) as i32;
                if matches!(button, ALLOW_ONCE | TRUST_ACCOUNT | DENY) {
                    let selected = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut i32;
                    if !selected.is_null() {
                        *selected = button;
                    }
                    DestroyWindow(window);
                    return 0;
                }
            }
            WM_CLOSE => {
                let selected = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut i32;
                if !selected.is_null() {
                    *selected = DENY;
                }
                DestroyWindow(window);
                return 0;
            }
            _ => {}
        }
        DefWindowProcW(window, message, wparam, lparam)
    }

    unsafe fn child(
        class_name: &[u16],
        text: &[u16],
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: HWND,
        id: i32,
    ) {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            parent,
            id as usize as *mut c_void,
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        );
    }

    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let class_name = wide("22PieConsentFallbackWindow");
    let title = wide("22Pie Remote");
    let mut class: WNDCLASSW = unsafe { std::mem::zeroed() };
    class.lpfnWndProc = Some(window_proc);
    class.hInstance = instance;
    class.hCursor = unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) };
    class.lpszClassName = class_name.as_ptr();
    unsafe { RegisterClassW(&class) };

    let window = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            300,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if window.is_null() {
        return ConsentDecision::Deny;
    }

    let mut selected = 0i32;
    unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, (&mut selected as *mut i32) as isize) };
    let static_class = wide("STATIC");
    let button_class = wide("BUTTON");
    let message = wide(&format!("{viewer_name} is requesting {}.\n\nComputer: {device_name}\nRequested permissions: Screen viewing{}", if mouse_control { "remote mouse control" } else { "access to this computer" }, if mouse_control { ", Mouse control" } else { "" }));
    unsafe {
        child(&static_class, &message, 0, 20, 18, 510, 100, window, 0);
        child(
            &button_class,
            &wide("Allow Once"),
            BS_PUSHBUTTON as u32,
            20,
            145,
            150,
            48,
            window,
            ALLOW_ONCE,
        );
        child(
            &button_class,
            &wide(if mouse_control {
                "Trust These Permissions"
            } else {
                "Trust This Account"
            }),
            BS_PUSHBUTTON as u32,
            190,
            145,
            170,
            48,
            window,
            TRUST_ACCOUNT,
        );
        child(
            &button_class,
            &wide("Deny"),
            BS_PUSHBUTTON as u32,
            380,
            145,
            150,
            48,
            window,
            DENY,
        );
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        SetForegroundWindow(window);
    }

    let mut message: MSG = unsafe { std::mem::zeroed() };
    while selected == 0 && unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    decision_from_button(selected)
}

#[cfg(not(windows))]
fn prompt(_viewer_name: &str, _device_name: &str, _mouse_control: bool) -> ConsentDecision {
    ConsentDecision::Deny
}

#[cfg(windows)]
fn review(grants: &[LocalTrust]) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONINFORMATION, MB_SETFOREGROUND, MB_YESNO,
    };
    let entries = grants
        .iter()
        .map(|grant| format!("{} — Screen viewing", grant.viewer_name))
        .collect::<Vec<_>>()
        .join("\n");
    let message = wide(&format!("Trusted Access\n\n{entries}\n\nRevoke all locally trusted screen-view accounts?\n\nYes = Revoke\nNo = Keep trusted access"));
    let title = wide("22Pie Remote — Trusted Access");
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
fn review(_grants: &[LocalTrust]) -> bool {
    false
}

#[cfg(windows)]
pub fn show_sharing_indicator(stop: tokio::sync::oneshot::Sender<()>) {
    std::thread::spawn(move || {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND,
        };
        let title = wide("22Pie Remote — Screen sharing active");
        let message = wide(
            "Your screen is being shared.\n\nSelect Stop Sharing to end the session immediately.",
        );
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

#[cfg(windows)]
pub fn show_mouse_control_indicator(stop: tokio::sync::oneshot::Sender<()>) {
    std::thread::spawn(move || {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND,
        };
        let title = wide("22Pie Remote — Mouse control active");
        let message = wide("Your screen is being shared and remotely controlled.\n\nSelect OK to stop remote mouse control. Screen viewing will continue.");
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
pub fn show_mouse_control_indicator(stop: tokio::sync::oneshot::Sender<()>) {
    let _ = stop.send(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_consent_buttons_map_to_the_expected_decisions() {
        assert_eq!(decision_from_button(ALLOW_ONCE), ConsentDecision::AllowOnce);
        assert_eq!(
            decision_from_button(TRUST_ACCOUNT),
            ConsentDecision::TrustAccount
        );
        assert_eq!(decision_from_button(DENY), ConsentDecision::Deny);
        assert_eq!(decision_from_button(0), ConsentDecision::Deny);
    }
}

#[cfg(not(windows))]
pub fn show_sharing_indicator(stop: tokio::sync::oneshot::Sender<()>) {
    let _ = stop.send(());
}
