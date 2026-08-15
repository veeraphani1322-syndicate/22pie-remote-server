use crate::media::{MediaCommand, MediaEvent, MediaSession};
use crate::{
    config::Config,
    consent::ConsentDecision,
    device::DeviceIdentity,
    protocol::{AgentMessage, DeviceStatus, ServerMessage},
    trusted_access::TrustedAccess,
};
use anyhow::{anyhow, Context, Result};
use futures_util::future::pending;
use futures_util::{SinkExt, StreamExt};
use std::{
    cmp::min,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio::{sync::watch, time};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

const MAX_BACKOFF: Duration = Duration::from_secs(30);
const REGISTRATION_TIMEOUT: Duration = Duration::from_secs(10);

enum ConnectionEnd {
    Disconnected,
    Shutdown,
}

enum IncomingMessage {
    Protocol(ServerMessage),
    Closed,
    ControlFrame,
}

pub async fn run(
    config: Config,
    device: DeviceIdentity,
    mut trusted_access: TrustedAccess,
    mut local_commands: mpsc::Receiver<()>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut backoff = Duration::from_secs(1);

    loop {
        if *shutdown.borrow() {
            break;
        }

        println!("Connecting...");
        info!(url = %config.server_url, "Connecting");
        match connect_async(&config.server_url).await {
            Ok((stream, _)) => {
                info!("Connected");
                println!("Connected");
                backoff = Duration::from_secs(1);
                match handle_connection(
                    stream,
                    &config,
                    &device,
                    &mut trusted_access,
                    &mut local_commands,
                    &mut shutdown,
                )
                .await
                {
                    Ok(ConnectionEnd::Shutdown) => break,
                    Ok(ConnectionEnd::Disconnected) => info!("Disconnected"),
                    Err(error) => warn!(error = %error, "Connection ended"),
                }
            }
            Err(error) => {
                println!("Connection failed");
                warn!(error = %error, "Connection failed");
            }
        }

        if *shutdown.borrow() {
            break;
        }
        println!(
            "Retrying in {} second{}",
            backoff.as_secs(),
            if backoff.as_secs() == 1 { "" } else { "s" }
        );
        info!(seconds = backoff.as_secs(), "Retrying");
        tokio::select! {
            _ = time::sleep(backoff) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() { break; }
            }
        }
        backoff = min(backoff.saturating_mul(2), MAX_BACKOFF);
    }
    info!("Agent stopped");
}

