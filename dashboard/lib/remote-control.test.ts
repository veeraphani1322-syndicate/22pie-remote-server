import assert from "node:assert/strict";
import test from "node:test";
import { hasCompleteRemoteControl } from "./remote-control";

test("normal viewer control requires the complete bundled permission set", () => {
  assert.equal(hasCompleteRemoteControl(["SCREEN_VIEW"]), false);
  assert.equal(hasCompleteRemoteControl(["SCREEN_VIEW", "MOUSE_CONTROL"]), false);
  assert.equal(hasCompleteRemoteControl(["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"]), true);
});
