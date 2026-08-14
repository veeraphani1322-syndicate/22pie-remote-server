import type WebSocket from "ws";
import type { RegisterMessage } from "../types/protocol.js";
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

export type DeviceStatus = "online" | "offline";

export interface DeviceView {
  deviceId: string;
  deviceName: string;
  operatingSystem: string;
  agentVersion: string;
  connectedAt: string;
  lastSeenAt: string;
  status: DeviceStatus;
  ownerId: string;
}

interface DeviceRecord extends DeviceView {
  publicKey: string;
  connectionId?: string;
  socket?: WebSocket;
}

export interface RegistrationResult {
  device: DeviceView;
  replacedSocket?: WebSocket;
}

export class DeviceManager {
  private readonly devices = new Map<string, DeviceRecord>();

  constructor(private readonly storePath: string) {
    this.load();
  }

  register(message: RegisterMessage, socket: WebSocket, connectionId: string, ownerId: string): RegistrationResult {
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
      ownerId: existing?.ownerId ?? ownerId,
      publicKey: message.publicKey,
      connectionId,
      socket,
    };

    this.devices.set(message.deviceId, device);
    this.persist();
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

  list(ownerId?: string): DeviceView[] {
    return [...this.devices.values()]
      .filter((device) => !ownerId || device.ownerId === ownerId)
      .map((device) => this.toView(device));
  }

  get(deviceId: string): DeviceView | undefined {
    const device = this.devices.get(deviceId);
    return device ? this.toView(device) : undefined;
  }

  getOwned(deviceId: string, ownerId: string): DeviceView | undefined {
    const device = this.devices.get(deviceId);
    return device?.ownerId === ownerId ? this.toView(device) : undefined;
  }

  socketFor(deviceId: string): WebSocket | undefined {
    const device = this.devices.get(deviceId);
    return device?.status === "online" ? device.socket : undefined;
  }

  publicKeyFor(deviceId: string): string | undefined {
    return this.devices.get(deviceId)?.publicKey;
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

  private load(): void {
    try {
      const records = JSON.parse(readFileSync(this.storePath, "utf8")) as DeviceRecord[];
      for (const record of records) this.devices.set(record.deviceId, { ...record, status: "offline" });
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
  }

  private persist(): void {
    mkdirSync(dirname(this.storePath), { recursive: true });
    const tempPath = `${this.storePath}.tmp`;
    const records = [...this.devices.values()].map(({ socket: _socket, connectionId: _connectionId, ...record }) => record);
    writeFileSync(tempPath, `${JSON.stringify(records, null, 2)}\n`, { mode: 0o600 });
    renameSync(tempPath, this.storePath);
  }

  private toView(device: DeviceRecord): DeviceView {
    const { deviceId, deviceName, operatingSystem, agentVersion, connectedAt, lastSeenAt, status, ownerId } = device;
    return { deviceId, deviceName, operatingSystem, agentVersion, connectedAt, lastSeenAt, status, ownerId };
  }
}