async fn handle_connection<S>(
    stream: tokio_tungstenite::WebSocketStream<S>,
    config: &Config,
    device: &DeviceIdentity,
    trusted_access: &mut TrustedAccess,
    local_commands: &mut mpsc::Receiver<()>,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<ConnectionEnd>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut writer, mut reader) = stream.split();
    send(&mut writer, &AgentMessage::register(device)).await?;

    let registration = time::timeout(REGISTRATION_TIMEOUT, async {
        let mut challenge_answered = false;
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok::<_, anyhow::Error>(None);
                    }
                }
                incoming = reader.next() => {
                    let message = parse_incoming(incoming)?;
                    match message {
                        IncomingMessage::Protocol(ServerMessage::AuthChallenge { nonce }) => {
                            if challenge_answered { return Err(anyhow!("server sent more than one authentication challenge")); }
                            let signature = device.sign_challenge(&nonce);
                            send(&mut writer, &AgentMessage::Authenticate { signature }).await?;
                            challenge_answered = true;
                        }
                        IncomingMessage::Protocol(ServerMessage::Registered { device_id, status: DeviceStatus::Online }) => {
                            if device_id != device.device_id.to_string() {
                                return Err(anyhow!("server registered an unexpected device ID"));
                            }
                            return Ok(Some(()));
                        }
                        IncomingMessage::Protocol(ServerMessage::Error { code, message }) => {
                            return Err(anyhow!("server rejected registration ({code}): {message}"));
                        }
                        IncomingMessage::Protocol(other) => debug!(?other, "Ignoring message while registering"),
                        IncomingMessage::ControlFrame => {}
                        IncomingMessage::Closed => return Err(anyhow!("server disconnected during registration")),
                    }
                }
            }
        }
    })
    .await
    .context("registration timed out")??;

    if registration.is_none() {
        writer.send(Message::Close(None)).await.ok();
        writer.close().await.ok();
        return Ok(ConnectionEnd::Shutdown);
    }
    info!("Registered successfully");
    let pending_revocations = trusted_access.pending_revocations_for(&config.server_url);
    for revoked in &pending_revocations {
        send(
            &mut writer,
            &AgentMessage::TrustRevoke {
                user_id: &revoked.user_id,
                permission: if revoked.permission == "MOUSE_CONTROL" {
                    crate::protocol::Permission::MouseControl
                } else {
                    crate::protocol::Permission::ScreenView
                },
            },
        )
        .await?;
    }
    if !pending_revocations.is_empty() {
        trusted_access.clear_pending_revocations_for(&config.server_url)?;
    }
    println!("Registered successfully\n\nStatus:\nONLINE\n\nPress Ctrl+C to stop.\n");

    let mut heartbeat = time::interval(config.heartbeat_interval);
    heartbeat.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let mut awaiting_ack = false;
    let (consent_tx, mut consent_rx) = mpsc::channel::<(
        String,
        String,
        String,
        ConsentDecision,
        Vec<crate::protocol::IceServer>,
    )>(1);
    let mut consent_pending: Option<String> = None;
    let mut consent_started_at: Option<Instant> = None;
    let mut media: Option<MediaSession> = None;
    let mut active_session_id: Option<String> = None;
    let mut local_stop: Option<tokio::sync::oneshot::Receiver<()>> = None;
    let mut local_mouse_stop: Option<tokio::sync::oneshot::Receiver<()>> = None;
    let (mouse_consent_tx, mut mouse_consent_rx) =
        mpsc::channel::<(String, String, String, ConsentDecision)>(1);
    let mut mouse_consent_pending: Option<String> = None;
    let (review_tx, mut review_rx) = mpsc::channel(1);
    let mut review_pending = false;
    let mut local_commands_open = true;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    writer.send(Message::Close(None)).await.ok();
                    writer.close().await.ok();
                    return Ok(ConnectionEnd::Shutdown);
                }
            }
            command = local_commands.recv(), if local_commands_open => {
                if command.is_none() { local_commands_open = false; }
                if command.is_some() && !review_pending && !trusted_access.grants().is_empty() {
                    review_pending = true;
                    let grants = trusted_access.grants().to_vec();
                    let tx = review_tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(crate::consent::review_trusted_access(&grants).await).await;
                    });
                }
            }
            review = review_rx.recv() => {
                review_pending = false;
                if review == Some(true) {
                    trusted_access.revoke_all_locally()?;
                    let revoked = trusted_access.pending_revocations_for(&config.server_url);
                    for record in &revoked {
                        send(&mut writer, &AgentMessage::TrustRevoke { user_id: &record.user_id, permission: if record.permission == "MOUSE_CONTROL" { crate::protocol::Permission::MouseControl } else { crate::protocol::Permission::ScreenView } }).await?;
                    }
                    trusted_access.clear_pending_revocations_for(&config.server_url)?;
                    info!(count = revoked.len(), "Trusted screen access revoked locally");
                }
            }
            _ = heartbeat.tick() => {
                if awaiting_ack {
                    warn!("Previous heartbeat was not acknowledged");
                }
                send(&mut writer, &AgentMessage::Heartbeat).await?;
                awaiting_ack = true;
            }
            incoming = reader.next() => {
                match parse_incoming(incoming)? {
                    IncomingMessage::Protocol(ServerMessage::HeartbeatAck { timestamp }) => {
                        awaiting_ack = false;
                        debug!(server_timestamp = %timestamp, "Heartbeat acknowledged");
                    }
                    IncomingMessage::Protocol(ServerMessage::Error { code, message }) => {
                        warn!(%code, %message, "Server reported an error");
                    }
                    IncomingMessage::Protocol(ServerMessage::SessionRequested { session_id, viewer_user_id, viewer_name, permissions, trusted, ice_servers }) => {
                        info!(%session_id, "Screen-view session request received");
                        if consent_pending.is_some() || media.is_some() || permissions != vec![crate::protocol::Permission::ScreenView] {
                            send(&mut writer, &AgentMessage::SessionReject { session_id: &session_id, reason: "another_request_pending" }).await?;
                        } else {
                            consent_pending = Some(session_id.clone());
                            consent_started_at = Some(Instant::now());
                            if trusted && trusted_access.has_screen_view(&config.server_url, &viewer_user_id) {
                                let _ = consent_tx.send((session_id, viewer_user_id, viewer_name, ConsentDecision::ExistingTrust, ice_servers)).await;
                            } else {
                                let tx = consent_tx.clone();
                                let device_name = device.device_name.clone();
                                tokio::spawn(async move {
                                    let decision = crate::consent::request_screen_view(viewer_name.clone(), device_name).await;
                                    let _ = tx.send((session_id, viewer_user_id, viewer_name, decision, ice_servers)).await;
                                });
                            }
                        }
                    }
                    IncomingMessage::Protocol(ServerMessage::MouseControlRequested { session_id, viewer_user_id, viewer_name, trusted }) => {
                        if active_session_id.as_deref() != Some(&session_id) || mouse_consent_pending.is_some() {
                            send(&mut writer, &AgentMessage::MouseControlReject { session_id: &session_id, reason: "session_not_available" }).await?;
                        } else if trusted && trusted_access.has_mouse_control(&config.server_url, &viewer_user_id) {
                            if let Some(media) = &media { let _ = media.commands.send(MediaCommand::SetMouseControl(true)).await; }
                            let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
                            crate::consent::show_mouse_control_indicator(stop_tx);
                            local_mouse_stop = Some(stop_rx);
                            send(&mut writer, &AgentMessage::MouseControlAccept { session_id: &session_id }).await?;
                        } else {
                            mouse_consent_pending = Some(session_id.clone());
                            let tx = mouse_consent_tx.clone();
                            let device_name = device.device_name.clone();
                            tokio::spawn(async move {
                                let decision = crate::consent::request_mouse_control(viewer_name.clone(), device_name).await;
                                let _ = tx.send((session_id, viewer_user_id, viewer_name, decision)).await;
                            });
                        }
                    }
                    IncomingMessage::Protocol(ServerMessage::MouseControlDisabled { session_id }) => {
                        if active_session_id.as_deref() == Some(&session_id) {
                            if let Some(media) = &media { let _ = media.commands.send(MediaCommand::SetMouseControl(false)).await; }
                            local_mouse_stop = None;
                        }
                    }
                    IncomingMessage::Protocol(ServerMessage::MouseControlEnabled { session_id }) => {
                        if active_session_id.as_deref() == Some(&session_id) {
                            if let Some(media) = &media { let _ = media.commands.send(MediaCommand::SetMouseControl(true)).await; }
                        }
                    }
                    IncomingMessage::Protocol(ServerMessage::WebrtcAnswer { session_id, sdp }) => {
                        if active_session_id.as_deref() == Some(&session_id) {
                            if let Some(media) = &media { let _ = media.commands.send(MediaCommand::Answer(sdp)).await; }
                        }
                        if mouse_consent_pending.as_deref() == Some(&session_id) { mouse_consent_pending = None; }
                    }
                    IncomingMessage::Protocol(ServerMessage::IceCandidate { session_id, candidate, sdp_mid, sdp_m_line_index }) => {
                        if active_session_id.as_deref() == Some(&session_id) {
                            if let Some(media) = &media { let _ = media.commands.send(MediaCommand::IceCandidate { candidate, sdp_mid, sdp_m_line_index }).await; }
                        }
                    }
                    IncomingMessage::Protocol(ServerMessage::SessionEnded { session_id, reason }) => {
                        if consent_pending.as_deref() == Some(&session_id) {
                            consent_pending = None;
                            consent_started_at = None;
                            info!(%session_id, %reason, "Pending screen-sharing consent cancelled");
                        }
                        if active_session_id.as_deref() == Some(&session_id) {
                            if let Some(media) = &media { let _ = media.commands.send(MediaCommand::Stop).await; }
                            media = None; active_session_id = None; local_stop = None; local_mouse_stop = None;
                            info!(%session_id, %reason, "Screen sharing ended");
                        }
                    }
                    IncomingMessage::Protocol(ServerMessage::TrustRevoked { user_id, permission: crate::protocol::Permission::ScreenView }) => {
                        trusted_access.revoke_screen_view(&config.server_url, &user_id)?;
                        info!(%user_id, "Local screen-view trust revoked by server");
                    }
                    IncomingMessage::Protocol(ServerMessage::TrustRevoked { user_id, permission: crate::protocol::Permission::MouseControl }) => {
                        trusted_access.revoke_mouse_control(&config.server_url, &user_id)?;
                        info!(%user_id, "Local mouse-control trust revoked by server");
                    }
                    IncomingMessage::Protocol(other) => debug!(?other, "Ignoring unexpected server message"),
                    IncomingMessage::ControlFrame => {}
                    IncomingMessage::Closed => return Ok(ConnectionEnd::Disconnected),
                }
            }
            consent = consent_rx.recv() => {
                if let Some((session_id, viewer_user_id, viewer_name, decision, ice_servers)) = consent {
                    if consent_pending.as_deref() != Some(&session_id) {
                        debug!(%session_id, "Ignoring stale screen-sharing consent result");
                        continue;
                    }
                    consent_pending = None;
                    if let Some(started_at) = consent_started_at.take() {
                        debug!(CONSENT_MS = started_at.elapsed().as_millis(), %session_id, ?decision, "Screen-sharing consent completed");
                    }
                    if decision != ConsentDecision::Deny {
                        if decision == ConsentDecision::TrustAccount {
                            trusted_access.grant_screen_view(&config.server_url, &viewer_user_id, &viewer_name)?;
                            send(&mut writer, &AgentMessage::TrustGrant { session_id: &session_id, permission: crate::protocol::Permission::ScreenView }).await?;
                        }
                        let media_started_at = Instant::now();
                        match crate::media::start(ice_servers, session_id.clone()).await {
                            Ok(started) => {
                                debug!(MEDIA_STARTUP_MS = media_started_at.elapsed().as_millis(), %session_id, "Screen media initialized");
                                let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
                                crate::consent::show_sharing_indicator(stop_tx);
                                active_session_id = Some(session_id.clone()); media = Some(started); local_stop = Some(stop_rx);
                                send(&mut writer, &AgentMessage::SessionAccept { session_id: &session_id }).await?;
                            }
                            Err(error) => {
                                warn!(%error, "Could not start screen sharing");
                                send(&mut writer, &AgentMessage::SessionReject { session_id: &session_id, reason: "screen_capture_failed" }).await?;
                            }
                        }
                    } else {
                        send(&mut writer, &AgentMessage::SessionReject { session_id: &session_id, reason: "denied_by_remote_user" }).await?;
                    }
                }
            }
            consent = mouse_consent_rx.recv() => {
                if let Some((session_id, viewer_user_id, viewer_name, decision)) = consent {
                    if mouse_consent_pending.as_deref() != Some(&session_id) { continue; }
                    mouse_consent_pending = None;
                    if decision == ConsentDecision::Deny {
                        send(&mut writer, &AgentMessage::MouseControlReject { session_id: &session_id, reason: "denied_by_remote_user" }).await?;
                    } else if active_session_id.as_deref() == Some(&session_id) {
                        if decision == ConsentDecision::TrustAccount {
                            trusted_access.grant_mouse_control(&config.server_url, &viewer_user_id, &viewer_name)?;
                            send(&mut writer, &AgentMessage::TrustGrant { session_id: &session_id, permission: crate::protocol::Permission::MouseControl }).await?;
                        }
                        if let Some(media) = &media { let _ = media.commands.send(MediaCommand::SetMouseControl(true)).await; }
                        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
                        crate::consent::show_mouse_control_indicator(stop_tx);
                        local_mouse_stop = Some(stop_rx);
                        send(&mut writer, &AgentMessage::MouseControlAccept { session_id: &session_id }).await?;
                    }
                }
            }
            event = async { match media.as_mut() { Some(media) => media.events.recv().await, None => pending().await } } => {
                if let (Some(event), Some(session_id)) = (event, active_session_id.clone()) {
                    match event {
                        MediaEvent::Offer(sdp) => send(&mut writer, &AgentMessage::WebrtcOffer { session_id: &session_id, sdp: &sdp }).await?,
                        MediaEvent::IceCandidate { candidate, sdp_mid, sdp_m_line_index } => send(&mut writer, &AgentMessage::IceCandidate { session_id: &session_id, candidate: &candidate, sdp_mid: sdp_mid.as_deref(), sdp_m_line_index }).await?,
                        MediaEvent::Connected => info!(%session_id, "Screen viewer connected"),
                        MediaEvent::Ended(reason) => {
                            send(&mut writer, &AgentMessage::SessionEnd { session_id: &session_id, reason: &reason }).await?;
                            media = None; active_session_id = None; local_stop = None; local_mouse_stop = None;
                        }
                    }
                }
            }
            _ = async { match local_stop.as_mut() { Some(stop) => { let _ = stop.await; }, None => pending().await } } => {
                if let Some(session_id) = active_session_id.clone() {
                    if let Some(media) = &media { let _ = media.commands.send(MediaCommand::Stop).await; }
                    send(&mut writer, &AgentMessage::SessionEnd { session_id: &session_id, reason: "stopped_by_remote_user" }).await?;
                    media = None; active_session_id = None; local_stop = None; local_mouse_stop = None;
                }
            }
            _ = async { match local_mouse_stop.as_mut() { Some(stop) => { let _ = stop.await; }, None => pending().await } } => {
                local_mouse_stop = None;
                if let Some(session_id) = active_session_id.clone() {
                    if let Some(media) = &media { let _ = media.commands.send(MediaCommand::SetMouseControl(false)).await; }
                    send(&mut writer, &AgentMessage::MouseControlReject { session_id: &session_id, reason: "stopped_by_remote_user" }).await?;
                }
            }
        }
    }
}

async fn send<S>(
    writer: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<S>, Message>,
    message: &AgentMessage<'_>,
) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let json = serde_json::to_string(message).context("failed to serialize protocol message")?;
    writer
        .send(Message::Text(json.into()))
        .await
        .context("failed to send protocol message")
}

fn parse_incoming(
    incoming: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> Result<IncomingMessage> {
    match incoming {
        Some(Ok(Message::Text(text))) => serde_json::from_str(&text)
            .map(IncomingMessage::Protocol)
            .context("server sent an invalid protocol message"),
        Some(Ok(Message::Close(_))) | None => Ok(IncomingMessage::Closed),
        Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {
            Ok(IncomingMessage::ControlFrame)
        }
        Some(Ok(Message::Binary(_))) => Err(anyhow!("server sent unsupported binary data")),
        Some(Ok(Message::Frame(_))) => Ok(IncomingMessage::ControlFrame),
        Some(Err(error)) => Err(error).context("WebSocket receive failed"),
    }
}
