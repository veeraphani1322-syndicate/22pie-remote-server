import { config } from "./config/env.js";
import { AgentWebSocketServer } from "./websocket/agent-server.js";
import { ViewerWebSocketServer } from "./websocket/viewer-server.js";
import { createApp } from "./app.js";

async function main(): Promise<void> {
  const { app, auth, devices, sessions } = await createApp(config);

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
