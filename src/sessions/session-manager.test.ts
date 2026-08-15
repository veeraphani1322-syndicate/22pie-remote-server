import assert from "node:assert/strict";
import test from "node:test";
import WebSocket from "ws";
import type { DeviceManager } from "../devices/device-manager.js";
import { SessionManager } from "./session-manager.js";

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
  const devices = { socketFor: () => agent } as unknown as DeviceManager;
  const sessions = new SessionManager(devices, 60_000, 60_000, []);
  return { agent, sessions };
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
