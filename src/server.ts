import Fastify from "fastify";
import cors from "@fastify/cors";
import helmet from "@fastify/helmet";
import { config } from "./config/env.js";
import { DeviceManager } from "./devices/device-manager.js";
import { deviceRoutes } from "./routes/devices.js";
import { healthRoutes } from "./routes/health.js";
import { AgentWebSocketServer } from "./websocket/agent-server.js";

async function main(): Promise<void> {
  const app = Fastify({
    logger: { level: config.LOG_LEVEL },
    bodyLimit: 64 * 1024,
  });
  const devices = new DeviceManager();

  await app.register(helmet);
  await app.register(cors, {
    origin: (origin, callback) => {
      if (!origin || config.corsOrigins.includes(origin)) callback(null, true);
      else callback(null, false);
    },
  });
  await app.register(healthRoutes, { prefix: "/api" });
  await app.register(deviceRoutes(devices), { prefix: "/api" });

  const agentServer = new AgentWebSocketServer(app.server, devices, config, app.log);
  let shuttingDown = false;
  const shutdown = async (signal: string): Promise<void> => {
    if (shuttingDown) return;
    shuttingDown = true;
    app.log.info({ signal }, "Graceful shutdown started");
    try {
      await agentServer.close();
      await app.close();
      process.exitCode = 0;
    } catch (error) {
      app.log.error({ err: error }, "Graceful shutdown failed");
      process.exitCode = 1;
    }
  };
  process.once("SIGINT", () => void shutdown("SIGINT"));
  process.once("SIGTERM", () => void shutdown("SIGTERM"));

  try {
    await app.listen({ host: config.HOST, port: config.PORT });
    app.log.info(`22Pie Remote Server started\n\nHTTP:\nhttp://localhost:${config.PORT}\n\nAgent WebSocket:\nws://localhost:${config.PORT}/agent`);
  } catch (error) {
    app.log.error({ err: error }, "Unable to start server");
    process.exitCode = 1;
  }
}

void main();
