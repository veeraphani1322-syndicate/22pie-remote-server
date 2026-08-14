import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { DeviceManager } from "../devices/device-manager.js";

const paramsSchema = z.object({ deviceId: z.string().uuid() });

export function deviceRoutes(deviceManager: DeviceManager): FastifyPluginAsync {
  return async (app) => {
    app.get("/devices", async () => {
      const devices = deviceManager.list();
      return { count: devices.length, devices };
    });

    app.get("/devices/:deviceId", async (request, reply) => {
      const parsed = paramsSchema.safeParse(request.params);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid device ID" });
      const device = deviceManager.get(parsed.data.deviceId);
      if (!device) return reply.code(404).send({ error: "Device not found" });
      return device;
    });
  };
}
