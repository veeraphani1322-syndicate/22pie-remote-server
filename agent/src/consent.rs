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

pub async fn request_screen_view(viewer_name: String, device_name: String) -> ConsentDecision {
    tokio::task::spawn_blocking(move || prompt(&viewer_name, &device_name))
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
fn prompt(viewer_name: &str, device_name: &str) -> ConsentDecision {
    use windows_sys::Win32::UI::Controls::{
        TaskDialogIndirect, TASKDIALOGCONFIG, TASKDIALOG_BUTTON, TDF_ALLOW_DIALOG_CANCELLATION,
        TDF_USE_COMMAND_LINKS,
    };
    const ALLOW_ONCE: i32 = 1001;
    const TRUST_ACCOUNT: i32 = 1002;
    const DENY: i32 = 1003;
    let title = wide("22Pie Remote");
    let instruction = wide(&format!(
        "{viewer_name} is requesting access to this computer."
    ));
    let content = wide(&format!(
        "Computer: {device_name}\n\nRequested permission:\nScreen viewing"
    ));
    let allow = wide("Allow Once\nAllow screen viewing for only this session.");
    let trust =
        wide("Trust This Account\nAllow now and future screen-view sessions until revoked.");
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
        TaskDialogIndirect(
            &config,
            &mut selected,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result < 0 {
        return ConsentDecision::Deny;
    }
    match selected {
        ALLOW_ONCE => ConsentDecision::AllowOnce,
        TRUST_ACCOUNT => ConsentDecision::TrustAccount,
        _ => ConsentDecision::Deny,
    }
}

#[cfg(not(windows))]
fn prompt(_viewer_name: &str, _device_name: &str) -> ConsentDecision {
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

#[cfg(not(windows))]
pub fn show_sharing_indicator(stop: tokio::sync::oneshot::Sender<()>) {
    let _ = stop.send(());
}
