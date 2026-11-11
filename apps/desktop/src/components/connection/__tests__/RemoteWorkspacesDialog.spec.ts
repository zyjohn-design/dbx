// @vitest-environment happy-dom
import { createApp, nextTick } from "vue";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import RemoteWorkspacesDialog from "../RemoteWorkspacesDialog.vue";
import en from "@/i18n/locales/en";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const cleanup: Array<() => void> = [];
const server = { id: "123e4567-e89b-12d3-a456-426614174000", name: "Production", url: "https://dbx.example.com/dbx/" };

async function flush() {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

async function mount() {
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(RemoteWorkspacesDialog, { open: true });
  app.use(createI18n({ legacy: false, locale: "en", messages: { en } }));
  app.mount(host);
  cleanup.push(() => {
    app.unmount();
    host.remove();
  });
  await flush();
}

function button(text: string) {
  const result = [...document.querySelectorAll("button")].find((element) => element.textContent?.trim() === text || element.getAttribute("aria-label") === text);
  if (!result) throw new Error(`Button not found: ${text}`);
  return result;
}

beforeEach(() => {
  vi.resetAllMocks();
  invoke.mockImplementation(async (command: string) => (command === "load_remote_workspaces" ? [] : undefined));
});
afterEach(() => {
  for (const dispose of cleanup.splice(0)) dispose();
});

describe("remote server management", () => {
  it("loads the list on first opening and opens the chosen server", async () => {
    invoke.mockResolvedValue([server]);
    await mount();
    expect(document.body.textContent).toContain("Production");
    expect(invoke).toHaveBeenCalledWith("load_remote_workspaces");
    button("Open").click();
    await flush();
    expect(invoke).toHaveBeenCalledWith("open_remote_workspace", { id: server.id });
  });

  it("tests a normalized address before saving the new server", async () => {
    await mount();
    button("Add server").click();
    await flush();
    const inputs = document.querySelectorAll("input");
    inputs[0].value = "Staging";
    inputs[0].dispatchEvent(new Event("input", { bubbles: true }));
    inputs[1].value = "https://staging.example.com/dbx";
    inputs[1].dispatchEvent(new Event("input", { bubbles: true }));
    await flush();
    invoke.mockImplementation(async (command: string) => (command === "save_remote_workspace" ? [{ ...server, name: "Staging" }] : {}));
    document.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await flush();
    const testIndex = invoke.mock.calls.findIndex(([command]) => command === "test_remote_workspace");
    const saveIndex = invoke.mock.calls.findIndex(([command]) => command === "save_remote_workspace");
    expect(testIndex).toBeGreaterThan(-1);
    expect(saveIndex).toBeGreaterThan(testIndex);
    expect(invoke.mock.calls[saveIndex][1].workspace).toMatchObject({ name: "Staging", url: "https://staging.example.com/dbx/" });
    expect(document.querySelector("form")).toBeNull();
    expect(document.body.textContent).toContain("Staging");
  });

  it("keeps an incompatible server unsaved and explains the required update", async () => {
    invoke.mockResolvedValue([server]);
    await mount();
    button("Edit server").click();
    await flush();
    invoke.mockRejectedValue("SERVER_UPGRADE_REQUIRED");
    document.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await flush();
    expect(document.querySelector('[role="alert"]')?.textContent).toContain("Update the server");
    expect(invoke.mock.calls.some(([command]) => command === "save_remote_workspace")).toBe(false);
    expect(document.querySelector("form")).not.toBeNull();
  });

  it("removes only after the user confirms and refreshes the list", async () => {
    invoke.mockResolvedValueOnce([server]);
    await mount();
    button("Remove").click();
    await flush();
    expect(invoke).not.toHaveBeenCalledWith("remove_remote_workspace", expect.anything());
    expect(document.body.textContent).toContain("The server and its connections will be kept");
    invoke.mockResolvedValueOnce([]);
    button("Remove").click();
    await flush();
    expect(invoke).toHaveBeenCalledWith("remove_remote_workspace", { id: server.id });
    expect(document.body.textContent).toContain("No remote servers yet");
  });
});
