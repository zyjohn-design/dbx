import { invoke } from "@tauri-apps/api/core";

export interface RemoteWorkspace {
  id: string;
  name: string;
  url: string;
}

export interface RemoteWorkspaceStatus {
  required: boolean;
  setupRequired: boolean;
}

export function normalizeRemoteWorkspaceUrl(value: string): string {
  let url: URL;
  try {
    url = new URL(value.trim());
  } catch {
    throw new Error("INVALID_URL");
  }
  if (!["http:", "https:"].includes(url.protocol) || !url.hostname || url.username || url.password || url.search || url.hash) {
    throw new Error("INVALID_URL");
  }
  url.pathname = `${url.pathname.replace(/\/+$/, "")}/`;
  return url.href;
}

export function loadRemoteWorkspaces(): Promise<RemoteWorkspace[]> {
  return invoke("load_remote_workspaces");
}

export function saveRemoteWorkspace(workspace: RemoteWorkspace): Promise<RemoteWorkspace[]> {
  return invoke("save_remote_workspace", { workspace: { ...workspace, name: workspace.name.trim(), url: normalizeRemoteWorkspaceUrl(workspace.url) } });
}

export function removeRemoteWorkspace(id: string): Promise<RemoteWorkspace[]> {
  return invoke("remove_remote_workspace", { id });
}

export function testRemoteWorkspace(url: string): Promise<RemoteWorkspaceStatus> {
  return invoke("test_remote_workspace", { url: normalizeRemoteWorkspaceUrl(url) });
}

export function openRemoteWorkspace(id: string): Promise<void> {
  return invoke("open_remote_workspace", { id });
}

export function remoteWorkspaceErrorCode(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return ["INVALID_URL", "INVALID_NAME", "DUPLICATE_URL", "WORKSPACE_OPEN", "SERVER_UNREACHABLE", "INVALID_SERVER", "SERVER_UPGRADE_REQUIRED", "WORKSPACE_NOT_FOUND"].includes(message) ? message : "FAILED";
}
