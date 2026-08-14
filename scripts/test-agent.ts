import "dotenv/config";
import { randomUUID } from "node:crypto";
import WebSocket from "ws";

const url = process.env.AGENT_WS_URL ?? "ws://localhost:4000/agent";
const deviceId = process.env.TEST_DEVICE_ID ?? randomUUID();
const socket = new WebSocket(url);
let heartbeatTimer: NodeJS.Timeout | undefined;

socket.on("open", () => {
  console.log(`Connected to ${url}`);
  console.log(`Test device ID: ${deviceId}`);
  socket.send(JSON.stringify({
    type: "register",
    deviceId,
    deviceName: "TEST-LAPTOP",
    operatingSystem: "Test/Windows",
    agentVersion: "0.1.0",
  }));
  heartbeatTimer = setInterval(() => socket.send(JSON.stringify({ type: "heartbeat" })), 5_000);
});

socket.on("message", (data) => console.log("Server:", data.toString()));
socket.on("error", (error) => console.error("WebSocket error:", error.message));
socket.on("close", (code, reason) => {
  if (heartbeatTimer) clearInterval(heartbeatTimer);
  console.log(`Disconnected (${code}${reason.length ? `: ${reason.toString()}` : ""})`);
});

process.once("SIGINT", () => {
  console.log("\nClosing test agent...");
  if (heartbeatTimer) clearInterval(heartbeatTimer);
  socket.close(1000, "Simulator stopped");
});
