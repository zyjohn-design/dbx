# Desktop remote workspaces / 桌面远程工作区

The desktop client can save multiple DBX Web deployments and open each in a
separate native window. The server owns the connection profiles, credentials,
saved SQL, and database sessions. Queries run on that server, so the desktop
machine does not need direct network access to its databases.

## 使用方法

1. 使用包含本功能的源码重新构建桌面客户端，以及服务端的 Web 前端和
   `dbx-web` 后端。仅更新桌面客户端，或继续使用尚未包含本功能的旧版
   Docker 镜像，都无法启用远程工作区。
2. 在桌面客户端左侧连接工具栏点击服务器图标，打开“远程服务器”。
3. 点击“添加服务器”，填写名称和部署地址，例如
   `https://dbx.example.com/` 或 `http://192.168.1.10:4224/dbx/`。
   地址应包含反向代理路径前缀，不包含 `/login`、账号密码、查询参数或片段。
4. 测试连接并保存，然后点击“打开”。在独立窗口中使用服务端原有的登录页
   登录，需要验证码时同样在该窗口完成。
5. 在该窗口查看、编辑服务器的连接配置并执行查询。网页端的配置更新可通过
   现有刷新操作读取；它们始终保存在同一服务器上，无需导出或复制到本机。

可同时打开多个服务器。再次打开同一服务器会聚焦已有窗口。关闭对应工作区后，
可以编辑或从本机移除服务器记录；移除不会删除服务端数据。

## 会话与文件

- 本机持久化的 `remote-workspaces.json` 位于应用配置目录，只保存服务器
  ID、名称和地址。数据库密码和服务器登录密码不写入这个文件。
- 远程窗口使用独立的临时浏览会话，关闭后重新打开需要重新登录。
  Windows / Linux 另外使用每个服务器独立的 WebView 数据目录，避免不同部署
  因相同主机名或 Cookie 名称而串用登录状态。
- 文件导入导出沿用 Web 端的文件选择、上传和下载。下载保存到系统“下载”目录，
  同名文件自动更名，不覆盖已有文件。服务端文件路径、驱动、
  SSH 隧道和备份目录均属于服务端环境。
- 不支持脱机使用远程连接，也不在本地与远程之间自动合并配置。
- 当前窗口限制在所选部署的同源路径范围内，外部弹窗不会打开。前置 SSO
  重定向到其他域名的部署不在这一版的支持范围内。

## Implementation

`src-tauri/src/commands/remote_workspaces.rs` validates and atomically persists
server metadata, probes the deployment, and creates its native window. The
public `/api/auth/check` response advertises `desktop_workspace_supported` so
older servers fail with an upgrade message before a window opens.

Each remote window receives `__DBX_REMOTE_WORKSPACE__` before application
scripts run. The frontend runtime resolver consequently chooses HTTP even
though it is hosted by Tauri. Requests, cookies, SSE, WebSockets, uploads, and
downloads remain on the deployment's own origin and base path. The client uses
the server's frontend assets, keeping its UI and API versions paired.

Remote window labels receive no local Tauri capabilities. The central custom
command dispatcher also rejects every invoke from those labels, independently
of the frontend marker. Remote windows close normally rather than entering the
local workbench's hide-to-tray flow.

## Verification

```sh
pnpm exec vitest run \
  apps/desktop/src/components/connection/__tests__/RemoteWorkspacesDialog.spec.ts \
  apps/desktop/src/lib/app/__tests__/remoteWorkspaces.spec.ts \
  apps/desktop/src/lib/backend/__tests__/remoteWorkspaceHttp.spec.ts

NODE_OPTIONS=--max-old-space-size=8192 pnpm typecheck
pnpm build
cargo test -p dbx --lib commands::remote_workspaces::tests
cargo test -p dbx --lib migration_gate::tests
```

Before release, exercise a real deployment on each supported desktop platform:
login and captcha, two servers, connection refresh, SQL execution, a streaming
operation, file upload/download, closing with unsaved SQL, and session isolation
after closing and reopening. Also verify that an older server shows the upgrade
message and that a server behind a path prefix keeps requests under that prefix.
