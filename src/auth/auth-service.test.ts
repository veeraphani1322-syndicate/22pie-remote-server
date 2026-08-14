import assert from "node:assert/strict";
import test from "node:test";
import { hash } from "@node-rs/argon2";
import { AuthService } from "./auth-service.js";

test("issues and verifies a bounded admin session", async () => {
  const service = new AuthService(
    "owner@example.com",
    await hash("correct horse battery staple"),
    "a-test-secret-that-is-at-least-thirty-two-bytes",
  );
  assert.equal(await service.login("owner@example.com", "wrong"), undefined);
  const token = await service.login("owner@example.com", "correct horse battery staple");
  assert.ok(token);
  assert.deepEqual(await service.verifyToken(token), { userId: "admin", email: "owner@example.com" });
});
