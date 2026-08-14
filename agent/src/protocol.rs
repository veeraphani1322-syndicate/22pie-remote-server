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
    },
    Heartbeat,
}

impl<'a> AgentMessage<'a> {
    pub fn register(device: &'a DeviceIdentity) -> Self {
        Self::Register {
            device_id: device.device_id.to_string(),
            device_name: &device.device_name,
            operating_system: &device.operating_system,
            agent_version: &device.agent_version,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Registered {
        #[serde(rename = "deviceId")]
        device_id: String,
        status: DeviceStatus,
    },
    HeartbeatAck {
        timestamp: String,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Online,
    Offline,
}
