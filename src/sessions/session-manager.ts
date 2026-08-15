import { randomUUID } from "node:crypto";
import WebSocket from "ws";
import type { DeviceManager } from "../devices/device-manager.js";
import type { ServerMessage, ViewerMessage, AgentMessage } from "../types/protocol.js";
import type { TrustedAccessStore } from "../trust/trusted-access-store.js";

export type SessionStatus = "requested" | "accepted" | "connecting" | "connected" | "rejected" | "ended" | "failed" | "expired";

export interface SessionView {
  sessionId: string;
  userId: string;
  deviceId: string;
  status: SessionStatus;
  permissions: ["SCREEN_VIEW"];
  requestedAt: string;
  acceptedAt?: string;
  connectedAt?: string;
  endedAt?: string;
  trusted: boolean;
}

interface SessionRecord extends SessionView {
  viewer?: WebSocket;
  timeout: NodeJS.Timeout;
}

function send(socket: WebSocket | undefined, message: ServerMessage): void {
  if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify(message));
}

export class SessionManager {
  private readonly sessions = new Map<string, SessionRecord>();

  constructor(
    private readonly devices: DeviceManager,
    private readonly approvalTimeoutMs: number,
    private readonly negotiationTimeoutMs: number,
    private readonly iceServers: Array<{ urls: string | string[]; username?: string; credential?: string }>,
    private readonly trustedAccess: TrustedAccessStore,
    private readonly audit: (data: Record<string, unknown>, message: string) => void = () => undefined,
  ) {}

  request(userId: string, deviceId: string, viewerName: string): SessionView {
    const agent = this.devices.socketFor(deviceId);
    if (!agent) throw new Error("DEVICE_OFFLINE");
    const sessionId = randomUUID();
    const trusted = this.trustedAccess.has(deviceId, userId, "SCREEN_VIEW", this.devices.publicKeyFor(deviceId));
    const record: SessionRecord = {
      sessionId, userId, deviceId, status: "requested", permissions: ["SCREEN_VIEW"],
      requestedAt: new Date().toISOString(), trusted,
      timeout: setTimeout(() => this.end(sessionId, "approval_timeout", "expired"), this.approvalTimeoutMs),
    };
    record.timeout.unref();
    this.sessions.set(sessionId, record);
    this.audit({ event: trusted ? "TRUST_USED" : "TRUST_REQUESTED", deviceId, userId, permission: "SCREEN_VIEW" }, trusted ? "Trusted access used" : "Trusted access requested");
    send(agent, { type: "session_requested", sessionId, viewerUserId: userId, viewerName, permissions: ["SCREEN_VIEW"], trusted, iceServers: this.iceServers });
    return this.view(record);
  }

  getOwned(sessionId: string, userId: string): SessionView | undefined {
    const session = this.sessions.get(sessionId);
    return session?.userId === userId ? this.view(session) : undefined;
  }

  attachViewer(sessionId: string, userId: string, socket: WebSocket): SessionView | undefined {
    const session = this.sessions.get(sessionId);
    if (!session || session.userId !== userId) return undefined;
    session.viewer?.close(4001, "Replaced by newer viewer connection");
    session.viewer = socket;
    return this.view(session);
  }

  viewerDisconnected(sessionId: string, userId: string, socket: WebSocket): void {
    const session = this.sessions.get(sessionId);
    if (
      !session ||
      session.userId !== userId ||
      session.viewer !== socket ||
      ["ended", "failed", "expired", "rejected"].includes(session.status)
    ) return;
    this.end(sessionId, "viewer_socket_closed", "ended");
  }

