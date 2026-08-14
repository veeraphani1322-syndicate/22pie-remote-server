import { z } from "zod";

export const registerMessageSchema = z.object({
  type: z.literal("register"),
  deviceId: z.string().uuid(),
  deviceName: z.string().trim().min(1).max(100),
  operatingSystem: z.string().trim().min(1).max(100),
  agentVersion: z.string().trim().min(1).max(50),
}).strict();

export const heartbeatMessageSchema = z.object({
  type: z.literal("heartbeat"),
}).strict();

export const agentMessageSchema = z.discriminatedUnion("type", [
  registerMessageSchema,
  heartbeatMessageSchema,
]);

export type RegisterMessage = z.infer<typeof registerMessageSchema>;
export type AgentMessage = z.infer<typeof agentMessageSchema>;

export type ServerMessage =
  | { type: "registered"; deviceId: string; status: "online" }
  | { type: "heartbeat_ack"; timestamp: string }
  | { type: "error"; code: string; message: string };
