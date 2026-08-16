import { z } from "zod";

const sessionId = z.string().uuid();
const sdp = z.string().min(1).max(1_000_000);
const candidate = z.string().min(1).max(16_384);

export const registerMessageSchema = z.object({
  type: z.literal("register"),
  deviceId: z.string().uuid(),
  deviceName: z.string().trim().min(1).max(100),
  operatingSystem: z.string().trim().min(1).max(100),
  agentVersion: z.string().trim().min(1).max(50),
  publicKey: z.string().min(40).max(128),
}).strict();

export const authenticateMessageSchema = z.object({
  type: z.literal("authenticate"),
  signature: z.string().min(40).max(256),
}).strict();

export const heartbeatMessageSchema = z.object({ type: z.literal("heartbeat") }).strict();
export const sessionAcceptSchema = z.object({ type: z.literal("session_accept"), sessionId }).strict();
export const trustGrantSchema = z.object({
  type: z.literal("trust_grant"), sessionId, permission: z.enum(["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"]),
}).strict();
export const trustRevokeSchema = z.object({
  type: z.literal("trust_revoke"), userId: z.string().min(1).max(128), permission: z.enum(["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"]),
}).strict();
export const mouseControlAcceptSchema = z.object({ type: z.literal("mouse_control_accept"), sessionId }).strict();
export const mouseControlRejectSchema = z.object({ type: z.literal("mouse_control_reject"), sessionId, reason: z.string().max(200).optional() }).strict();
export const keyboardControlAcceptSchema = z.object({ type: z.literal("keyboard_control_accept"), sessionId }).strict();
export const keyboardControlRejectSchema = z.object({ type: z.literal("keyboard_control_reject"), sessionId, reason: z.string().max(200).optional() }).strict();
export const sessionRejectSchema = z.object({
  type: z.literal("session_reject"), sessionId, reason: z.string().max(200).optional(),
}).strict();
export const webRtcOfferSchema = z.object({ type: z.literal("webrtc_offer"), sessionId, sdp }).strict();
export const webRtcAnswerSchema = z.object({ type: z.literal("webrtc_answer"), sessionId, sdp }).strict();
export const iceCandidateSchema = z.object({
  type: z.literal("ice_candidate"), sessionId, candidate,
  sdpMid: z.string().max(256).nullable(), sdpMLineIndex: z.number().int().min(0).nullable(),
}).strict();
export const sessionEndSchema = z.object({
  type: z.literal("session_end"), sessionId, reason: z.string().max(200).optional(),
}).strict();
export const sessionConnectedSchema = z.object({ type: z.literal("session_connected"), sessionId }).strict();

export const agentMessageSchema = z.discriminatedUnion("type", [
  registerMessageSchema, authenticateMessageSchema, heartbeatMessageSchema,
  sessionAcceptSchema, trustGrantSchema, trustRevokeSchema, mouseControlAcceptSchema, mouseControlRejectSchema, keyboardControlAcceptSchema, keyboardControlRejectSchema, sessionRejectSchema, webRtcOfferSchema, webRtcAnswerSchema,
  iceCandidateSchema, sessionEndSchema,
]);

export const viewerMessageSchema = z.discriminatedUnion("type", [
  webRtcOfferSchema, webRtcAnswerSchema, iceCandidateSchema, sessionConnectedSchema, sessionEndSchema,
]);

export type RegisterMessage = z.infer<typeof registerMessageSchema>;
export type AgentMessage = z.infer<typeof agentMessageSchema>;
export type ViewerMessage = z.infer<typeof viewerMessageSchema>;

export type ServerMessage =
  | { type: "auth_challenge"; nonce: string }
  | { type: "registered"; deviceId: string; status: "online" }
  | { type: "heartbeat_ack"; timestamp: string }
  | { type: "session_requested"; sessionId: string; viewerUserId: string; viewerName: string; permissions: ["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"]; trusted: boolean; iceServers: Array<{ urls: string | string[]; username?: string; credential?: string }> }
  | { type: "mouse_control_requested"; sessionId: string; viewerUserId: string; viewerName: string; trusted: boolean }
  | { type: "mouse_control_enabled"; sessionId: string }
  | { type: "mouse_control_authorized"; sessionId: string }
  | { type: "mouse_control_rejected"; sessionId: string; reason: string }
  | { type: "mouse_control_disabled"; sessionId: string }
  | { type: "keyboard_control_requested"; sessionId: string; viewerUserId: string; viewerName: string; trusted: boolean }
  | { type: "keyboard_control_enabled"; sessionId: string }
  | { type: "keyboard_control_authorized"; sessionId: string }
  | { type: "keyboard_control_rejected"; sessionId: string; reason: string }
  | { type: "keyboard_control_disabled"; sessionId: string }
  | { type: "session_accepted"; sessionId: string }
  | { type: "session_rejected"; sessionId: string; reason?: string }
  | { type: "webrtc_offer"; sessionId: string; sdp: string }
  | { type: "webrtc_answer"; sessionId: string; sdp: string }
  | { type: "ice_candidate"; sessionId: string; candidate: string; sdpMid: string | null; sdpMLineIndex: number | null }
  | { type: "session_ended"; sessionId: string; reason: string }
  | { type: "trust_revoked"; userId: string; permission: "SCREEN_VIEW" | "MOUSE_CONTROL" | "KEYBOARD_CONTROL" }
  | { type: "error"; code: string; message: string };
