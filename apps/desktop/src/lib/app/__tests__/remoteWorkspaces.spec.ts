import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { isTauriRuntime } from "@/lib/backend/tauriRuntime";
import { loadRemoteWorkspaces, normalizeRemoteWorkspaceUrl, openRemoteWorkspace, remoteWorkspaceErrorCode, removeRemoteWorkspace, saveRemoteWorkspace, testRemoteWorkspace } from "../remoteWorkspaces";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

beforeEach(() => vi.resetAllMocks());
afterEach(() => vi.unstubAllGlobals());

describe("remote workspaces", () => {
  it.each([
    [" https://dbx.example.com ", "https://dbx.example.com/"],
    ["http://127.0.0.1:4224/dbx///", "http://127.0.0.1:4224/dbx/"],
    ["http://[::1]:4224", "http://[::1]:4224/"],
  ])("normalizes %s without losing a reverse-proxy path", (input, expected) => {
    expect(normalizeRemoteWorkspaceUrl(input)).toBe(expected);
  });

  it.each(["dbx.example.com", "file:///etc/passwd", "javascript:alert(1)", "https://user:secret@dbx.example.com", "https://dbx.example.com/?password=secret", "https://dbx.example.com/#token"])("rejects an invalid server address: %s", (url) => {
    expect(() => normalizeRemoteWorkspaceUrl(url)).toThrow("INVALID_URL");
  });

  it("persists only server metadata and opens by saved identity", async () => {
    const workspace = { id: "server-1", name: " Production ", url: "https://dbx.example.com/dbx" };
    invoke.mockResolvedValue([workspace]);
    await loadRemoteWorkspaces();
    await saveRemoteWorkspace(workspace);
    await testRemoteWorkspace(workspace.url);
    await openRemoteWorkspace(workspace.id);
    await removeRemoteWorkspace(workspace.id);
    expect(invoke.mock.calls).toEqual([
      ["load_remote_workspaces"],
      ["save_remote_workspace", { workspace: { ...workspace, name: "Production", url: "https://dbx.example.com/dbx/" } }],
      ["test_remote_workspace", { url: "https://dbx.example.com/dbx/" }],
      ["open_remote_workspace", { id: "server-1" }],
      ["remove_remote_workspace", { id: "server-1" }],
    ]);
  });

  it("selects the HTTP runtime inside remote windows even with Tauri injected", () => {
    expect(isTauriRuntime({ __TAURI_INTERNALS__: {} })).toBe(true);
    expect(isTauriRuntime({ __TAURI_INTERNALS__: {}, __DBX_REMOTE_WORKSPACE__: true })).toBe(false);
    expect(isTauriRuntime({})).toBe(false);
  });

  it("shows a safe localized fallback for unexpected native errors", () => {
    expect(remoteWorkspaceErrorCode(new Error("SERVER_UPGRADE_REQUIRED"))).toBe("SERVER_UPGRADE_REQUIRED");
    expect(remoteWorkspaceErrorCode("WORKSPACE_OPEN")).toBe("WORKSPACE_OPEN");
    expect(remoteWorkspaceErrorCode("an internal filesystem path")).toBe("FAILED");
  });
});
