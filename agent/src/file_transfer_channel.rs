use crate::file_transfer::{validate_offer, Message, Receiver, MAX_MESSAGE_BYTES};
use anyhow::Result;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::{mpsc, watch};
use webrtc::{
    data_channel::data_channel_message::DataChannelMessage, peer_connection::RTCPeerConnection,
};

pub async fn attach(
    peer: &RTCPeerConnection,
    session: String,
    prompt_gate: Arc<AtomicBool>,
    release_input: impl Fn() + Send + Sync + 'static,
) -> Result<()> {
    let channel = peer.create_data_channel("files-v1", None).await?;
    let weak_channel = Arc::downgrade(&channel);
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(8);
    let (closed_tx, mut closed_rx) = watch::channel(false);
    let stopped = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    let message_cancelled = cancelled.clone();
    let message_stop = stopped.clone();
    let overflow_close = closed_tx.clone();
    let message_session = session.clone();
    channel.on_message(Box::new(move |data: DataChannelMessage| {
        if let Ok(message) = Message::parse(&data.data, &message_session) {
            match message {
                Message::FileOffer { .. } => message_cancelled.store(false, Ordering::Release),
                Message::FileCancel { .. } => message_cancelled.store(true, Ordering::Release),
                _ => {}
            }
        }
        if !data.is_string
            || data.data.len() > MAX_MESSAGE_BYTES
            || tx.try_send(data.data.to_vec()).is_err()
        {
            message_stop.store(true, Ordering::Release);
            let _ = overflow_close.send(true);
        }
        Box::pin(async {})
    }));
    let close_stop = stopped.clone();
    channel.on_close(Box::new(move || {
        close_stop.store(true, Ordering::Release);
        let _ = closed_tx.send(true);
        Box::pin(async {})
    }));
    tokio::spawn(async move {
        let mut receiver: Option<Receiver> = None;
        let mut offers = 0;
        loop {
            let data = tokio::select! {
                _ = closed_rx.changed() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(90)), if receiver.is_some() => break,
                data = rx.recv() => { match data { Some(data) => data, None => break } }
            };
            if stopped.load(Ordering::Acquire) {
                break;
            }
            let message = match Message::parse(&data, &session) {
                Ok(message) => message,
                Err(_) => break,
            };
            let id = message.id();
            let result: Result<serde_json::Value> = async {
                match message {
                    Message::FileOffer {
                        name, size, sha256, ..
                    } => {
                        if receiver.is_some() {
                            anyhow::bail!("A file transfer is already active");
                        }
                        offers += 1;
                        if offers > 10 {
                            anyhow::bail!(
                                "File request limit reached; reconnect to send more files"
                            );
                        }
                        validate_offer(&name, size, &sha256)?;
                        prompt_gate.store(true, Ordering::Release);
                        release_input();
                        let approved =
                            confirm_file(name.clone(), size, stopped.clone(), cancelled.clone())
                                .await;
                        prompt_gate.store(false, Ordering::Release);
                        if stopped.load(Ordering::Acquire) {
                            anyhow::bail!("Session ended");
                        }
                        if !approved {
                            anyhow::bail!("File declined or approval expired");
                        }
                        let dirs = directories::UserDirs::new()
                            .ok_or_else(|| anyhow::anyhow!("Downloads folder unavailable"))?;
                        let root = dirs
                            .download_dir()
                            .ok_or_else(|| anyhow::anyhow!("Downloads folder unavailable"))?
                            .join("22Pie Transfers");
                        receiver = Some(Receiver::start(&root, id, &name, size, &sha256)?);
                        Ok(json!({"type":"file_ready","offset":0}))
                    }
                    Message::FileChunk { offset, data, .. } => {
                        let offset = receiver
                            .as_mut()
                            .ok_or_else(|| anyhow::anyhow!("File has not been accepted"))?
                            .chunk(id, offset, &data)?;
                        Ok(json!({"type":"file_ack","offset":offset}))
                    }
                    Message::FileFinish { .. } => {
                        receiver
                            .as_mut()
                            .ok_or_else(|| anyhow::anyhow!("File has not been accepted"))?
                            .finish(id)?;
                        receiver = None;
                        Ok(json!({"type":"file_complete"}))
                    }
                    Message::FileCancel { .. } => {
                        if receiver.as_ref().is_some_and(|r| r.id != id) {
                            anyhow::bail!("Unexpected transfer");
                        }
                        receiver = None;
                        Ok(json!({"type":"file_cancelled"}))
                    }
                }
            }
            .await;
            let mut response = match result {
                Ok(response) => response,
                Err(error) => {
                    receiver = None;
                    json!({"type":"file_error","message":error.to_string()})
                }
            };
            if stopped.load(Ordering::Acquire) {
                break;
            }
            response["session_id"] = json!(session);
            response["transfer_id"] = json!(id);
            let Some(channel) = weak_channel.upgrade() else {
                break;
            };
            if channel.send_text(response.to_string()).await.is_err() {
                break;
            }
        }
        stopped.store(true, Ordering::Release);
        prompt_gate.store(false, Ordering::Release);
        drop(receiver);
        if let Some(channel) = weak_channel.upgrade() {
            let _ = channel.close().await;
        }
    });
    Ok(())
}

