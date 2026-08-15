import assert from "node:assert/strict";
import test from "node:test";
import { agentMessageSchema, viewerMessageSchema } from "./protocol.js";

test("accepts a typed screen-only session response", () => {
  assert.equal(agentMessageSchema.safeParse({
    type: "session_accept",
    sessionId: "4f63cc0c-f1f9-4e4f-9f56-ced4e345d813",
  }).success, true);
});

test("rejects signaling with unknown fields", () => {
  assert.equal(viewerMessageSchema.safeParse({
    type: "webrtc_offer",
    sessionId: "4f63cc0c-f1f9-4e4f-9f56-ced4e345d813",
    sdp: "v=0",
    command: "not-allowed",
  }).success, false);
});

test("accepts device-originated permission-specific trust messages", () => {
  assert.equal(agentMessageSchema.safeParse({
    type: "trust_grant", sessionId: "4f63cc0c-f1f9-4e4f-9f56-ced4e345d813", permission: "SCREEN_VIEW",
  }).success, true);
  assert.equal(agentMessageSchema.safeParse({
    type: "trust_grant", sessionId: "4f63cc0c-f1f9-4e4f-9f56-ced4e345d813", permission: "MOUSE_CONTROL",
  }).success, true);
});

test("accepts mouse authorization results but rejects unknown control messages", () => {
  assert.equal(agentMessageSchema.safeParse({ type: "mouse_control_accept", sessionId: "4f63cc0c-f1f9-4e4f-9f56-ced4e345d813" }).success, true);
  assert.equal(agentMessageSchema.safeParse({ type: "mouse_control_accept", sessionId: "bad" }).success, false);
});
