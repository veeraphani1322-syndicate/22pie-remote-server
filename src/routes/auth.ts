import type { FastifyPluginAsync, FastifyRequest } from "fastify";
import { z } from "zod";
import { type AuthService, readSessionToken } from "../auth/auth-service.js";

const loginSchema = z.object({
  email: z.string().email(),
  password: z.string().min(1).max(512),
}).strict();

export async function requireUser(request: FastifyRequest, auth: AuthService) {
  const token = readSessionToken(request.headers.cookie);
  return token ? auth.verifyToken(token) : undefined;
}

export function authRoutes(auth: AuthService, secureCookies: boolean): FastifyPluginAsync {
  return async (app) => {
    app.post("/auth/login", async (request, reply) => {
      const parsed = loginSchema.safeParse(request.body);
      if (!parsed.success) return reply.code(400).send({ error: "Invalid login request" });
      const token = await auth.login(parsed.data.email, parsed.data.password);
      if (!token) return reply.code(401).send({ error: "Invalid email or password" });
      reply.header(
        "Set-Cookie",
        `22pie_session=${encodeURIComponent(token)}; Path=/; HttpOnly; SameSite=Strict; Max-Age=28800${secureCookies ? "; Secure" : ""}`,
      );
      return { user: { email: parsed.data.email } };
    });

    app.post("/auth/logout", async (_request, reply) => {
      reply.header(
        "Set-Cookie",
        `22pie_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0${secureCookies ? "; Secure" : ""}`,
      );
      return { ok: true };
    });

    app.get("/auth/me", async (request, reply) => {
      const user = await requireUser(request, auth);
      if (!user) return reply.code(401).send({ error: "Unauthorized" });
      return { user };
    });
  };
}
