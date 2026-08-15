import assert from "node:assert/strict";
import test from "node:test";
import { api, ApiError } from "./api";

function captureRequest() {
  let received: RequestInit | undefined;
  globalThis.fetch = (async (_input: string | URL | Request, init?: RequestInit) => {
    received = init;
    return new Response(JSON.stringify({ ok: true }), { status: 200, headers: { "Content-Type": "application/json" } });
  }) as typeof fetch;
  return () => received;
}

test("GET includes credentials without adding JSON content type", async () => {
  const received = captureRequest();
  await api("/api/devices");
  assert.equal(received()?.credentials, "include");
  assert.equal(new Headers(received()?.headers).has("Content-Type"), false);
});

test("bodyless session POST does not add JSON content type", async () => {
  const received = captureRequest();
  await api("/api/devices/4f63cc0c-f1f9-4e4f-9f56-ced4e345d813/sessions", { method: "POST" });
  assert.equal(received()?.credentials, "include");
  assert.equal(new Headers(received()?.headers).has("Content-Type"), false);
});

test("JSON login POST adds JSON content type and preserves caller headers", async () => {
  const received = captureRequest();
  await api("/api/auth/login", {
    method: "POST",
    body: JSON.stringify({ email: "owner@example.com", password: "secret" }),
    headers: { "X-Request-Source": "dashboard" },
  });
  const headers = new Headers(received()?.headers);
  assert.equal(received()?.credentials, "include");
  assert.equal(headers.get("Content-Type"), "application/json");
  assert.equal(headers.get("X-Request-Source"), "dashboard");
});

test("API client exposes unauthorized and network failures", async () => {
  globalThis.fetch = (async () => new Response(JSON.stringify({ error: "Unauthorized" }), { status: 401 })) as typeof fetch;
  await assert.rejects(api("/api/devices"), (error: unknown) => error instanceof ApiError && error.status === 401 && error.message === "Session expired");
  globalThis.fetch = (async () => { throw new TypeError("network"); }) as typeof fetch;
  await assert.rejects(api("/api/devices"), (error: unknown) => error instanceof ApiError && error.status === 0 && error.message === "Unable to connect to server");
});
