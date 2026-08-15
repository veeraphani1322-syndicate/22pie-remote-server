import assert from "node:assert/strict";
import test from "node:test";
import WebSocket from "ws";
import type { DeviceManager } from "../devices/device-manager.js";
import { SessionManager } from "./session-manager.js";
import { TrustedAccessStore } from "../trust/trusted-access-store.js";

interface FakeSocket {
  readyState: number;
  messages: unknown[];
  closeCalls: Array<{ code?: number; reason?: string }>;
  send(data: string): void;
  close(code?: number, reason?: string): void;
}

function fakeSocket(): FakeSocket & WebSocket {
  const messages: unknown[] = [];
  const closeCalls: Array<{ code?: number; reason?: string }> = [];
  return {
    readyState: WebSocket.OPEN,
    messages,
    closeCalls,
    send(data: string) { messages.push(JSON.parse(data)); },
    close(code?: number, reason?: string) {
      closeCalls.push({ code, reason });
    },
  } as unknown as FakeSocket & WebSocket;
}

function fixture() {
  const agent = fakeSocket();
  const devices = { socketFor: () => agent, publicKeyFor: () => "test-device-key" } as unknown as DeviceManager;
  const trust = new TrustedAccessStore(`/private/tmp/22pie-session-trust-${process.pid}-${Math.random()}.json`);
  const sessions = new SessionManager(devices, 60_000, 60_000, [], trust);
  return { agent, sessions, trust };
}

test("closing the attached viewer ends the session and notifies the agent", () => {
  const { agent, sessions } = fixture();
  const viewer = fakeSocket();
  const session = sessions.request("user-a", "device-a", "Viewer");
  sessions.attachViewer(session.sessionId, "user-a", viewer);

  sessions.viewerDisconnected(session.sessionId, "user-a", viewer);

  assert.equal(sessions.getOwned(session.sessionId, "user-a")?.status, "ended");
  assert.deepEqual(agent.messages.at(-1), {
    type: "session_ended", sessionId: session.sessionId, reason: "viewer_socket_closed",
  });
});

test("only device-granted trust makes a later session trusted", () => {
  const { agent, sessions, trust } = fixture();
  const first = sessions.request("user-a", "device-a", "Viewer");
  assert.equal(first.trusted, false);
  sessions.fromAgent("device-a", { type: "trust_grant", sessionId: first.sessionId, permission: "SCREEN_VIEW" });
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: first.sessionId });
  sessions.end(first.sessionId, "test");

  const second = sessions.request("user-a", "device-a", "Viewer");
  assert.equal(second.trusted, true);
  assert.equal(trust.has("device-a", "user-a", "SCREEN_VIEW", "test-device-key"), true);
  assert.equal((agent.messages.at(-1) as { trusted?: boolean }).trusted, true);
  sessions.end(second.sessionId, "test");
});

test("allow once and deny do not persist trust", () => {
  const { sessions, trust } = fixture();
  const allowed = sessions.request("user-a", "device-a", "Viewer");
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: allowed.sessionId });
  sessions.end(allowed.sessionId, "test");
  const denied = sessions.request("user-a", "device-a", "Viewer");
  sessions.fromAgent("device-a", { type: "session_reject", sessionId: denied.sessionId, reason: "denied" });
  assert.equal(trust.has("device-a", "user-a", "SCREEN_VIEW", "test-device-key"), false);
});

test("an old replacement viewer cannot end the session, but the current viewer can", () => {
  const { sessions } = fixture();
  const viewerA = fakeSocket();
  const viewerB = fakeSocket();
  const session = sessions.request("user-a", "device-a", "Viewer");
  sessions.attachViewer(session.sessionId, "user-a", viewerA);
  sessions.attachViewer(session.sessionId, "user-a", viewerB);

  sessions.viewerDisconnected(session.sessionId, "user-a", viewerA);
  assert.equal(sessions.getOwned(session.sessionId, "user-a")?.status, "requested");

  sessions.viewerDisconnected(session.sessionId, "user-a", viewerB);
  assert.equal(sessions.getOwned(session.sessionId, "user-a")?.status, "ended");
});

test("a new session can be requested after the previous viewer disconnects", () => {
  const { sessions } = fixture();
  const viewer = fakeSocket();
  const first = sessions.request("user-a", "device-a", "Viewer");
  sessions.attachViewer(first.sessionId, "user-a", viewer);
  sessions.viewerDisconnected(first.sessionId, "user-a", viewer);

  const second = sessions.request("user-a", "device-a", "Viewer");

  assert.notEqual(second.sessionId, first.sessionId);
  assert.equal(second.status, "requested");
  sessions.end(second.sessionId, "test_cleanup");
});

test("mouse control is separately authorized and can be downgraded without ending video", () => {
  const { agent, sessions, trust } = fixture();
  const viewer = fakeSocket();
  const session = sessions.request("user-a", "device-a", "Viewer");
  sessions.attachViewer(session.sessionId, "user-a", viewer);
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: session.sessionId });
  sessions.markConnected(session.sessionId, "user-a");
  sessions.requestMouseControl(session.sessionId, "user-a", "Viewer");
  assert.deepEqual(agent.messages.at(-1), { type: "mouse_control_requested", sessionId: session.sessionId, viewerUserId: "user-a", viewerName: "Viewer", trusted: false });
  sessions.fromAgent("device-a", { type: "trust_grant", sessionId: session.sessionId, permission: "MOUSE_CONTROL" });
  sessions.fromAgent("device-a", { type: "mouse_control_accept", sessionId: session.sessionId });
  assert.deepEqual(sessions.getOwned(session.sessionId, "user-a")?.permissions, ["SCREEN_VIEW", "MOUSE_CONTROL"]);
  assert.equal(trust.has("device-a", "user-a", "MOUSE_CONTROL", "test-device-key"), true);
  sessions.disableMouseControl(session.sessionId, "user-a");
  assert.deepEqual(sessions.getOwned(session.sessionId, "user-a")?.permissions, ["SCREEN_VIEW", "MOUSE_CONTROL"]);
  assert.equal(sessions.getOwned(session.sessionId, "user-a")?.status, "connected");
  sessions.end(session.sessionId, "test_cleanup");
});
