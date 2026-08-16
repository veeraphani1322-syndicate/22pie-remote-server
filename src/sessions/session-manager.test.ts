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
  for (const permission of ["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"] as const) sessions.fromAgent("device-a", { type: "trust_grant", sessionId: first.sessionId, permission });
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: first.sessionId });
  sessions.end(first.sessionId, "test");

  const second = sessions.request("user-a", "device-a", "Viewer");
  assert.equal(second.trusted, true);
  assert.equal(trust.hasRemoteControl("device-a", "user-a", "test-device-key"), true);
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

test("remote-control sessions request all permissions and require complete trust", () => {
  const { agent, sessions, trust } = fixture();
  trust.grant("device-a", "user-a", "SCREEN_VIEW", "test-device-key");
  const partial = sessions.request("user-a", "device-a", "Viewer");
  assert.equal(partial.trusted, false);
  assert.deepEqual(partial.permissions, ["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"]);
  assert.equal((agent.messages.at(-1) as { trusted: boolean }).trusted, false);
  for (const permission of ["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"] as const) sessions.fromAgent("device-a", { type: "trust_grant", sessionId: partial.sessionId, permission });
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: partial.sessionId });
  sessions.end(partial.sessionId, "test_cleanup");
  const trusted = sessions.request("user-a", "device-a", "Viewer");
  assert.equal(trusted.trusted, true);
  assert.equal(trust.hasRemoteControl("device-a", "user-a", "test-device-key"), true);
  sessions.end(trusted.sessionId, "test_cleanup");
});

test("buffers an early agent offer and bounded ICE until the viewer attaches", () => {
  const { agent, sessions } = fixture();
  const session = sessions.request("user-a", "device-a", "Viewer");
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: session.sessionId });
  sessions.fromAgent("device-a", { type: "webrtc_offer", sessionId: session.sessionId, sdp: "offer-b" });
  for (let index = 0; index < 70; index += 1) {
    sessions.fromAgent("device-a", { type: "ice_candidate", sessionId: session.sessionId, candidate: `candidate-${index}`, sdpMid: "0", sdpMLineIndex: 0 });
  }

  const viewer = fakeSocket();
  sessions.attachViewer(session.sessionId, "user-a", viewer);
  sessions.flushPendingViewerSignaling(session.sessionId, "user-a", viewer);

  assert.deepEqual(viewer.messages[0], { type: "webrtc_offer", sessionId: session.sessionId, sdp: "offer-b" });
  assert.equal(viewer.messages.length, 65);
  assert.equal((viewer.messages[1] as { candidate: string }).candidate, "candidate-6");
  assert.equal((viewer.messages.at(-1) as { candidate: string }).candidate, "candidate-69");
  sessions.flushPendingViewerSignaling(session.sessionId, "user-a", viewer);
  assert.equal(viewer.messages.length, 65, "pending signaling is delivered exactly once");
  sessions.end(session.sessionId, "test_cleanup");
  assert.equal(agent.messages.at(-1) && (agent.messages.at(-1) as { sessionId?: string }).sessionId, session.sessionId);
});

test("ten immediate reconnect cycles use fresh combined-trust session signaling", () => {
  const { agent, sessions } = fixture();
  let previousSessionId: string | undefined;
  for (let cycle = 0; cycle < 10; cycle += 1) {
    const session = sessions.request("user-a", "device-a", "Viewer");
    assert.notEqual(session.sessionId, previousSessionId);
    sessions.fromAgent("device-a", { type: "session_accept", sessionId: session.sessionId });
    sessions.fromAgent("device-a", { type: "webrtc_offer", sessionId: session.sessionId, sdp: `offer-${cycle}` });
    const viewer = fakeSocket();
    sessions.attachViewer(session.sessionId, "user-a", viewer);
    sessions.flushPendingViewerSignaling(session.sessionId, "user-a", viewer);
    assert.deepEqual(viewer.messages, [{ type: "webrtc_offer", sessionId: session.sessionId, sdp: `offer-${cycle}` }]);
    sessions.fromViewer(session.sessionId, "user-a", { type: "webrtc_answer", sessionId: session.sessionId, sdp: `answer-${cycle}` });
    assert.deepEqual(agent.messages.at(-1), { type: "webrtc_answer", sessionId: session.sessionId, sdp: `answer-${cycle}` });
    sessions.fromViewer(session.sessionId, "user-a", { type: "session_connected", sessionId: session.sessionId });
    assert.equal(sessions.getOwned(session.sessionId, "user-a")?.status, "connected");
    sessions.viewerDisconnected(session.sessionId, "user-a", viewer);
    assert.equal(sessions.getOwned(session.sessionId, "user-a")?.status, "ended");
    previousSessionId = session.sessionId;
  }
});

test("ended session signaling is cleared and never delivered to a new session", () => {
  const { sessions } = fixture();
  const first = sessions.request("user-a", "device-a", "Viewer");
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: first.sessionId });
  sessions.fromAgent("device-a", { type: "webrtc_offer", sessionId: first.sessionId, sdp: "stale-offer" });
  sessions.end(first.sessionId, "viewer_left");

  const second = sessions.request("user-a", "device-a", "Viewer");
  const viewer = fakeSocket();
  sessions.attachViewer(second.sessionId, "user-a", viewer);
  sessions.flushPendingViewerSignaling(second.sessionId, "user-a", viewer);
  assert.deepEqual(viewer.messages, []);
  sessions.end(second.sessionId, "test_cleanup");
});

test("a failed negotiation is terminal and an immediate fresh session can connect", () => {
  const { sessions } = fixture();
  const failed = sessions.request("user-a", "device-a", "Viewer");
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: failed.sessionId });
  sessions.fromAgent("device-a", { type: "webrtc_offer", sessionId: failed.sessionId, sdp: "failed-offer" });
  sessions.end(failed.sessionId, "webrtc_failed", "failed");
  const recovered = sessions.request("user-a", "device-a", "Viewer");
  sessions.fromAgent("device-a", { type: "session_accept", sessionId: recovered.sessionId });
  sessions.fromAgent("device-a", { type: "webrtc_offer", sessionId: recovered.sessionId, sdp: "fresh-offer" });
  const viewer = fakeSocket();
  sessions.attachViewer(recovered.sessionId, "user-a", viewer);
  sessions.flushPendingViewerSignaling(recovered.sessionId, "user-a", viewer);
  assert.deepEqual(viewer.messages, [{ type: "webrtc_offer", sessionId: recovered.sessionId, sdp: "fresh-offer" }]);
  sessions.fromViewer(recovered.sessionId, "user-a", { type: "session_connected", sessionId: recovered.sessionId });
  assert.equal(sessions.getOwned(recovered.sessionId, "user-a")?.status, "connected");
  sessions.end(recovered.sessionId, "test_cleanup");
});
