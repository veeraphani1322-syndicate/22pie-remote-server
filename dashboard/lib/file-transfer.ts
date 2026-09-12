import { sha256 } from "@noble/hashes/sha2.js";

export const MAX_FILE_BYTES = 32 * 1024 * 1024;
const CHUNK_BYTES = 16 * 1024;
export type TransferProgress = { status: "idle" | "hashing" | "awaiting" | "sending" | "verifying" | "complete" | "cancelled" | "failed"; name: string; sent: number; total: number; message?: string };
export const idleTransfer: TransferProgress = { status: "idle", name: "", sent: 0, total: 0 };

type Pending = { type: string; offset?: number; resolve: () => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> };

export class FileSender {
  private pending: Pending | null = null;
  private id: string | null = null;
  private cancelled = false;
  private progress = idleTransfer;
  constructor(private channel: RTCDataChannel, private sessionId: string, private update: (progress: TransferProgress) => void) {
    channel.addEventListener("message", this.onMessage);
    channel.addEventListener("close", this.onClose);
    channel.addEventListener("error", this.onClose);
  }
  private publish(patch: Partial<TransferProgress>) { this.progress = { ...this.progress, ...patch }; this.update(this.progress); }
  private rejectPending(message: string) {
    const pending = this.pending; this.pending = null;
    if (pending) { clearTimeout(pending.timer); pending.reject(new Error(message)); }
  }
  private onClose = () => { this.rejectPending("File channel disconnected"); };
  private onMessage = (event: MessageEvent) => {
    if (typeof event.data !== "string" || event.data.length > 4096) return;
    let message: Record<string, unknown>;
    try { message = JSON.parse(event.data); } catch { return; }
    if (!message || message.session_id !== this.sessionId || message.transfer_id !== this.id || !this.pending) return;
    if (message.type === "file_error") { this.rejectPending(typeof message.message === "string" ? message.message.slice(0, 300) : "Transfer rejected"); return; }
    const pending = this.pending;
    if (message.type !== pending.type || (pending.offset !== undefined && message.offset !== pending.offset)) return;
    this.pending = null; clearTimeout(pending.timer); pending.resolve();
  };
  private exchange(message: Record<string, unknown>, type: string, offset?: number): Promise<void> {
    if (this.cancelled) return Promise.reject(new Error("Transfer cancelled"));
    if (this.channel.readyState !== "open") return Promise.reject(new Error("File channel disconnected"));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.rejectPending("Transfer timed out"), type === "file_ready" ? 70_000 : 30_000);
      this.pending = { type, offset, resolve, reject, timer };
      try { this.channel.send(JSON.stringify({ ...message, session_id: this.sessionId, transfer_id: this.id })); }
      catch { this.rejectPending("Could not send file data"); }
    });
  }
  async send(file: File): Promise<void> {
    if (this.id) throw new Error("A transfer is already active");
    if (file.size > MAX_FILE_BYTES) throw new Error("Files are limited to 32 MiB");
    if (!globalThis.crypto?.getRandomValues) throw new Error("Secure random numbers are unavailable in this browser");
    const random = crypto.getRandomValues(new Uint8Array(16));
    random[6] = (random[6] & 0x0f) | 0x40;
    random[8] = (random[8] & 0x3f) | 0x80;
    const hex = Array.from(random, value => value.toString(16).padStart(2, "0")).join("");
    this.id = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    this.cancelled = false;
    this.publish({ status: "hashing", name: file.name, sent: 0, total: file.size, message: undefined });
    try {
      const buffer = await file.arrayBuffer();
      const digest = sha256(new Uint8Array(buffer));
      if (this.cancelled) throw new Error("Transfer cancelled");
      const digestHex = Array.from(digest, value => value.toString(16).padStart(2, "0")).join("");
      this.publish({ status: "awaiting" });
      await this.exchange({ type: "file_offer", name: file.name, size: file.size, sha256: digestHex }, "file_ready", 0);
      this.publish({ status: "sending" });
      for (let offset = 0; offset < buffer.byteLength; offset += CHUNK_BYTES) {
        const chunk = new Uint8Array(buffer, offset, Math.min(CHUNK_BYTES, buffer.byteLength - offset));
        let raw = "";
        for (const byte of chunk) raw += String.fromCharCode(byte);
        const next = offset + chunk.byteLength;
        await this.exchange({ type: "file_chunk", offset, data: btoa(raw) }, "file_ack", next);
        this.publish({ sent: next });
      }
      this.publish({ status: "verifying" });
      await this.exchange({ type: "file_finish" }, "file_complete");
      this.publish({ status: "complete", message: "Saved in the remote Downloads / 22Pie Transfers folder." });
    } catch (error) {
      if (!this.cancelled) this.sendCancel();
      this.publish({ status: this.cancelled ? "cancelled" : "failed", message: error instanceof Error ? error.message : "Transfer failed" });
    } finally { this.id = null; }
  }
  private sendCancel() {
    if (this.id && this.channel.readyState === "open") {
      try { this.channel.send(JSON.stringify({ type: "file_cancel", session_id: this.sessionId, transfer_id: this.id })); } catch { /* The channel may close between the check and send. */ }
    }
  }
  cancel() { this.cancelled = true; this.sendCancel(); this.rejectPending("Transfer cancelled"); }
  dispose() {
    this.cancel();
    this.channel.removeEventListener("message", this.onMessage);
    this.channel.removeEventListener("close", this.onClose);
    this.channel.removeEventListener("error", this.onClose);
  }
}
