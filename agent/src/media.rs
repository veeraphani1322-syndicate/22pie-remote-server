use crate::protocol::IceServer;
use anyhow::Result;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum MediaEvent {
    Offer(String),
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
    Connected,
    Ended(String),
}

#[derive(Debug)]
pub enum MediaCommand {
    Answer(String),
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_m_line_index: Option<u16>,
    },
    SetMouseControl(bool),
    SetKeyboardControl(bool),
    Stop {
        complete: tokio::sync::oneshot::Sender<()>,
    },
}

pub struct MediaSession {
    pub events: mpsc::Receiver<MediaEvent>,
    pub commands: mpsc::Sender<MediaCommand>,
}

impl MediaSession {
    pub async fn stop(self) {
        let (complete, done) = tokio::sync::oneshot::channel();
        if self
            .commands
            .send(MediaCommand::Stop { complete })
            .await
            .is_ok()
        {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), done).await;
        }
    }
}

#[cfg(not(windows))]
pub async fn start(_ice_servers: Vec<IceServer>, _session_id: String) -> Result<MediaSession> {
    anyhow::bail!("screen capture is supported only on Windows")
}

#[cfg(windows)]
pub async fn start(ice_servers: Vec<IceServer>, session_id: String) -> Result<MediaSession> {
    windows::start(ice_servers, session_id).await
}

#[cfg(windows)]
mod windows {
    use super::{MediaCommand, MediaEvent, MediaSession};
    use crate::{
        keyboard::{KeyboardController, KeyboardMessage},
        mouse::{MouseController, MouseMessage},
        protocol::{IceServer, UrlList},
    };
    use anyhow::{Context, Result};
    use bytes::Bytes;
    use fast_image_resize::{
        images::{Image, ImageRef},
        PixelType, ResizeAlg, ResizeOptions, Resizer,
    };
    use openh264::{
        encoder::{
            BitRate, Complexity, Encoder, EncoderConfig, FrameRate, IntraFramePeriod,
            RateControlMode, UsageType,
        },
        formats::{BgraSliceU8, YUVBuffer},
        OpenH264API,
    };
    use scrap::{Capturer, Display};
    use std::{
        io::ErrorKind::WouldBlock,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        time::{Duration, Instant},
    };
    use tokio::sync::mpsc;
    use tracing::{debug, info, warn};
    use webrtc::{
        api::{
            interceptor_registry::register_default_interceptors,
            media_engine::{MediaEngine, MIME_TYPE_H264},
            APIBuilder,
        },
        data_channel::data_channel_message::DataChannelMessage,
        ice_transport::{
            ice_candidate::{RTCIceCandidate, RTCIceCandidateInit},
            ice_server::RTCIceServer,
        },
        interceptor::registry::Registry,
        media::Sample,
        peer_connection::{
            configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
            sdp::session_description::RTCSessionDescription,
        },
        rtp_transceiver::rtp_codec::RTCRtpCodecCapability,
        track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
    };

