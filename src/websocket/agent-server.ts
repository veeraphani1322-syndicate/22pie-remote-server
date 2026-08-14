import { createPublicKey, randomBytes, randomUUID, verify } from "node:crypto";
import type { Server as HttpServer } from "node:http";
import type { FastifyBaseLogger } from "fastify";
import WebSocket, { WebSocketServer } from "ws";
import type { AppConfig } from "../config/env.js";
import type { DeviceManager } from "../devices/device-manager.js";
import type { SessionManager } from "../sessions/session-manager.js";
import { agentMessageSchema, type ServerMessage } from "../types/protocol.js";

function send(socket: WebSocket, message: ServerMessage): void {
  if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
}

export class AgentWebSocketServer {
  private readonly wss: WebSocketServer;
  private heartbeatTimer?: NodeJS.Timeout;

  constructor(
    httpServer: HttpServer,
    private readonly devices: DeviceManager,
    private readonly config: AppConfig,
    private readonly logger: FastifyBaseLogger,
    private readonly sessions: SessionManager,
  ) {
    this.wss = new WebSocketServer({ noServer: true, maxPayload: config.WS_MAX_PAYLOAD_BYTES });

    httpServer.on("upgrade", (request, socket, head) => {
      let pathname: string;
      try {
        pathname = new URL(request.url ?? "/", "http://localhost").pathname;
      } catch {
        socket.destroy();
        return;
      }
      if (pathname !== "/agent") {
        return;
      }
      this.wss.handleUpgrade(request, socket, head, (ws) => this.wss.emit("connection", ws, request));
    });

    this.wss.on("connection", (socket) => this.handleConnection(socket));
    this.heartbeatTimer = setInterval(() => this.expireStaleConnections(), config.HEARTBEAT_CHECK_INTERVAL_MS);
    this.heartbeatTimer.unref();
  }

  private handleConnection(socket: WebSocket): void {
    const connectionId = randomUUID();
    let deviceId: string | undefined;
    let pendingRegistration: import("../types/protocol.js").RegisterMessage | undefined;
    let challenge: string | undefined;
    const registrationTimer = setTimeout(() => {
      if (!deviceId) socket.close(1008, "Registration required");
    }, this.config.REGISTRATION_TIMEOUT_MS);

    socket.on("message", (data, isBinary) => {
      if (isBinary) {
        send(socket, { type: "error", code: "UNSUPPORTED_DATA", message: "Binary messages are not supported" });
        return;
      }

      let json: unknown;
      try {
        json = JSON.parse(data.toString());
      } catch {
        send(socket, { type: "error", code: "INVALID_JSON", message: "Message must be valid JSON" });
        return;
      }

      const parsed = agentMessageSchema.safeParse(json);
      if (!parsed.success) {
        send(socket, { type: "error", code: "INVALID_MESSAGE", message: "Message failed validation" });
        return;
      }

      if (parsed.data.type === "register") {
        if (deviceId || pendingRegistration) {
          send(socket, { type: "error", code: "ALREADY_REGISTERED", message: "Connection is already registered" });
          return;
        }
        const savedKey = this.devices.publicKeyFor(parsed.data.deviceId);
        if (savedKey && savedKey !== parsed.data.publicKey) {
          send(socket, { type: "error", code: "DEVICE_KEY_MISMATCH", message: "Device credential does not match" });
          socket.close(1008, "Device authentication failed");
          return;
        }
        pendingRegistration = parsed.data;
        challenge = randomBytes(32).toString("base64url");
        send(socket, { type: "auth_challenge", nonce: challenge });
        return;
      }

      if (parsed.data.type === "authenticate") {
        if (!pendingRegistration || !challenge || deviceId) {
          send(socket, { type: "error", code: "AUTH_NOT_PENDING", message: "Register before authenticating" });
          return;
        }
        const rawKey = Buffer.from(pendingRegistration.publicKey, "base64");
        const key = createPublicKey({
          key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), rawKey]),
          format: "der",
          type: "spki",
        });
        const signed = Buffer.from(`${pendingRegistration.deviceId}:${challenge}`, "utf8");
        if (!verify(null, signed, key, Buffer.from(parsed.data.signature, "base64"))) {
          send(socket, { type: "error", code: "DEVICE_AUTH_FAILED", message: "Invalid device signature" });
          socket.close(1008, "Device authentication failed");
          return;
        }
        deviceId = pendingRegistration.deviceId;
        clearTimeout(registrationTimer);
        const result = this.devices.register(pendingRegistration, socket, connectionId, "admin");
        if (result.replacedSocket && result.replacedSocket !== socket) {
          this.logger.warn({ deviceId }, "Replacing duplicate device connection");
          result.replacedSocket.close(4001, "Replaced by newer connection");
        }
        this.logger.info({
          event: "DEVICE_CONNECTED",
          deviceId,
          deviceName: pendingRegistration.deviceName,
          operatingSystem: pendingRegistration.operatingSystem,
        }, "Device connected");
        send(socket, { type: "registered", deviceId, status: "online" });
        return;
      }

      if (!deviceId) {
        send(socket, { type: "error", code: "NOT_REGISTERED", message: "Register before sending heartbeats" });
        return;
      }
      if (parsed.data.type !== "heartbeat") {
        this.sessions.fromAgent(deviceId, parsed.data);
        return;
      }
      if (!this.devices.touch(deviceId, connectionId)) {
        send(socket, { type: "error", code: "STALE_CONNECTION", message: "Connection is no longer active" });
        socket.close(4001, "Stale connection");
        return;
      }
      send(socket, { type: "heartbeat_ack", timestamp: new Date().toISOString() });
    });

    socket.on("close", (code) => {
      clearTimeout(registrationTimer);
      if (!deviceId) return;
      const device = this.devices.markOffline(deviceId, connectionId);
      if (device) {
        this.sessions.endForDevice(deviceId, "agent_disconnected");
        this.logger.info({ event: "DEVICE_DISCONNECTED", deviceId, deviceName: device.deviceName, code }, "Device disconnected");
      }
    });

    socket.on("error", (error) => this.logger.warn({ err: error, deviceId }, "Agent WebSocket error"));
  }

  private expireStaleConnections(): void {
    for (const stale of this.devices.staleConnections(this.config.HEARTBEAT_TIMEOUT_MS)) {
      this.logger.warn({ deviceId: stale.deviceId }, "Device heartbeat timed out");
      stale.socket.terminate();
    }
  }

  async close(): Promise<void> {
    if (this.heartbeatTimer) clearInterval(this.heartbeatTimer);
    this.devices.closeAll();
    await new Promise<void>((resolve) => this.wss.close(() => resolve()));
  }
}
