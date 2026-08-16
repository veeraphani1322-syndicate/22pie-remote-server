export interface KeyboardModifiers { alt: boolean; ctrl: boolean; meta: boolean; shift: boolean }

export function modifiersOf(event: Pick<KeyboardEvent, "altKey" | "ctrlKey" | "metaKey" | "shiftKey">): KeyboardModifiers {
  return { alt: event.altKey, ctrl: event.ctrlKey, meta: event.metaKey, shift: event.shiftKey };
}

export function isLocalKeyboardRelease(event: Pick<KeyboardEvent, "code" | "altKey" | "ctrlKey">): boolean {
  return event.code === "Escape" && event.altKey && event.ctrlKey;
}

export function shouldSendText(event: Pick<KeyboardEvent, "key" | "altKey" | "ctrlKey" | "metaKey">): boolean {
  return !event.altKey && !event.ctrlKey && !event.metaKey && [...event.key].length === 1;
}
