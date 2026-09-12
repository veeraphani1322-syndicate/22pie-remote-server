import assert from "node:assert/strict";
import { test } from "node:test";
import { createHash } from "node:crypto";
import { FileSender, MAX_FILE_BYTES, type TransferProgress } from "./file-transfer";

class Channel extends EventTarget {
  readyState = "open";
  messages: Record<string, unknown>[] = [];
  handle: (message: Record<string, unknown>) => void = () => {};
  send(data: string) { const message = JSON.parse(data); this.messages.push(message); this.handle(message); }
  reply(message: Record<string, unknown>, type: string, extra = {}) {
    queueMicrotask(() => this.dispatchEvent(new MessageEvent("message", { data: JSON.stringify({ session_id: message.session_id, transfer_id: message.transfer_id, type, ...extra }) })));
  }
  close() { this.readyState = "closed"; this.dispatchEvent(new Event("close")); }
}
function sender(channel: Channel) {
  const progress: TransferProgress[] = [];
  const instance = new FileSender(channel as unknown as RTCDataChannel, "session-one", value => progress.push(value));
  return { instance, progress };
}

test("sends bounded chunks only after acceptance and waits for confirmed completion", async () => {
  const channel = new Channel(); const { instance, progress } = sender(channel);
  const bytes = new Uint8Array(40_000).map((_, i) => i % 251);
  const received: Buffer[] = []; let offset = 0;
  channel.handle = message => {
    if (message.type === "file_offer") {
      assert.equal(message.sha256, createHash("sha256").update(bytes).digest("hex"));
      channel.reply(message, "file_ready", { offset: 0 });
    } else if (message.type === "file_chunk") {
      const chunk = Buffer.from(message.data as string, "base64");
      assert.ok(chunk.length <= 16 * 1024); assert.equal(message.offset, offset);
      received.push(chunk); offset += chunk.length;
      channel.reply(message, "file_ack", { offset });
    } else if (message.type === "file_finish") {
      assert.equal(progress.at(-1)?.status, "verifying");
      channel.reply(message, "file_complete");
    }
  };
  await instance.send(new File([bytes], "sample.bin"));
  assert.deepEqual(Buffer.concat(received), Buffer.from(bytes));
  assert.equal(progress.at(-1)?.status, "complete"); instance.dispose();
});

test("denial sends no file contents", async () => {
  const channel = new Channel(); const { instance, progress } = sender(channel);
  channel.handle = message => { if (message.type === "file_offer") channel.reply(message, "file_error", { message: "Declined" }); };
  await instance.send(new File(["private"], "private.txt"));
  assert.equal(progress.at(-1)?.status, "failed");
  assert.ok(!channel.messages.some(m => m.type === "file_chunk")); instance.dispose();
});

test("cancellation while awaiting approval terminates without sending contents", async () => {
  const channel = new Channel(); const { instance, progress } = sender(channel);
  channel.handle = message => { if (message.type === "file_offer") queueMicrotask(() => instance.cancel()); };
  await instance.send(new File(["private"], "private.txt"));
  assert.equal(progress.at(-1)?.status, "cancelled");
  assert.ok(!channel.messages.some(m => m.type === "file_chunk")); instance.dispose();
});

test("stale-session acceptance is ignored and a disconnect fails the transfer", async () => {
  const channel = new Channel(); const { instance, progress } = sender(channel);
  channel.handle = message => {
    if (message.type === "file_offer") {
      channel.reply({ ...message, session_id: "old-session" }, "file_ready", { offset: 0 });
      queueMicrotask(() => channel.close());
    }
  };
  await instance.send(new File(["private"], "private.txt"));
  assert.equal(progress.at(-1)?.status, "failed");
  assert.ok(!channel.messages.some(m => m.type === "file_chunk")); instance.dispose();
});

test("oversized files are rejected before reading or sending", async () => {
  const channel = new Channel(); const { instance } = sender(channel);
  await assert.rejects(instance.send({ size: MAX_FILE_BYTES + 1 } as File), /32 MiB/);
  assert.equal(channel.messages.length, 0); instance.dispose();
});


test("uploads work on the existing HTTP dashboard without subtle or randomUUID", async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "crypto");
  const getRandomValues = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
  Object.defineProperty(globalThis, "crypto", { configurable: true, value: { getRandomValues } });
  const channel = new Channel(); const { instance, progress } = sender(channel);
  channel.handle = message => {
    if (message.type === "file_offer") {
      assert.match(message.transfer_id as string, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
      channel.reply(message, "file_ready", { offset: 0 });
    } else if (message.type === "file_finish") channel.reply(message, "file_complete");
  };
  try {
    await instance.send(new File([], "empty.txt"));
    assert.equal(progress.at(-1)?.status, "complete");
  } finally {
    instance.dispose();
    if (descriptor) Object.defineProperty(globalThis, "crypto", descriptor);
  }
});
