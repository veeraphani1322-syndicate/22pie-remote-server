import { verify } from "@node-rs/argon2";
import { jwtVerify, SignJWT } from "jose";

export interface AuthUser {
  userId: string;
  email: string;
}

const ADMIN_USER_ID = "admin";

export class AuthService {
  private readonly key: Uint8Array;

  constructor(
    private readonly email: string,
    private readonly passwordHash: string,
    jwtSecret: string,
  ) {
    this.key = new TextEncoder().encode(jwtSecret);
  }

  async login(email: string, password: string): Promise<string | undefined> {
    if (email.toLowerCase() !== this.email.toLowerCase()) return undefined;
    if (!(await verify(this.passwordHash, password))) return undefined;
    return new SignJWT({ email: this.email })
      .setProtectedHeader({ alg: "HS256" })
      .setSubject(ADMIN_USER_ID)
      .setIssuedAt()
      .setExpirationTime("8h")
      .sign(this.key);
  }

  async verifyToken(token: string): Promise<AuthUser | undefined> {
    try {
      const { payload } = await jwtVerify(token, this.key, { algorithms: ["HS256"] });
      if (payload.sub !== ADMIN_USER_ID || typeof payload.email !== "string") return undefined;
      return { userId: payload.sub, email: payload.email };
    } catch {
      return undefined;
    }
  }
}

export function readSessionToken(cookieHeader?: string): string | undefined {
  if (!cookieHeader) return undefined;
  for (const part of cookieHeader.split(";")) {
    const [name, ...value] = part.trim().split("=");
    if (name === "22pie_session") return decodeURIComponent(value.join("="));
  }
  return undefined;
}
