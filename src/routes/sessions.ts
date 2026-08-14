import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { AuthService } from "../auth/auth-service.js";
import type { DeviceManager } from "../devices/device-manager.js";
import type { SessionManager } from "../sessions/session-manager.js";
import { requireUser } from "./auth.js";

const deviceParams = z.object({ deviceId: z.string().uuid() });
const sessionParams = z.object({ sessionId: z.string().uuid() });

export function sessionRoutes(
  auth: AuthService,
  devices: DeviceManager,
  sessions: SessionManager,
  iceServers: Array<{ urls: string | string[]; username?: string; credential?: string }>,
): FastifyPluginAsync {
  return async (app) => {
    app.post("/devices/:deviceId/sessions", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = deviceParams.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      const device = devices.getOwned(parsed.data.deviceId, user.userId);
      if (!device) return reply.code(404).send({ error: "Device not found" });
      if (device.status !== "online") return reply.code(409).send({ error: "Device is offline" });
      const session = sessions.request(user.userId, device.deviceId, user.email);
      return reply.code(201).send({ session, iceServers });
    });

    app.get("/sessions/:sessionId", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = sessionParams.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid session ID" });
      const session = sessions.getOwned(parsed.data.sessionId, user.userId);
      if (!session) return reply.code(404).send({ error: "Session not found" });
      return { session, iceServers };
    });
  };
}
