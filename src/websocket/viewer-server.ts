import type { Server as HttpServer, IncomingMessage } from "node:http";
import type { Duplex } from "node:stream";
import type { FastifyBaseLogger } from "fastify";
import { WebSocketServer, type WebSocket } from "ws";
import type { AuthService } from "../auth/auth-service.js";
import { readSessionToken } from "../auth/auth-service.js";
import type { SessionManager } from "../sessions/session-manager.js";
import { viewerMessageSchema } from "../types/protocol.js";

export class ViewerWebSocketServer {
  private readonly wss = new WebSocketServer({ noServer: true, maxPayload: 1_048_576 });

  constructor(
    httpServer: HttpServer,
    private readonly auth: AuthService,
    private readonly sessions: SessionManager,
    private readonly logger: FastifyBaseLogger,
  ) {
    httpServer.on("upgrade", (request, socket, head) => {
      const url = new URL(request.url ?? "/", "http://localhost");
      if (url.pathname !== "/viewer") return;
      void this.authorizeUpgrade(request, socket, head, url.searchParams.get("sessionId"));
    });
  }

  private async authorizeUpgrade(
    request: IncomingMessage,
    socket: Duplex,
    head: Buffer,
    sessionId: string | null,
  ): Promise<void> {
    const token = readSessionToken(request.headers.cookie);
    const user = token ? await this.auth.verifyToken(token) : undefined;
    if (!user || !sessionId) {
      socket.write("HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n");
      socket.destroy();
      return;
    }
    this.wss.handleUpgrade(request, socket, head, (ws) => this.handleConnection(ws, sessionId, user.userId));
  }

  private handleConnection(socket: WebSocket, sessionId: string, userId: string): void {
    const session = this.sessions.attachViewer(sessionId, userId, socket);
    if (!session) {
      socket.close(1008, "Invalid or expired session");
      return;
    }
    socket.send(JSON.stringify({ type: "session_state", session }));
    this.sessions.flushPendingViewerSignaling(sessionId, userId, socket);
    socket.on("message", (data, isBinary) => {
      if (isBinary) return socket.close(1003, "Text messages required");
      try {
        const parsed = viewerMessageSchema.safeParse(JSON.parse(data.toString()));
        if (!parsed.success) return socket.send(JSON.stringify({ type: "error", code: "INVALID_MESSAGE", message: "Message failed validation" }));
        this.sessions.fromViewer(sessionId, userId, parsed.data);
      } catch {
        socket.send(JSON.stringify({ type: "error", code: "INVALID_JSON", message: "Message must be valid JSON" }));
      }
    });
    socket.on("error", (error) => this.logger.warn({ err: error, sessionId, userId }, "Viewer WebSocket error"));
    socket.on("close", () => {
      this.logger.info({ sessionId, userId }, "Viewer WebSocket closed");
      this.sessions.viewerDisconnected(sessionId, userId, socket);
    });
  }

  async close(): Promise<void> {
    for (const socket of this.wss.clients) socket.close(1001, "Server shutting down");
    await new Promise<void>((resolve) => this.wss.close(() => resolve()));
  }
}