    pub async fn start(ice_servers: Vec<IceServer>, session_id: String) -> Result<MediaSession> {
        let media_started_at = Instant::now();
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()?;
        let registry = register_default_interceptors(Registry::new(), &mut media_engine)?;
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();
        let configuration = RTCConfiguration {
            ice_servers: ice_servers.into_iter().map(to_rtc_ice_server).collect(),
            ..Default::default()
        };
        let peer = Arc::new(api.new_peer_connection(configuration).await?);
        let mouse_authorized = Arc::new(AtomicBool::new(false));
        let mouse = Arc::new(Mutex::new(MouseController::new()));
        let keyboard_authorized = Arc::new(AtomicBool::new(false));
        let keyboard = Arc::new(Mutex::new(KeyboardController::new()));
        let channel = peer.create_data_channel("control", None).await?;
        let opened_at = Instant::now();
        channel.on_open(Box::new(move || {
            info!(
                CONTROL_CHANNEL_OPEN_MS = opened_at.elapsed().as_millis(),
                "CONTROL_CHANNEL_OPEN"
            );
            Box::pin(async {})
        }));
        let message_authorized = mouse_authorized.clone();
        let message_controller = mouse.clone();
        let message_keyboard_authorized = keyboard_authorized.clone();
        let message_keyboard = keyboard.clone();
        channel.on_message(Box::new(move |data: DataChannelMessage| {
            let authorized = message_authorized.clone();
            let controller = message_controller.clone();
            let keyboard_authorized = message_keyboard_authorized.clone();
            let keyboard = message_keyboard.clone();
            let expected_session_id = session_id.clone();
            Box::pin(async move {
                if !data.is_string {
                    return;
                }
                let success = if authorized.load(Ordering::Acquire) {
                    MouseMessage::parse(&data.data)
                        .filter(|message| message.session_id() == expected_session_id)
                        .map(|message| {
                            controller
                                .lock()
                                .map(|mut mouse| mouse.execute(message))
                                .unwrap_or(false)
                        })
                } else {
                    None
                }
                .or_else(|| {
                    if !keyboard_authorized.load(Ordering::Acquire) {
                        return None;
                    }
                    KeyboardMessage::parse(&data.data)
                        .filter(|message| message.session_id() == expected_session_id)
                        .map(|message| {
                            keyboard
                                .lock()
                                .map(|mut keyboard| keyboard.execute(message))
                                .unwrap_or(false)
                        })
                });
                let Some(success) = success else {
                    return;
                };
                if !success {
                    warn!(
                        SENDINPUT_FAILURES = 1,
                        "Windows SendInput rejected control event"
                    );
                }
            })
        }));
        let close_controller = mouse.clone();
        let close_keyboard = keyboard.clone();
        channel.on_close(Box::new(move || {
            if let Ok(mut mouse) = close_controller.lock() {
                mouse.release_all();
            }
            if let Ok(mut keyboard) = close_keyboard.lock() {
                let released = keyboard.release_all();
                info!(
                    HELD_KEYS_RELEASED = released,
                    "Released keyboard state on control-channel close"
                );
            }
            Box::pin(async {})
        }));
        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            "screen".to_owned(),
            "22pie".to_owned(),
        ));
        let sender = peer
            .add_track(track.clone() as Arc<dyn TrackLocal + Send + Sync>)
            .await?;
        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1500];
            while sender.read(&mut buffer).await.is_ok() {}
        });

        let (event_tx, event_rx) = mpsc::channel(64);
        let (command_tx, mut command_rx) = mpsc::channel(64);
        let ice_tx = event_tx.clone();
        peer.on_ice_candidate(Box::new(move |candidate: Option<RTCIceCandidate>| {
            let tx = ice_tx.clone();
            Box::pin(async move {
                if let Some(candidate) = candidate {
                    if let Ok(json) = candidate.to_json() {
                        let _ = tx
                            .send(MediaEvent::IceCandidate {
                                candidate: json.candidate,
                                sdp_mid: json.sdp_mid,
                                sdp_m_line_index: json.sdp_mline_index,
                            })
                            .await;
                    }
                }
            })
        }));
        let state_tx = event_tx.clone();
        let capture_connected = Arc::new(AtomicBool::new(false));
        let state_connected = capture_connected.clone();
        peer.on_peer_connection_state_change(Box::new(move |state| {
            let tx = state_tx.clone();
            let connected = state_connected.clone();
            Box::pin(async move {
                if state == RTCPeerConnectionState::Connected {
                    connected.store(true, Ordering::Release);
                    info!(
                        WEBRTC_NEGOTIATION_MS = media_started_at.elapsed().as_millis(),
                        "WebRTC peer connected"
                    );
                    let _ = tx.send(MediaEvent::Connected).await;
                }
                if matches!(
                    state,
                    RTCPeerConnectionState::Failed
                        | RTCPeerConnectionState::Closed
                        | RTCPeerConnectionState::Disconnected
                ) {
                    let _ = tx.send(MediaEvent::Ended(format!("webrtc_{state}"))).await;
                }
            })
        }));

        let offer = peer.create_offer(None).await?;
        peer.set_local_description(offer).await?;
        let local = peer
            .local_description()
            .await
            .context("missing local WebRTC description")?;
        debug!(
            OFFER_CREATED_MS = media_started_at.elapsed().as_millis(),
            "WebRTC offer created"
        );
        event_tx.send(MediaEvent::Offer(local.sdp)).await.ok();

        let stopped = Arc::new(AtomicBool::new(false));
        start_capture(track, stopped.clone(), capture_connected, event_tx.clone());
        let command_peer = peer.clone();
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                let result = match command {
                    MediaCommand::Answer(sdp) => match RTCSessionDescription::answer(sdp) {
                        Ok(answer) => {
                            debug!(
                                ANSWER_RECEIVED_MS = media_started_at.elapsed().as_millis(),
                                "WebRTC answer received"
                            );
                            command_peer.set_remote_description(answer).await
                        }
                        Err(error) => Err(error),
                    },
                    MediaCommand::IceCandidate {
                        candidate,
                        sdp_mid,
                        sdp_m_line_index,
                    } => {
                        command_peer
                            .add_ice_candidate(RTCIceCandidateInit {
                                candidate,
                                sdp_mid,
                                sdp_mline_index: sdp_m_line_index,
                                username_fragment: None,
                            })
                            .await
                    }
                    MediaCommand::SetMouseControl(enabled) => {
                        mouse_authorized.store(enabled, Ordering::Release);
                        if !enabled {
                            if let Ok(mut controller) = mouse.lock() {
                                controller.release_all();
                            }
                        }
                        Ok(())
                    }
                    MediaCommand::SetKeyboardControl(enabled) => {
                        keyboard_authorized.store(enabled, Ordering::Release);
                        if !enabled {
                            if let Ok(mut controller) = keyboard.lock() {
                                let released = controller.release_all();
                                info!(HELD_KEYS_RELEASED = released, "Released keyboard state");
                            }
                        }
                        Ok(())
                    }
                    MediaCommand::Stop { complete } => {
                        stopped.store(true, Ordering::SeqCst);
                        mouse_authorized.store(false, Ordering::Release);
                        keyboard_authorized.store(false, Ordering::Release);
                        if let Ok(mut controller) = mouse.lock() {
                            controller.release_all();
                        }
                        if let Ok(mut controller) = keyboard.lock() {
                            let released = controller.release_all();
                            info!(
                                HELD_KEYS_RELEASED = released,
                                "Released keyboard state on session stop"
                            );
                        }
                        let _ = command_peer.close().await;
                        info!("MEDIA_STOP_COMPLETE");
                        let _ = complete.send(());
                        break;
                    }
                };
                if let Err(error) = result {
                    let _ = event_tx.send(MediaEvent::Ended(error.to_string())).await;
                }
            }
        });
        Ok(MediaSession {
            events: event_rx,
            commands: command_tx,
        })
    }

    fn to_rtc_ice_server(server: IceServer) -> RTCIceServer {
        RTCIceServer {
            urls: match server.urls {
                UrlList::One(url) => vec![url],
                UrlList::Many(urls) => urls,
            },
            username: server.username.unwrap_or_default(),
            credential: server.credential.unwrap_or_default(),
            ..Default::default()
        }
    }

    fn start_capture(
        track: Arc<TrackLocalStaticSample>,
        stopped: Arc<AtomicBool>,
        connected: Arc<AtomicBool>,
        event_tx: mpsc::Sender<MediaEvent>,
    ) {
        tokio::task::spawn_blocking(move || {
            if let Err(error) = capture_loop(track, stopped, connected) {
                let _ = event_tx.blocking_send(MediaEvent::Ended(error.to_string()));
            }
        });
    }

    fn capture_loop(
        track: Arc<TrackLocalStaticSample>,
        stopped: Arc<AtomicBool>,
        connected: Arc<AtomicBool>,
    ) -> Result<()> {
        while !stopped.load(Ordering::Acquire) && !connected.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if stopped.load(Ordering::Acquire) {
            return Ok(());
        }

        let connected_at = Instant::now();
        let display = Display::primary().context("failed to find primary monitor")?;
        let mut capturer =
            Capturer::new(display).context("failed to start primary monitor capture")?;
        let source_width = capturer.width();
        let source_height = capturer.height();
        let scale = (1280.0 / source_width as f64)
            .min(720.0 / source_height as f64)
            .min(1.0);
        let width = ((source_width as f64 * scale) as usize) & !1;
        let height = ((source_height as f64 * scale) as usize) & !1;
        let encoder_config = EncoderConfig::new()
            .usage_type(UsageType::ScreenContentRealTime)
            .complexity(Complexity::Low)
            .rate_control_mode(RateControlMode::Bitrate)
            .bitrate(BitRate::from_bps(2_000_000))
            .max_frame_rate(FrameRate::from_hz(15.0))
            .intra_frame_period(IntraFramePeriod::from_num_frames(30))
            .skip_frames(true);
        let mut encoder = Encoder::with_api_config(OpenH264API::from_source(), encoder_config)
            .context("failed to initialize H.264 encoder")?;
        let mut packed_bgra = vec![0u8; source_width * source_height * 4];
        let mut scaled_bgra = Image::new(width as u32, height as u32, PixelType::U8x4);
        let mut yuv = YUVBuffer::new(width, height);
        let mut resizer = Resizer::new();
        let resize_options = ResizeOptions::new()
            .resize_alg(ResizeAlg::Nearest)
            .use_alpha(false);
        let runtime = tokio::runtime::Handle::current();
        let frame_interval = Duration::from_nanos(1_000_000_000 / 15);
        let mut next_frame_at = Instant::now();
        let mut frame_count = 0u64;
        while !stopped.load(Ordering::SeqCst) {
            let capture_started = Instant::now();
            match capturer.frame() {
                Ok(frame) => {
                    let stride = frame.len() / source_height;
                    for (source, target) in frame
                        .chunks(stride)
                        .zip(packed_bgra.chunks_mut(source_width * 4))
                    {
                        target.copy_from_slice(&source[..source_width * 4]);
                    }
                    let capture_ms = capture_started.elapsed().as_micros() as f64 / 1000.0;

                    let resize_started = Instant::now();
                    if source_width == width && source_height == height {
                        scaled_bgra.buffer_mut().copy_from_slice(&packed_bgra);
                    } else {
                        let source = ImageRef::new(
                            source_width as u32,
                            source_height as u32,
                            &packed_bgra,
                            PixelType::U8x4,
                        )?;
                        resizer.resize(&source, &mut scaled_bgra, Some(&resize_options))?;
                    }
                    let resize_ms = resize_started.elapsed().as_micros() as f64 / 1000.0;

                    let convert_started = Instant::now();
                    yuv.read_bgra8(BgraSliceU8::new(scaled_bgra.buffer(), (width, height)));
                    let color_convert_ms = convert_started.elapsed().as_micros() as f64 / 1000.0;

                    let encode_started = Instant::now();
                    let encoded = encoder.encode(&yuv)?.to_vec();
                    let encode_ms = encode_started.elapsed().as_micros() as f64 / 1000.0;

                    let write_started = Instant::now();
                    runtime.block_on(track.write_sample(&Sample {
                        data: Bytes::from(encoded),
                        duration: frame_interval,
                        ..Default::default()
                    }))?;
                    let write_sample_ms = write_started.elapsed().as_micros() as f64 / 1000.0;
                    frame_count += 1;
                    if frame_count == 1 {
                        info!(
                            TIME_TO_FIRST_FRAME_MS = connected_at.elapsed().as_millis(),
                            width, height, "First encoded frame written after WebRTC connection"
                        );
                    }
                    if frame_count == 1 || frame_count % 150 == 0 {
                        debug!(
                            CAPTURE_MS = capture_ms,
                            RESIZE_MS = resize_ms,
                            COLOR_CONVERT_MS = color_convert_ms,
                            ENCODE_MS = encode_ms,
                            WRITE_SAMPLE_MS = write_sample_ms,
                            frame_count,
                            "Screen frame performance"
                        );
                    }

                    next_frame_at += frame_interval;
                    let now = Instant::now();
                    if next_frame_at > now {
                        std::thread::sleep(next_frame_at - now);
                    } else {
                        next_frame_at = now;
                    }
                }
                Err(error) if error.kind() == WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}
