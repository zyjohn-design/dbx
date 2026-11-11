import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({ loadConnections: vi.fn(), executeQuery: vi.fn() }));
vi.mock("@/lib/backend/tauri", () => native);
vi.mock("@/lib/backend/debugLog", () => ({ appendDebugLog: vi.fn() }));

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
});
afterEach(() => vi.unstubAllGlobals());

describe("remote workspace backend routing", () => {
  it("loads server connections and runs queries through that server's HTTP API", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    vi.stubGlobal("__DBX_REMOTE_WORKSPACE__", true);
    vi.stubGlobal("location", { pathname: "/dbx/", protocol: "https:", host: "remote.example.com" });
    const connections = [{ id: "remote-db", name: "Production" }];
    const result = { columns: ["value"], rows: [[1]] };
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify(connections)))
      .mockResolvedValueOnce(new Response(JSON.stringify(result)));
    vi.stubGlobal("fetch", fetch);
    const api = await import("../api");
    expect(await api.loadConnections()).toEqual(connections);
    expect(await api.executeQuery("remote-db", "app", "select 1")).toEqual(result);
    expect(fetch.mock.calls[0][0]).toBe("/dbx/api/connection/list");
    expect(fetch.mock.calls[1][0]).toBe("/dbx/api/query/execute");
    expect(JSON.parse(fetch.mock.calls[1][1].body)).toEqual({ connectionId: "remote-db", database: "app", sql: "select 1" });
    expect(native.loadConnections).not.toHaveBeenCalled();
    expect(native.executeQuery).not.toHaveBeenCalled();
  });

  it("keeps the local workspace on the native backend", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    native.loadConnections.mockResolvedValue([{ id: "local-db" }]);
    const api = await import("../api");
    expect(await api.loadConnections()).toEqual([{ id: "local-db" }]);
    expect(native.loadConnections).toHaveBeenCalledOnce();
  });
});
