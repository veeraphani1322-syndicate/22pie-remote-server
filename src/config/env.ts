import "dotenv/config";
import { z } from "zod";

const envSchema = z.object({
  PORT: z.coerce.number().int().min(1).max(65535).default(4000),
  HOST: z.string().min(1).default("0.0.0.0"),
  LOG_LEVEL: z.enum(["fatal", "error", "warn", "info", "debug", "trace", "silent"]).default("info"),
  CORS_ORIGIN: z.string().default("http://localhost:3000"),
  HEARTBEAT_TIMEOUT_MS: z.coerce.number().int().positive().default(30_000),
  HEARTBEAT_CHECK_INTERVAL_MS: z.coerce.number().int().positive().default(5_000),
  REGISTRATION_TIMEOUT_MS: z.coerce.number().int().positive().default(10_000),
  WS_MAX_PAYLOAD_BYTES: z.coerce.number().int().positive().max(1_048_576).default(16_384),
});

const parsed = envSchema.safeParse(process.env);
if (!parsed.success) {
  console.error("Invalid environment configuration", parsed.error.flatten().fieldErrors);
  process.exit(1);
}

const corsOrigins = parsed.data.CORS_ORIGIN.split(",").map((origin) => origin.trim()).filter(Boolean);

export const config = {
  ...parsed.data,
  corsOrigins,
};

export type AppConfig = typeof config;
