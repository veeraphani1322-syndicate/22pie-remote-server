import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { DeviceManager } from "../devices/device-manager.js";
import type { AuthService } from "../auth/auth-service.js";
import { requireUser } from "./auth.js";

const paramsSchema = z.object({ deviceId: z.string().uuid() });

export function deviceRoutes(deviceManager: DeviceManager, auth: AuthService): FastifyPluginAsync {
  return async (app) => {
    app.get("/devices", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const devices = deviceManager.list(user.userId);
      return { count: devices.length, devices };
    });

    app.get("/devices/:deviceId", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      const device = deviceManager.getOwned(parsed.data.deviceId, user.userId);
      if (!device) return reply.code(404).send({ error: "Device not found" });
      return device;
    });
  };
}
