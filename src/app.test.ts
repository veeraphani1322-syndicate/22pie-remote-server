import assert from "node:assert/strict";
import test from "node:test";
import { hash } from "@node-rs/argon2";
import type { AppConfig } from "./config/env.js";
import { createApp } from "./app.js";

async function fixture() {
  const suffix = `${process.pid}-${Math.random().toString(16).slice(2)}`;
  const config = {
    PORT: 4000, HOST: "127.0.0.1", LOG_LEVEL: "silent",
    CORS_ORIGIN: "http://8.234.114.242:3000",
    HEARTBEAT_TIMEOUT_MS: 30_000, HEARTBEAT_CHECK_INTERVAL_MS: 5_000,
    REGISTRATION_TIMEOUT_MS: 10_000, WS_MAX_PAYLOAD_BYTES: 16_384,
    JWT_SECRET: "test-secret-that-is-longer-than-thirty-two-characters",
    ADMIN_EMAIL: "owner@example.com", ADMIN_PASSWORD_HASH: await hash("test-password"),
    DEVICE_STORE_PATH: `/private/tmp/22pie-test-devices-${suffix}.json`,
    SESSION_APPROVAL_TIMEOUT_MS: 60_000, SESSION_NEGOTIATION_TIMEOUT_MS: 45_000,
    SESSION_DISCONNECT_TIMEOUT_MS: 15_000, PUBLIC_BASE_URL: "http://8.234.114.242:4000",
    ICE_SERVERS: "[]", corsOrigins: ["http://8.234.114.242:3000"], iceServers: [],
  } as AppConfig;
  return createApp(config);
}

test("credentialed CORS allows only the configured dashboard origin", async () => {
  const { app } = await fixture();
  const allowed = await app.inject({ method: "GET", url: "/api/health", headers: { origin: "http://8.234.114.242:3000" } });
  assert.equal(allowed.headers["access-control-allow-origin"], "http://8.234.114.242:3000");
  assert.equal(allowed.headers["access-control-allow-credentials"], "true");
  const denied = await app.inject({ method: "GET", url: "/api/health", headers: { origin: "http://evil.example" } });
  assert.equal(denied.headers["access-control-allow-origin"], undefined);
  await app.close();
});

test("login cookie authenticates me and devices while anonymous devices stays 401", async () => {
  const { app } = await fixture();
  const anonymous = await app.inject({ method: "GET", url: "/api/devices" });
  assert.equal(anonymous.statusCode, 401);

  const login = await app.inject({
    method: "POST", url: "/api/auth/login",
    payload: { email: "owner@example.com", password: "test-password" },
  });
  assert.equal(login.statusCode, 200);
  const header = login.headers["set-cookie"];
  const setCookie = Array.isArray(header) ? header[0] : header;
  assert.match(setCookie ?? "", /22pie_session=/);
  assert.match(setCookie ?? "", /HttpOnly/);
  assert.match(setCookie ?? "", /SameSite=Strict/);
  assert.doesNotMatch(setCookie ?? "", /; Secure/);
  const cookie = setCookie?.split(";")[0];
  assert.ok(cookie);

  const me = await app.inject({ method: "GET", url: "/api/auth/me", headers: { cookie } });
  assert.equal(me.statusCode, 200);
  assert.equal(me.json().user.email, "owner@example.com");
  const devices = await app.inject({ method: "GET", url: "/api/devices", headers: { cookie } });
  assert.equal(devices.statusCode, 200);
  assert.deepEqual(devices.json(), { count: 0, devices: [] });
  await app.close();
});
