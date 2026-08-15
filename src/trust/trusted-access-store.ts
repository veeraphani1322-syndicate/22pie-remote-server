import { randomUUID } from "node:crypto";
import { dirname } from "node:path";
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";

export type TrustedPermission = "SCREEN_VIEW";

export interface TrustedAccessRecord {
  trustId: string;
  deviceId: string;
  userId: string;
  devicePublicKey: string;
  permission: TrustedPermission;
  createdAt: string;
  createdByDevice: true;
  expiresAt?: string;
  revokedAt?: string;
}

export class TrustedAccessStore {
  private readonly records = new Map<string, TrustedAccessRecord>();

  constructor(private readonly storePath: string) { this.load(); }

  grant(deviceId: string, userId: string, permission: TrustedPermission, devicePublicKey: string): TrustedAccessRecord {
    const existing = this.find(deviceId, userId, permission);
    if (existing && !existing.revokedAt && existing.devicePublicKey === devicePublicKey) return { ...existing };
    const record: TrustedAccessRecord = {
      trustId: randomUUID(), deviceId, userId, permission,
      createdAt: new Date().toISOString(), createdByDevice: true, devicePublicKey,
    };
    this.records.set(record.trustId, record);
    this.persist();
    return { ...record };
  }

  has(deviceId: string, userId: string, permission: TrustedPermission, devicePublicKey: string | undefined): boolean {
    const record = this.find(deviceId, userId, permission);
    return Boolean(record && devicePublicKey && record.devicePublicKey === devicePublicKey && !record.revokedAt && (!record.expiresAt || Date.parse(record.expiresAt) > Date.now()));
  }

  list(deviceId: string, userId: string, devicePublicKey?: string): TrustedAccessRecord[] {
    return [...this.records.values()]
      .filter((record) => record.deviceId === deviceId && record.userId === userId && !record.revokedAt && (!devicePublicKey || record.devicePublicKey === devicePublicKey))
      .map((record) => ({ ...record }));
  }

  revoke(deviceId: string, userId: string, permission: TrustedPermission): TrustedAccessRecord | undefined {
    const record = this.find(deviceId, userId, permission);
    if (!record || record.revokedAt) return undefined;
    record.revokedAt = new Date().toISOString();
    this.persist();
    return { ...record };
  }

  private find(deviceId: string, userId: string, permission: TrustedPermission): TrustedAccessRecord | undefined {
    return [...this.records.values()].find((record) =>
      record.deviceId === deviceId && record.userId === userId && record.permission === permission,
    );
  }

  private load(): void {
    try {
      const records = JSON.parse(readFileSync(this.storePath, "utf8")) as TrustedAccessRecord[];
      for (const record of records) this.records.set(record.trustId, record);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
    }
  }

  private persist(): void {
    mkdirSync(dirname(this.storePath), { recursive: true });
    const temporary = `${this.storePath}.tmp`;
    writeFileSync(temporary, `${JSON.stringify([...this.records.values()], null, 2)}\n`, { mode: 0o600 });
    renameSync(temporary, this.storePath);
  }
}
