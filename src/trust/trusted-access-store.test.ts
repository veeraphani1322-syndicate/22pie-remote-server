import assert from "node:assert/strict";
import test from "node:test";
import { TrustedAccessStore } from "./trusted-access-store.js";

function fixture() {
  return new TrustedAccessStore(`/private/tmp/22pie-trust-${process.pid}-${Math.random()}.json`);
}

test("trust is scoped by device, user, and permission and survives reload", () => {
  const path = `/private/tmp/22pie-trust-persist-${process.pid}-${Math.random()}.json`;
  const store = new TrustedAccessStore(path);
  const record = store.grant("device-a", "user-a", "SCREEN_VIEW", "key-a");
  assert.equal(record.createdByDevice, true);
  assert.equal(new TrustedAccessStore(path).has("device-a", "user-a", "SCREEN_VIEW", "key-a"), true);
  assert.equal(store.has("device-a", "user-b", "SCREEN_VIEW", "key-a"), false);
  assert.equal(store.has("device-b", "user-a", "SCREEN_VIEW", "key-a"), false);
  assert.equal(store.has("device-a", "user-a", "SCREEN_VIEW", "replacement-key"), false);
  store.grant("device-a", "user-a", "MOUSE_CONTROL", "key-a");
  assert.equal(store.has("device-a", "user-a", "MOUSE_CONTROL", "key-a"), true);
  store.revoke("device-a", "user-a", "MOUSE_CONTROL");
  assert.equal(store.has("device-a", "user-a", "SCREEN_VIEW", "key-a"), true);
  store.grant("device-a", "user-a", "KEYBOARD_CONTROL", "key-a");
  assert.equal(store.has("device-a", "user-a", "KEYBOARD_CONTROL", "key-a"), true);
  store.revoke("device-a", "user-a", "KEYBOARD_CONTROL");
  assert.equal(store.has("device-a", "user-a", "SCREEN_VIEW", "key-a"), true);
});

test("revocation disables trust without affecting another account", () => {
  const store = fixture();
  store.grant("device-a", "user-a", "SCREEN_VIEW", "key-a");
  store.grant("device-a", "user-b", "SCREEN_VIEW", "key-a");
  store.revoke("device-a", "user-a", "SCREEN_VIEW");
  assert.equal(store.has("device-a", "user-a", "SCREEN_VIEW", "key-a"), false);
  assert.equal(store.has("device-a", "user-b", "SCREEN_VIEW", "key-a"), true);
});

test("remote-control trust requires and revokes the complete permission bundle", () => {
  const store = fixture();
  store.grant("device-a", "user-a", "SCREEN_VIEW", "key-a");
  assert.equal(store.hasRemoteControl("device-a", "user-a", "key-a"), false);
  store.grantRemoteControl("device-a", "user-a", "key-a");
  assert.equal(store.hasRemoteControl("device-a", "user-a", "key-a"), true);
  assert.equal(store.hasRemoteControl("device-a", "user-b", "key-a"), false);
  assert.equal(store.hasRemoteControl("device-a", "user-a", "wrong-key"), false);
  assert.equal(store.revokeRemoteControl("device-a", "user-a").length, 3);
  assert.equal(store.hasRemoteControl("device-a", "user-a", "key-a"), false);
});