async fn confirm_file(
    name: String,
    size: u64,
    stopped: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> bool {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let spawn = std::thread::Builder::new()
        .name("file-consent".into())
        .spawn(move || {
            let approved = consent_window(&name, size, stopped, cancelled);
            let _ = tx.send(approved);
        });
    if spawn.is_err() {
        return false;
    }
    rx.await.unwrap_or(false)
}

fn consent_window(
    name: &str,
    size: u64,
    stopped: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) -> bool {
    use std::{
        ffi::c_void,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    };
    struct State {
        accepted: bool,
        cancelled: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
        deadline: Instant,
    }
    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        let state = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut State;
        match message {
            WM_COMMAND if !state.is_null() => {
                let id = w & 0xffff;
                if id == 1 || id == 2 {
                    (*state).accepted = id == 1
                        && !(*state).stopped.load(Ordering::Acquire)
                        && !(*state).cancelled.load(Ordering::Acquire)
                        && Instant::now() < (*state).deadline;
                    DestroyWindow(window);
                    return 0;
                }
            }
            WM_TIMER if !state.is_null() => {
                if (*state).stopped.load(Ordering::Acquire)
                    || (*state).cancelled.load(Ordering::Acquire)
                    || Instant::now() >= (*state).deadline
                {
                    DestroyWindow(window);
                }
                return 0;
            }
            WM_CLOSE => {
                DestroyWindow(window);
                return 0;
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return 0;
            }
            _ => {}
        }
        DefWindowProcW(window, message, w, l)
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    let mut state = State {
        accepted: false,
        cancelled,
        stopped,
        deadline: Instant::now() + Duration::from_secs(60),
    };
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("22PieFileConsent");
        let mut class: WNDCLASSW = std::mem::zeroed();
        class.lpfnWndProc = Some(procedure);
        class.hInstance = instance;
        class.lpszClassName = class_name.as_ptr();
        class.hCursor = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        class.hbrBackground = 6usize as _;
        RegisterClassW(&class);
        let window = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name.as_ptr(),
            wide("22Pie — incoming file").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            540,
            250,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if window.is_null() {
            return false;
        }
        SetWindowLongPtrW(window, GWLP_USERDATA, &mut state as *mut State as isize);
        let body = format!("The connected viewer wants to send:\n{name}\n{size} bytes\n\nSave in Downloads / 22Pie Transfers?\nThe file will not be opened. This request expires in 60 seconds.");
        CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide(&body).as_ptr(),
            WS_CHILD | WS_VISIBLE,
            16,
            12,
            500,
            140,
            window,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        for (id, label, x) in [(1, "Accept file", 230), (2, "Deny", 365)] {
            CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                x,
                165,
                120,
                30,
                window,
                id as usize as *mut c_void,
                instance,
                std::ptr::null(),
            );
        }
        if SetTimer(window, 1, 250, None) == 0 {
            DestroyWindow(window);
            return false;
        }
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    state.accepted
        && !state.stopped.load(Ordering::Acquire)
        && !state.cancelled.load(Ordering::Acquire)
}
