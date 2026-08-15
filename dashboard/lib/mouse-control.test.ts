import assert from "node:assert/strict";
import test from "node:test";
import { createMoveCoalescer, normalizedVideoPoint } from "./mouse-control.js";

test("maps exact and letterboxed video coordinates", () => {
  assert.deepEqual(normalizedVideoPoint(500, 250, { left: 0, top: 0, width: 1000, height: 500 }, 1920, 960), { x: 0.5, y: 0.5 });
  assert.deepEqual(normalizedVideoPoint(500, 500, { left: 0, top: 0, width: 1000, height: 1000 }, 1920, 1080), { x: 0.5, y: 0.5 });
  assert.equal(normalizedVideoPoint(500, 100, { left: 0, top: 0, width: 1000, height: 1000 }, 1920, 1080), null);
  assert.deepEqual(normalizedVideoPoint(500, 250, { left: 0, top: 0, width: 1000, height: 500 }, 1000, 1000), { x: 0.5, y: 0.5 });
});

test("coalesces moves to the latest point", () => {
  const callbacks: Array<() => void> = []; const sent: unknown[] = [];
  const move = createMoveCoalescer((point) => sent.push(point), (callback) => { callbacks.push(callback); return 1; });
  move({ x: 0.1, y: 0.1 }); move({ x: 0.9, y: 0.8 });
  assert.equal(callbacks.length, 1); callbacks[0]();
  assert.deepEqual(sent, [{ x: 0.9, y: 0.8 }]);
});
