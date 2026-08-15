import Fastify from "fastify";
import cors from "@fastify/cors";
import helmet from "@fastify/helmet";
import type { AppConfig } from "./config/env.js";
import { AuthService } from "./auth/auth-service.js";
import { DeviceManager } from "./devices/device-manager.js";
import { authRoutes } from "./routes/auth.js";
import { deviceRoutes } from "./routes/devices.js";
import { healthRoutes } from "./routes/health.js";
import { sessionRoutes } from "./routes/sessions.js";
import { SessionManager } from "./sessions/session-manager.js";
import { TrustedAccessStore } from "./trust/trusted-access-store.js";

export async function createApp(appConfig: AppConfig) {
  const app = Fastify({ logger: { level: appConfig.LOG_LEVEL }, bodyLimit: 64 * 1024 });
  const devices = new DeviceManager(appConfig.DEVICE_STORE_PATH);
  const trustedAccess = new TrustedAccessStore(appConfig.TRUST_STORE_PATH ?? "data/trusted-access.json");
  const auth = new AuthService(appConfig.ADMIN_EMAIL, appConfig.ADMIN_PASSWORD_HASH, appConfig.JWT_SECRET);
  const sessions = new SessionManager(
    devices,
    appConfig.SESSION_APPROVAL_TIMEOUT_MS,
    appConfig.SESSION_NEGOTIATION_TIMEOUT_MS,
    appConfig.iceServers,
    trustedAccess,
    (data, message) => app.log.info(data, message),
  );

  await app.register(helmet);
  await app.register(cors, {
    credentials: true,
    origin: (origin, callback) => {
      if (!origin || appConfig.corsOrigins.includes(origin)) callback(null, true);
      else callback(null, false);
    },
  });
  await app.register(healthRoutes, { prefix: "/api" });
  await app.register(authRoutes(auth, appConfig.PUBLIC_BASE_URL.startsWith("https://")), { prefix: "/api" });
  await app.register(deviceRoutes(devices, auth, trustedAccess), { prefix: "/api" });
  await app.register(sessionRoutes(auth, devices, sessions, appConfig.iceServers), { prefix: "/api" });

  return { app, auth, devices, sessions, trustedAccess };
}
