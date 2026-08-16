use crate::device::DeviceIdentity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentMessage<'a> {
    Register {
        #[serde(rename = "deviceId")]
        device_id: String,
        #[serde(rename = "deviceName")]
        device_name: &'a str,
        #[serde(rename = "operatingSystem")]
        operating_system: &'a str,
        #[serde(rename = "agentVersion")]
        agent_version: &'a str,
        #[serde(rename = "publicKey")]
        public_key: String,
    },
    Authenticate {
        signature: String,
    },
    Heartbeat,
    SessionAccept {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
    },
    TrustGrant {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        permission: Permission,
    },
    TrustRevoke {
        #[serde(rename = "userId")]
        user_id: &'a str,
        permission: Permission,
    },
    SessionReject {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        reason: &'a str,
    },
    WebrtcOffer {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        sdp: &'a str,
    },
    WebrtcAnswer {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        sdp: &'a str,
    },
    IceCandidate {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        candidate: &'a str,
        #[serde(rename = "sdpMid")]
        sdp_mid: Option<&'a str>,
        #[serde(rename = "sdpMLineIndex")]
        sdp_m_line_index: Option<u16>,
    },
    SessionEnd {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        reason: &'a str,
    },
    MouseControlAccept {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
    },
    MouseControlReject {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        reason: &'a str,
    },
    KeyboardControlAccept {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
    },
    KeyboardControlReject {
        #[serde(rename = "sessionId")]
        session_id: &'a str,
        reason: &'a str,
    },
}

impl<'a> AgentMessage<'a> {
    pub fn register(device: &'a DeviceIdentity) -> Self {
        Self::Register {
            device_id: device.device_id.to_string(),
            device_name: &device.device_name,
            operating_system: &device.operating_system,
            agent_version: &device.agent_version,
            public_key: device.public_key(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    AuthChallenge {
        nonce: String,
    },
    Registered {
        #[serde(rename = "deviceId")]
        device_id: String,
        status: DeviceStatus,
    },
    HeartbeatAck {
        timestamp: String,
    },
    SessionRequested {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "viewerUserId")]
        viewer_user_id: String,
        #[serde(rename = "viewerName")]
        viewer_name: String,
        permissions: Vec<Permission>,
        trusted: bool,
        #[serde(rename = "iceServers")]
        ice_servers: Vec<IceServer>,
    },
    WebrtcAnswer {
        #[serde(rename = "sessionId")]
        session_id: String,
        sdp: String,
    },
    IceCandidate {
        #[serde(rename = "sessionId")]
        session_id: String,
        candidate: String,
        #[serde(rename = "sdpMid")]
        sdp_mid: Option<String>,
        #[serde(rename = "sdpMLineIndex")]
        sdp_m_line_index: Option<u16>,
    },
    SessionEnded {
        #[serde(rename = "sessionId")]
        session_id: String,
        reason: String,
    },
    MouseControlRequested {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "viewerUserId")]
        viewer_user_id: String,
        #[serde(rename = "viewerName")]
        viewer_name: String,
        trusted: bool,
    },
    MouseControlDisabled {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    MouseControlEnabled {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    KeyboardControlRequested {
        #[serde(rename = "sessionId")]
        session_id: String,
        #[serde(rename = "viewerUserId")]
        viewer_user_id: String,
        #[serde(rename = "viewerName")]
        viewer_name: String,
        trusted: bool,
    },
    KeyboardControlDisabled {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    KeyboardControlEnabled {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    TrustRevoked {
        #[serde(rename = "userId")]
        user_id: String,
        permission: Permission,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Permission {
    ScreenView,
    MouseControl,
    KeyboardControl,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IceServer {
    pub urls: UrlList,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum UrlList {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Online,
    Offline,
}
