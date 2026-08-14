use crate::{
    config::Config,
    device::DeviceIdentity,
    protocol::{AgentMessage, DeviceStatus, ServerMessage},
};
use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::{cmp::min, time::Duration};
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

pub async fn run(config: Config, device: DeviceIdentity, mut shutdown: watch::Receiver<bool>) {
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
                match handle_connection(stream, &config, &device, &mut shutdown).await {
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
    shutdown: &mut watch::Receiver<bool>,
) -> Result<ConnectionEnd>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut writer, mut reader) = stream.split();
    send(&mut writer, &AgentMessage::register(device)).await?;

    let registration = time::timeout(REGISTRATION_TIMEOUT, async {
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
    println!("Registered successfully\n\nStatus:\nONLINE\n\nPress Ctrl+C to stop.\n");

    let mut heartbeat = time::interval(config.heartbeat_interval);
    heartbeat.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    let mut awaiting_ack = false;

    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    writer.send(Message::Close(None)).await.ok();
                    writer.close().await.ok();
                    return Ok(ConnectionEnd::Shutdown);
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
                    IncomingMessage::Protocol(other) => debug!(?other, "Ignoring unexpected server message"),
                    IncomingMessage::ControlFrame => {}
                    IncomingMessage::Closed => return Ok(ConnectionEnd::Disconnected),
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
