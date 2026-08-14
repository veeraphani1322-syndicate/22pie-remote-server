import type WebSocket from "ws";
import type { RegisterMessage } from "../types/protocol.js";

export type DeviceStatus = "online" | "offline";

export interface DeviceView {
  deviceId: string;
  deviceName: string;
  operatingSystem: string;
  agentVersion: string;
  connectedAt: string;
  lastSeenAt: string;
  status: DeviceStatus;
}

interface DeviceRecord extends DeviceView {
  connectionId?: string;
  socket?: WebSocket;
}

export interface RegistrationResult {
  device: DeviceView;
  replacedSocket?: WebSocket;
}

export class DeviceManager {
  private readonly devices = new Map<string, DeviceRecord>();

  register(message: RegisterMessage, socket: WebSocket, connectionId: string): RegistrationResult {
    const now = new Date().toISOString();
    const existing = this.devices.get(message.deviceId);
    const device: DeviceRecord = {
      deviceId: message.deviceId,
      deviceName: message.deviceName,
      operatingSystem: message.operatingSystem,
      agentVersion: message.agentVersion,
      connectedAt: now,
      lastSeenAt: now,
      status: "online",
      connectionId,
      socket,
    };

    this.devices.set(message.deviceId, device);
    return {
      device: this.toView(device),
      replacedSocket: existing?.status === "online" ? existing.socket : undefined,
    };
  }

  touch(deviceId: string, connectionId: string): DeviceView | undefined {
    const device = this.devices.get(deviceId);
    if (!device || device.connectionId !== connectionId || device.status !== "online") return undefined;
    device.lastSeenAt = new Date().toISOString();
    return this.toView(device);
  }

  markOffline(deviceId: string, connectionId: string): DeviceView | undefined {
    const device = this.devices.get(deviceId);
    if (!device || device.connectionId !== connectionId) return undefined;
    device.status = "offline";
    device.lastSeenAt = new Date().toISOString();
    delete device.socket;
    delete device.connectionId;
    return this.toView(device);
  }

  list(): DeviceView[] {
    return [...this.devices.values()].map((device) => this.toView(device));
  }

  get(deviceId: string): DeviceView | undefined {
    const device = this.devices.get(deviceId);
    return device ? this.toView(device) : undefined;
  }

  staleConnections(timeoutMs: number): Array<{ deviceId: string; connectionId: string; socket: WebSocket }> {
    const cutoff = Date.now() - timeoutMs;
    return [...this.devices.values()].flatMap((device) => {
      if (
        device.status === "online" &&
        device.connectionId &&
        device.socket &&
        Date.parse(device.lastSeenAt) < cutoff
      ) {
        return [{ deviceId: device.deviceId, connectionId: device.connectionId, socket: device.socket }];
      }
      return [];
    });
  }

  closeAll(): void {
    for (const device of this.devices.values()) device.socket?.close(1001, "Server shutting down");
  }

  private toView(device: DeviceRecord): DeviceView {
    const { deviceId, deviceName, operatingSystem, agentVersion, connectedAt, lastSeenAt, status } = device;
    return { deviceId, deviceName, operatingSystem, agentVersion, connectedAt, lastSeenAt, status };
  }
}
