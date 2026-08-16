import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { DeviceManager } from "../devices/device-manager.js";
import type { AuthService } from "../auth/auth-service.js";
import { requireUser } from "./auth.js";
import WebSocket from "ws";
import type { TrustedAccessStore } from "../trust/trusted-access-store.js";

const paramsSchema = z.object({ deviceId: z.string().uuid() });

export function deviceRoutes(deviceManager: DeviceManager, auth: AuthService, trustedAccess: TrustedAccessStore): FastifyPluginAsync {
  return async (app) => {
    app.get("/devices", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const devices = deviceManager.list(user.userId).map((device) => {
        const trusts = trustedAccess.list(device.deviceId, user.userId, deviceManager.publicKeyFor(device.deviceId));
        const screenTrust = trusts.find((trust) => trust.permission === "SCREEN_VIEW");
        const mouseTrust = trusts.find((trust) => trust.permission === "MOUSE_CONTROL");
        const keyboardTrust = trusts.find((trust) => trust.permission === "KEYBOARD_CONTROL");
        return {
          ...device,
          trustedScreenAccess: Boolean(screenTrust), trustedMouseControl: Boolean(mouseTrust), trustedKeyboardControl: Boolean(keyboardTrust), trustedRemoteControl: Boolean(screenTrust && mouseTrust && keyboardTrust),
          trustedAccess: screenTrust ? { permission: screenTrust.permission, createdAt: screenTrust.createdAt, userEmail: user.email } : undefined,
        };
      });
      return { count: devices.length, devices };
    });

    app.get("/devices/:deviceId", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      const device = deviceManager.getOwned(parsed.data.deviceId, user.userId);
      if (!device) return reply.code(404).send({ error: "Device not found" });
      return { ...device, trustedScreenAccess: trustedAccess.has(device.deviceId, user.userId, "SCREEN_VIEW", deviceManager.publicKeyFor(device.deviceId)), trustedMouseControl: trustedAccess.has(device.deviceId, user.userId, "MOUSE_CONTROL", deviceManager.publicKeyFor(device.deviceId)), trustedKeyboardControl: trustedAccess.has(device.deviceId, user.userId, "KEYBOARD_CONTROL", deviceManager.publicKeyFor(device.deviceId)), trustedRemoteControl: trustedAccess.hasRemoteControl(device.deviceId, user.userId, deviceManager.publicKeyFor(device.deviceId)) };
    });

    app.get("/devices/:deviceId/trusted-access", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      if (!deviceManager.getOwned(parsed.data.deviceId, user.userId)) return reply.code(404).send({ error: "Device not found" });
      return { trustedAccess: trustedAccess.list(parsed.data.deviceId, user.userId, deviceManager.publicKeyFor(parsed.data.deviceId)).map((record) => ({ ...record, userEmail: user.email })) };
    });

    app.delete("/devices/:deviceId/trusted-access/SCREEN_VIEW", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      if (!deviceManager.getOwned(parsed.data.deviceId, user.userId)) return reply.code(404).send({ error: "Device not found" });
      const revoked = trustedAccess.revoke(parsed.data.deviceId, user.userId, "SCREEN_VIEW");
      const socket = deviceManager.socketFor(parsed.data.deviceId);
      if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: "trust_revoked", userId: user.userId, permission: "SCREEN_VIEW" }));
      if (revoked) request.log.info({ event: "TRUST_REVOKED", deviceId: parsed.data.deviceId, userId: user.userId, permission: "SCREEN_VIEW" }, "Trusted access revoked from dashboard");
      return reply.code(204).send();
    });

    app.delete("/devices/:deviceId/trusted-access/MOUSE_CONTROL", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      if (!deviceManager.getOwned(parsed.data.deviceId, user.userId)) return reply.code(404).send({ error: "Device not found" });
      const revoked = trustedAccess.revoke(parsed.data.deviceId, user.userId, "MOUSE_CONTROL");
      const socket = deviceManager.socketFor(parsed.data.deviceId);
      if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: "trust_revoked", userId: user.userId, permission: "MOUSE_CONTROL" }));
      if (revoked) request.log.info({ event: "MOUSE_CONTROL_REVOKED", deviceId: parsed.data.deviceId, userId: user.userId }, "Mouse-control trust revoked");
      return reply.code(204).send();
    });

    app.delete("/devices/:deviceId/trusted-access/KEYBOARD_CONTROL", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      if (!deviceManager.getOwned(parsed.data.deviceId, user.userId)) return reply.code(404).send({ error: "Device not found" });
      const revoked = trustedAccess.revoke(parsed.data.deviceId, user.userId, "KEYBOARD_CONTROL");
      const socket = deviceManager.socketFor(parsed.data.deviceId);
      if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: "trust_revoked", userId: user.userId, permission: "KEYBOARD_CONTROL" }));
      if (revoked) request.log.info({ event: "KEYBOARD_CONTROL_REVOKED", deviceId: parsed.data.deviceId, userId: user.userId }, "Keyboard-control trust revoked");
      return reply.code(204).send();
    });

    app.delete("/devices/:deviceId/trusted-access/REMOTE_CONTROL", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      if (!deviceManager.getOwned(parsed.data.deviceId, user.userId)) return reply.code(404).send({ error: "Device not found" });
      const revoked = trustedAccess.revokeRemoteControl(parsed.data.deviceId, user.userId);
      const socket = deviceManager.socketFor(parsed.data.deviceId);
      if (socket?.readyState === WebSocket.OPEN) for (const permission of ["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"]) socket.send(JSON.stringify({ type: "trust_revoked", userId: user.userId, permission }));
      if (revoked.length) request.log.info({ event: "REMOTE_CONTROL_REVOKED", deviceId: parsed.data.deviceId, userId: user.userId }, "Remote-control trust revoked");
      return reply.code(204).send();
    });
  };
}
