import assert from "node:assert/strict";
import test from "node:test";
import { isLocalKeyboardRelease, shouldSendText } from "./keyboard-control";

test("only Ctrl+Alt+Escape is the local release chord", () => {
  assert.equal(isLocalKeyboardRelease({ code: "Escape", ctrlKey: true, altKey: true }), true);
  assert.equal(isLocalKeyboardRelease({ code: "Escape", ctrlKey: false, altKey: false }), false);
});

test("printable input uses Unicode while shortcuts use physical keys", () => {
  assert.equal(shouldSendText({ key: "é", ctrlKey: false, altKey: false, metaKey: false }), true);
  assert.equal(shouldSendText({ key: "c", ctrlKey: true, altKey: false, metaKey: false }), false);
  assert.equal(shouldSendText({ key: "Escape", ctrlKey: false, altKey: false, metaKey: false }), false);
});
