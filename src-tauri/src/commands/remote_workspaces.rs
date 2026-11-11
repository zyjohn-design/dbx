//! Remote pages run as Web clients in unprivileged, isolated native windows.
//! Only the local shell can manage these records; passwords never enter this file.
use std::{fs, path::PathBuf, sync::Mutex, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::Manager;

const WINDOW_PREFIX: &str = "remote-workspace-";
const MAX_WORKSPACES: usize = 100;
static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoteWorkspace {
    pub id: String,
    pub name: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteWorkspaceStatus {
    pub required: bool,
    pub setup_required: bool,
}

pub fn is_remote_workspace_window(label: &str) -> bool {
    label.starts_with(WINDOW_PREFIX)
}

fn normalize_url(value: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(value.trim()).map_err(|_| "INVALID_URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("INVALID_URL".into());
    }
    let path = format!("{}/", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn validate_workspace(mut workspace: RemoteWorkspace) -> Result<RemoteWorkspace, String> {
    workspace.id = uuid::Uuid::parse_str(&workspace.id).map_err(|_| "WORKSPACE_NOT_FOUND")?.to_string();
    workspace.name = workspace.name.trim().to_string();
    if workspace.name.is_empty() || workspace.name.chars().count() > 80 || workspace.name.chars().any(char::is_control)
    {
        return Err("INVALID_NAME".into());
    }
    workspace.url = normalize_url(&workspace.url)?.to_string();
    Ok(workspace)
}

fn store_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("remote-workspaces.json"))
}

fn read_store(path: &std::path::Path) -> Result<Vec<RemoteWorkspace>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let records: Vec<RemoteWorkspace> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if records.len() > MAX_WORKSPACES {
        return Err("FAILED".into());
    }
    records.into_iter().map(validate_workspace).collect()
}

fn write_store(path: &std::path::Path, records: &[RemoteWorkspace]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("FAILED")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, records).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

fn download_destination(directory: &std::path::Path, suggested: &std::path::Path) -> PathBuf {
    let filename =
        suggested.file_name().filter(|name| !name.is_empty()).unwrap_or_else(|| std::ffi::OsStr::new("dbx-download"));
    let mut destination = directory.join(filename);
    if destination.exists() {
        let suffix = uuid::Uuid::new_v4();
        let stem = destination.file_stem().unwrap_or(filename).to_string_lossy();
        let extension =
            destination.extension().map(|extension| format!(".{}", extension.to_string_lossy())).unwrap_or_default();
        destination = directory.join(format!("{stem}-{suffix}{extension}"));
    }
    destination
}

#[tauri::command]
pub fn load_remote_workspaces(app: tauri::AppHandle) -> Result<Vec<RemoteWorkspace>, String> {
    let _lock = STORE_LOCK.lock().map_err(|_| "FAILED")?;
    read_store(&store_path(&app)?)
}

#[tauri::command]
pub fn save_remote_workspace(
    app: tauri::AppHandle,
    workspace: RemoteWorkspace,
) -> Result<Vec<RemoteWorkspace>, String> {
    let workspace = validate_workspace(workspace)?;
    let _lock = STORE_LOCK.lock().map_err(|_| "FAILED")?;
    let path = store_path(&app)?;
    let mut records = read_store(&path)?;
    if records.iter().any(|record| record.id != workspace.id && record.url == workspace.url) {
        return Err("DUPLICATE_URL".into());
    }
    if app.get_webview_window(&format!("{WINDOW_PREFIX}{}", workspace.id)).is_some() {
        return Err("WORKSPACE_OPEN".into());
    }
    if let Some(record) = records.iter_mut().find(|record| record.id == workspace.id) {
        *record = workspace;
    } else {
        if records.len() >= MAX_WORKSPACES {
            return Err("FAILED".into());
        }
        records.push(workspace);
    }
    write_store(&path, &records)?;
    Ok(records)
}

#[tauri::command]
pub fn remove_remote_workspace(app: tauri::AppHandle, id: String) -> Result<Vec<RemoteWorkspace>, String> {
    let _lock = STORE_LOCK.lock().map_err(|_| "FAILED")?;
    if app.get_webview_window(&format!("{WINDOW_PREFIX}{id}")).is_some() {
        return Err("WORKSPACE_OPEN".into());
    }
    let path = store_path(&app)?;
    let mut records = read_store(&path)?;
    records.retain(|record| record.id != id);
    write_store(&path, &records)?;
    Ok(records)
}

#[derive(Deserialize)]
struct AuthCheck {
    required: bool,
    authenticated: bool,
    setup_required: bool,
    #[serde(default)]
    desktop_workspace_supported: bool,
}

async fn probe_server(url: &reqwest::Url) -> Result<RemoteWorkspaceStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "SERVER_UNREACHABLE")?;
    let endpoint = url.join("api/auth/check").map_err(|_| "INVALID_URL")?;
    let mut response = client.get(endpoint).send().await.map_err(|_| "SERVER_UNREACHABLE")?;
    if !response.status().is_success() {
        return Err("INVALID_SERVER".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "SERVER_UNREACHABLE")? {
        if bytes.len() + chunk.len() > 64 * 1024 {
            return Err("INVALID_SERVER".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let status: AuthCheck = serde_json::from_slice(&bytes).map_err(|_| "INVALID_SERVER")?;
    if !status.desktop_workspace_supported {
        return Err("SERVER_UPGRADE_REQUIRED".into());
    }
    // Deserializing all three fields also distinguishes DBX from unrelated JSON endpoints.
    let _authenticated = status.authenticated;
    Ok(RemoteWorkspaceStatus { required: status.required, setup_required: status.setup_required })
}

#[tauri::command]
pub async fn test_remote_workspace(url: String) -> Result<RemoteWorkspaceStatus, String> {
    probe_server(&normalize_url(&url)?).await
}

#[tauri::command]
pub async fn open_remote_workspace(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let workspace =
        load_remote_workspaces(app.clone())?.into_iter().find(|record| record.id == id).ok_or("WORKSPACE_NOT_FOUND")?;
    let label = format!("{WINDOW_PREFIX}{id}");
    if let Some(window) = app.get_webview_window(&label) {
        window.show().map_err(|e| e.to_string())?;
        return window.set_focus().map_err(|e| e.to_string());
    }
    let url = normalize_url(&workspace.url)?;
    probe_server(&url).await?;
    let origin = url.origin();
    let base_path = url.path().to_string();
    let downloads = app.path().download_dir().map_err(|e| e.to_string())?;
    let builder = tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::External(url));
    // WebView2 InPrivate profiles share cookies when they share a user-data
    // directory. Give each saved server its own profile, including on old
    // WebView2 runtimes where incognito mode is unavailable.
    #[cfg(not(target_os = "macos"))]
    let builder = builder
        .data_directory(app.path().app_cache_dir().map_err(|e| e.to_string())?.join("remote-workspaces").join(&id));
    builder
        .title(format!("{} — DBX Remote", workspace.name))
        .inner_size(1280.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .incognito(true)
        .disable_drag_drop_handler()
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        // WKWebView cancels HTML downloads unless a handler is registered.
        // Keep file names inside Downloads and never replace an existing file.
        .on_download(move |_, event| {
            if let tauri::webview::DownloadEvent::Requested { destination, .. } = event {
                if fs::create_dir_all(&downloads).is_err() {
                    return false;
                }
                *destination = download_destination(&downloads, destination);
            }
            true
        })
        .initialization_script("Object.defineProperty(window, '__DBX_REMOTE_WORKSPACE__', { value: true });")
        .on_navigation(move |target| {
            target.origin() == origin && (target.scheme() == "blob" || target.path().starts_with(&base_path))
        })
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_server_urls_and_keeps_reverse_proxy_prefixes() {
        assert_eq!(normalize_url(" https://example.com/dbx ").unwrap().as_str(), "https://example.com/dbx/");
        assert_eq!(normalize_url("http://127.0.0.1:4224").unwrap().as_str(), "http://127.0.0.1:4224/");
        for url in [
            "file:///tmp/dbx",
            "javascript:alert(1)",
            "https://user:secret@example.com",
            "https://example.com/?token=secret",
            "https://example.com/#fragment",
            "example.com",
        ] {
            assert!(normalize_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn metadata_round_trips_atomically_and_corruption_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("workspaces.json");
        assert!(read_store(&path).unwrap().is_empty());
        let record = RemoteWorkspace {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Production".into(),
            url: "https://example.com/dbx/".into(),
        };
        write_store(&path, &[record]).unwrap();
        assert_eq!(read_store(&path).unwrap()[0].name, "Production");
        write_store(&path, &[]).unwrap();
        assert!(read_store(&path).unwrap().is_empty());
        fs::write(&path, b"corrupted").unwrap();
        assert!(read_store(&path).is_err());
    }

    #[test]
    fn downloads_stay_in_downloads_and_do_not_replace_existing_files() {
        let directory = tempfile::tempdir().unwrap();
        let suggested = std::path::Path::new("/another/directory/results.csv");
        let first = download_destination(directory.path(), suggested);
        assert_eq!(first, directory.path().join("results.csv"));
        fs::write(&first, b"previous export").unwrap();
        let next = download_destination(directory.path(), suggested);
        assert_eq!(next.parent(), Some(directory.path()));
        assert_eq!(next.extension().unwrap(), "csv");
        assert_ne!(first, next);
        assert_eq!(fs::read(first).unwrap(), b"previous export");
    }

    #[tokio::test]
    async fn probes_the_server_and_requires_compatible_frontend() {
        use axum::{routing::get, Json, Router};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let router = Router::new()
            .route("/dbx/api/auth/check", get(|| async { Json(serde_json::json!({ "required": true, "authenticated": false, "setup_required": false, "desktop_workspace_supported": true })) }))
            .route("/legacy/api/auth/check", get(|| async { Json(serde_json::json!({ "required": true, "authenticated": false, "setup_required": false })) }));
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        assert!(probe_server(&normalize_url(&format!("{base}dbx")).unwrap()).await.unwrap().required);
        assert_eq!(
            probe_server(&normalize_url(&format!("{base}legacy")).unwrap()).await.unwrap_err(),
            "SERVER_UPGRADE_REQUIRED"
        );
        assert_eq!(
            probe_server(&normalize_url(&format!("{base}missing")).unwrap()).await.unwrap_err(),
            "INVALID_SERVER"
        );
        task.abort();
    }
}