  fromAgent(deviceId: string, message: AgentMessage): void {
    if (message.type === "trust_revoke") {
      const revoked = this.trustedAccess.revoke(deviceId, message.userId, message.permission);
      if (revoked) this.audit({ event: "TRUST_REVOKED", deviceId, userId: message.userId, permission: message.permission }, "Trusted access revoked by device");
      return;
    }
    if (!("sessionId" in message)) return;
    const session = this.sessions.get(message.sessionId);
    if (!session || session.deviceId !== deviceId) return;
    if (message.type === "trust_grant" && session.status === "requested" && message.permission === "SCREEN_VIEW") {
      const devicePublicKey = this.devices.publicKeyFor(deviceId);
      if (!devicePublicKey) return;
      this.trustedAccess.grant(deviceId, session.userId, message.permission, devicePublicKey);
      session.trusted = true;
      this.audit({ event: "TRUST_GRANTED", deviceId, userId: session.userId, permission: message.permission }, "Trusted access granted by device");
      return;
    }
    if (message.type === "session_accept" && session.status === "requested") {
      clearTimeout(session.timeout);
      session.status = "accepted";
      session.acceptedAt = new Date().toISOString();
      session.timeout = setTimeout(() => this.end(session.sessionId, "negotiation_timeout", "expired"), this.negotiationTimeoutMs);
      session.timeout.unref();
      send(session.viewer, { type: "session_accepted", sessionId: session.sessionId });
      return;
    }
    if (message.type === "session_reject") {
      this.audit({ event: "TRUST_DENIED", deviceId, userId: session.userId, permission: "SCREEN_VIEW" }, "Screen access denied by device");
      this.end(session.sessionId, message.reason ?? "denied", "rejected");
      return;
    }
    if (message.type === "session_end") {
      this.end(session.sessionId, message.reason ?? "agent_ended", "ended");
      return;
    }
    if (["webrtc_offer", "webrtc_answer", "ice_candidate"].includes(message.type) && ["accepted", "connecting", "connected"].includes(session.status)) {
      session.status = "connecting";
      send(session.viewer, message as ServerMessage);
    }
  }

  fromViewer(sessionId: string, userId: string, message: ViewerMessage): void {
    const session = this.sessions.get(sessionId);
    if (!session || session.userId !== userId || message.sessionId !== sessionId) return;
    if (message.type === "session_connected") {
      this.markConnected(sessionId, userId);
      return;
    }
    if (message.type === "session_end") {
      this.end(sessionId, message.reason ?? "viewer_disconnected", "ended");
      return;
    }
    if (!["accepted", "connecting", "connected"].includes(session.status)) return;
    session.status = "connecting";
    const agent = this.devices.socketFor(session.deviceId);
    if (!agent) return this.end(sessionId, "agent_offline", "failed");
    send(agent, message as ServerMessage);
  }

  markConnected(sessionId: string, userId: string): void {
    const session = this.sessions.get(sessionId);
    if (!session || session.userId !== userId) return;
    clearTimeout(session.timeout);
    session.status = "connected";
    session.connectedAt = new Date().toISOString();
  }

  endForDevice(deviceId: string, reason: string): void {
    for (const session of this.sessions.values()) {
      if (session.deviceId === deviceId && !["ended", "failed", "expired", "rejected"].includes(session.status)) {
        this.end(session.sessionId, reason, "failed");
      }
    }
  }

  end(sessionId: string, reason: string, status: SessionStatus = "ended"): void {
    const session = this.sessions.get(sessionId);
    if (!session || ["ended", "failed", "expired", "rejected"].includes(session.status)) return;
    clearTimeout(session.timeout);
    session.status = status;
    session.endedAt = new Date().toISOString();
    send(session.viewer, status === "rejected"
      ? { type: "session_rejected", sessionId, reason }
      : { type: "session_ended", sessionId, reason });
    send(this.devices.socketFor(session.deviceId), { type: "session_ended", sessionId, reason });
    session.viewer?.close(1000, reason);
    delete session.viewer;
  }

  close(): void {
    for (const session of this.sessions.values()) this.end(session.sessionId, "server_shutdown");
  }

  private view(session: SessionRecord): SessionView {
    const { viewer: _viewer, timeout: _timeout, ...view } = session;
    return view;
  }
}
