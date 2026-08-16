const REQUIRED_PERMISSIONS = ["SCREEN_VIEW", "MOUSE_CONTROL", "KEYBOARD_CONTROL"] as const;

export function hasCompleteRemoteControl(permissions: readonly string[]): boolean {
  return REQUIRED_PERMISSIONS.every((permission) => permissions.includes(permission));
}
