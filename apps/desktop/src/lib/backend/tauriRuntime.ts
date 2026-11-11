export function isTauriRuntime(globalObject: Record<string, unknown> = globalThis as Record<string, unknown>): boolean {
  // An external workspace has a Tauri host but must use its own HTTP backend.
  if (globalObject.__DBX_REMOTE_WORKSPACE__ === true) return false;
  return Boolean(globalObject.__TAURI_INTERNALS__ || globalObject.__TAURI__);
}
