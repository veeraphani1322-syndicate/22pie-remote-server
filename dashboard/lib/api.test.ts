import assert from "node:assert/strict";
import test from "node:test";
import { api, ApiError } from "./api";

test("API client includes credentials for authenticated requests", async () => {
  let received: RequestInit | undefined;
  globalThis.fetch = (async (_input: string | URL | Request, init?: RequestInit) => {
    received = init;
    return new Response(JSON.stringify({ ok: true }), { status: 200, headers: { "Content-Type": "application/json" } });
  }) as typeof fetch;
  await api("/api/auth/me");
  assert.equal(received?.credentials, "include");
});

test("API client exposes unauthorized and network failures", async () => {
  globalThis.fetch = (async () => new Response(JSON.stringify({ error: "Unauthorized" }), { status: 401 })) as typeof fetch;
  await assert.rejects(api("/api/devices"), (error: unknown) => error instanceof ApiError && error.status === 401 && error.message === "Session expired");
  globalThis.fetch = (async () => { throw new TypeError("network"); }) as typeof fetch;
  await assert.rejects(api("/api/devices"), (error: unknown) => error instanceof ApiError && error.status === 0 && error.message === "Unable to connect to server");
});
