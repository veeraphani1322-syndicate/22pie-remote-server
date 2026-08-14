import Fastify from "fastify";
import cors from "@fastify/cors";
import helmet from "@fastify/helmet";
import { config } from "./config/env.js";
import { DeviceManager } from "./devices/device-manager.js";
import { deviceRoutes } from "./routes/devices.js";
import { healthRoutes } from "./routes/health.js";
import { AgentWebSocketServer } from "./websocket/agent-server.js";
import { ViewerWebSocketServer } from "./websocket/viewer-server.js";
import { AuthService } from "./auth/auth-service.js";
import { authRoutes } from "./routes/auth.js";
import { SessionManager } from "./sessions/session-manager.js";
import { sessionRoutes } from "./routes/sessions.js";

async function main(): Promise<void> {
  const app = Fastify({
    logger: { level: config.LOG_LEVEL },
    bodyLimit: 64 * 1024,
  });
  const devices = new DeviceManager(config.DEVICE_STORE_PATH);
  const auth = new AuthService(config.ADMIN_EMAIL, config.ADMIN_PASSWORD_HASH, config.JWT_SECRET);
  const sessions = new SessionManager(devices, config.SESSION_APPROVAL_TIMEOUT_MS, config.SESSION_NEGOTIATION_TIMEOUT_MS, config.iceServers);

  await app.register(helmet);
  await app.register(cors, {
    origin: (origin, callback) => {
      if (!origin || config.corsOrigins.includes(origin)) callback(null, true);
      else callback(null, false);
    },
  });
  await app.register(healthRoutes, { prefix: "/api" });
  await app.register(authRoutes(auth, config.PUBLIC_BASE_URL.startsWith("https://")), { prefix: "/api" });
  await app.register(deviceRoutes(devices, auth), { prefix: "/api" });
  await app.register(sessionRoutes(auth, devices, sessions, config.iceServers), { prefix: "/api" });

  const agentServer = new AgentWebSocketServer(app.server, devices, config, app.log, sessions);
  const viewerServer = new ViewerWebSocketServer(app.server, auth, sessions, app.log);
  let shuttingDown = false;
  const shutdown = async (signal: string): Promise<void> => {
    if (shuttingDown) return;
    shuttingDown = true;
    app.log.info({ signal }, "Graceful shutdown started");
    try {
      await agentServer.close();
      sessions.close();
      await viewerServer.close();
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
