use crate::protocol::IceServer;
use anyhow::{bail, Result};
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
    Stop,
}

pub struct MediaSession {
    pub events: mpsc::Receiver<MediaEvent>,
    pub commands: mpsc::Sender<MediaCommand>,
}

#[cfg(not(windows))]
pub async fn start(_ice_servers: Vec<IceServer>) -> Result<MediaSession> {
    bail!("screen capture is supported only on Windows")
}

#[cfg(windows)]
pub async fn start(ice_servers: Vec<IceServer>) -> Result<MediaSession> {
    windows::start(ice_servers).await
}

#[cfg(windows)]
mod windows {
    use super::{MediaCommand, MediaEvent, MediaSession};
    use crate::protocol::{IceServer, UrlList};
    use anyhow::{Context, Result};
    use bytes::Bytes;
    use openh264::{
        encoder::Encoder,
        formats::{BgraSliceU8, YUVBuffer},
    };
    use scrap::{Capturer, Display};
    use std::{
        io::ErrorKind::WouldBlock,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Duration,
    };
    use tokio::sync::mpsc;
    use webrtc::{
        api::{
            interceptor_registry::register_default_interceptors, media_engine::MediaEngine,
            APIBuilder,
        },
        ice_transport::{
            ice_candidate::RTCIceCandidate, ice_candidate_init::RTCIceCandidateInit,
            ice_server::RTCIceServer,
        },
        interceptor::registry::Registry,
        media::Sample,
        peer_connection::{
            configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
            sdp::session_description::RTCSessionDescription,
        },
        rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, MIME_TYPE_H264},
        track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal},
    };

    pub async fn start(ice_servers: Vec<IceServer>) -> Result<MediaSession> {
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
        peer.on_peer_connection_state_change(Box::new(move |state| {
            let tx = state_tx.clone();
            Box::pin(async move {
                if state == RTCPeerConnectionState::Connected {
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
        event_tx.send(MediaEvent::Offer(local.sdp)).await.ok();

        let stopped = Arc::new(AtomicBool::new(false));
        start_capture(track, stopped.clone(), event_tx.clone());
        let command_peer = peer.clone();
        tokio::spawn(async move {
            while let Some(command) = command_rx.recv().await {
                let result = match command {
                    MediaCommand::Answer(sdp) => match RTCSessionDescription::answer(sdp) {
                        Ok(answer) => command_peer.set_remote_description(answer).await,
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
                    MediaCommand::Stop => {
                        stopped.store(true, Ordering::SeqCst);
                        let _ = command_peer.close().await;
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
        event_tx: mpsc::Sender<MediaEvent>,
    ) {
        tokio::task::spawn_blocking(move || {
            if let Err(error) = capture_loop(track, stopped) {
                let _ = event_tx.blocking_send(MediaEvent::Ended(error.to_string()));
            }
        });
    }

    fn capture_loop(track: Arc<TrackLocalStaticSample>, stopped: Arc<AtomicBool>) -> Result<()> {
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
        let mut encoder = Encoder::new().context("failed to initialize H.264 encoder")?;
        let runtime = tokio::runtime::Handle::current();
        while !stopped.load(Ordering::SeqCst) {
            match capturer.frame() {
                Ok(frame) => {
                    let stride = frame.len() / source_height;
                    let mut bgra = vec![0u8; width * height * 4];
                    for y in 0..height {
                        for x in 0..width {
                            let sx = x * source_width / width;
                            let sy = y * source_height / height;
                            let source = sy * stride + sx * 4;
                            let target = (y * width + x) * 4;
                            bgra[target..target + 4].copy_from_slice(&frame[source..source + 4]);
                        }
                    }
                    let yuv = YUVBuffer::from_rgb_source(BgraSliceU8::new(&bgra, (width, height)));
                    let encoded = encoder.encode(&yuv)?.to_vec();
                    runtime.block_on(track.write_sample(&Sample {
                        data: Bytes::from(encoded),
                        duration: Duration::from_millis(67),
                        ..Default::default()
                    }))?;
                    std::thread::sleep(Duration::from_millis(67));
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
