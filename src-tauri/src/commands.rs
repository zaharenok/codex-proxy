use crate::config::{AppState, Settings};
use crate::proxy::{get_presets, ProxyState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartProxyArgs {
    pub preset: String,
    pub upstream: String,
    pub api_key: String,
    pub port: u16,
    pub selected_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateKeyArgs {
    pub preset: String,
    pub api_key: String,
    pub upstream: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchModelsArgs {
    pub preset: String,
    pub api_key: String,
    pub upstream: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KeyValidationResult {
    pub valid: bool,
    pub status: Option<u16>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelsResult {
    pub models: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProxyResult {
    pub ok: bool,
    pub error: Option<String>,
    pub upstream: Option<String>,
}

fn models_base(preset: &str, upstream: &str) -> Option<String> {
    let presets = get_presets();
    if let Some(p) = presets.get(preset) {
        return Some(p.models_url.unwrap_or(p.url).trim_end_matches('/').to_string());
    }
    if !upstream.is_empty() {
        return Some(upstream.trim_end_matches('/').to_string());
    }
    None
}

fn appstate() -> &'static AppState {
    AppState::global()
}

fn proxystate() -> &'static ProxyState {
    ProxyState::global()
}

#[tauri::command]
pub async fn ping() -> Result<String, String> {
    Ok("pong".to_string())
}

#[tauri::command]
pub async fn load_settings() -> Result<Settings, String> {
    Ok(appstate().config.load_settings())
}

#[tauri::command]
pub async fn save_settings(settings: Settings) -> Result<bool, String> {
    appstate().config.save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn validate_key(args: ValidateKeyArgs) -> Result<KeyValidationResult, String> {
    let base = models_base(&args.preset, &args.upstream.unwrap_or_default());
    let base = match base {
        Some(b) => b,
        None => return Ok(KeyValidationResult {
            valid: false,
            status: None,
            error: Some("Unknown preset".to_string()),
        }),
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/models", base))
        .header("Authorization", format!("Bearer {}", args.api_key))
        .send()
        .await;

    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            Ok(KeyValidationResult {
                valid: r.status().is_success(),
                status: Some(status),
                error: if r.status().is_success() { None } else { Some(format!("HTTP {}", status)) },
            })
        }
        Err(e) => Ok(KeyValidationResult {
            valid: false,
            status: None,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn fetch_models(args: FetchModelsArgs) -> Result<ModelsResult, String> {
    let base = models_base(&args.preset, &args.upstream.unwrap_or_default());
    let base = match base {
        Some(b) => b,
        None => return Ok(ModelsResult {
            models: vec![],
            error: Some("Unknown preset".to_string()),
        }),
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/models", base))
        .header("Authorization", format!("Bearer {}", args.api_key))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let data: serde_json::Value = r.json().await.unwrap_or_default();
            let models: Vec<String> = data
                .get("data").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let mut result = ModelsResult { models, error: None };

            let presets = get_presets();
            if !presets.contains_key(args.preset.as_str()) && !result.models.contains(&"auto".to_string()) {
                result.models.insert(0, "auto".to_string());
            }
            Ok(result)
        }
        Ok(r) => Ok(ModelsResult { models: vec![], error: Some(format!("HTTP {}", r.status().as_u16())) }),
        Err(e) => Ok(ModelsResult { models: vec![], error: Some(e.to_string()) }),
    }
}

#[tauri::command]
pub async fn start_proxy(args: StartProxyArgs) -> Result<ProxyResult, String> {
    if args.api_key.is_empty() {
        return Ok(ProxyResult { ok: false, error: Some("No API key provided".to_string()), upstream: None });
    }

    let presets = get_presets();
    let (upstream, model_map, api_type) = if let Some(p) = presets.get(args.preset.as_str()) {
        let mut map = std::collections::HashMap::new();
        for (k, v) in p.models { map.insert(k.to_string(), v.to_string()); }
        (p.url.to_string(), map, p.api_type.to_string())
    } else {
        let api_type = if args.upstream.contains("/anthropic") { "anthropic" } else { "openai" };
        let target = args.selected_model.clone().unwrap_or_else(|| "auto".to_string());
        let mut map = std::collections::HashMap::new();
        for k in &["gpt-5.4", "gpt-5.4-mini", "gpt-4o", "gpt-4o-mini"] {
            map.insert(k.to_string(), target.clone());
        }
        (args.upstream.clone(), map, api_type.to_string())
    };

    let model_map = if let Some(ref sm) = args.selected_model {
        model_map.into_iter().map(|(k, _)| (k, sm.clone())).collect()
    } else {
        model_map
    };

    // Configure proxy
    let ps = proxystate();
    {
        let mut base = ps.upstream_base.write().await;
        *base = upstream.trim_end_matches('/').to_string();
    }
    {
        let mut mm = ps.model_map.write().await;
        mm.clear();
        mm.extend(model_map.clone());
    }
    {
        let mut k = ps.api_key_override.write().await;
        *k = args.api_key.clone();
    }
    {
        let mut at = ps.upstream_api_type.write().await;
        *at = api_type.clone();
    }

    // Persist settings
    let settings = Settings {
        preset: args.preset.clone(),
        upstream: upstream.clone(),
        port: args.port,
        host: "127.0.0.1".to_string(),
        api_type: api_type.clone(),
        selected_model: args.selected_model.clone(),
        model_map: model_map.clone(),
    };
    let appst = appstate();
    appst.config.save_settings(&settings).ok();
    appst.config.save_api_key(&args.api_key).ok();
    appst.config.install_codex_config(args.port, &args.api_key).ok();

    // launchctl
    std::process::Command::new("launchctl").arg("setenv").arg("CODEX_PROXY_API_KEY").arg(&args.api_key).spawn().ok();

    // Start proxy server and WAIT until it's listening
    {
        let mut inner = appst.inner.lock().unwrap();
        if !inner.proxy_running {
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            inner.proxy_handle = Some(tx);
            inner.proxy_running = true;

            let state_clone = ps.clone();
            let port = args.port;
            tauri::async_runtime::spawn(async move {
                let router = crate::proxy::build_router(state_clone);
                match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                    Ok(listener) => {
                        tracing::info!("Proxy server started on port {}", port);
                        axum::serve(listener, router).with_graceful_shutdown(async move { rx.await.ok(); }).await.ok();
                        tracing::info!("Proxy server stopped");
                    }
                    Err(e) => tracing::error!("Failed to start proxy: {}", e),
                }
            });
        }
    }
    // Poll health endpoint until server is ready (max 5s)
    let port = args.port;
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(1)).build().unwrap();
    for _i in 0..10 {
        if client.get(format!("http://127.0.0.1:{}/health", port)).send().await.is_ok() {
            tracing::info!("Proxy server ready on port {}", port);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    run_intercept_script("start");

    Ok(ProxyResult { ok: true, error: None, upstream: Some(upstream) })
}

#[tauri::command]
pub async fn stop_proxy() -> Result<bool, String> {
    let appst = appstate();
    let ps = proxystate();
    {
        let mut inner = appst.inner.lock().unwrap();
        if let Some(handle) = inner.proxy_handle.take() { let _: Result<_, _> = handle.send(()); }
        inner.proxy_running = false;
    }
    {
        let mut base = ps.upstream_base.write().await;
        *base = String::new();
    }
    appst.config.restore_original_config().ok();
    std::process::Command::new("launchctl").arg("unsetenv").arg("CODEX_PROXY_API_KEY").spawn().ok();

    run_intercept_script("stop");

    Ok(true)
}

fn run_intercept_script(action: &str) {
    let path = std::path::PathBuf::from("/Users/oleg/PythonBox HD/codex_proxy/intercept.py");
    if !path.exists() {
        tracing::info!("intercept.py not found — skipping HTTPS interception");
        return;
    }
    match std::process::Command::new("python3").arg(&path).arg(action).status() {
        Ok(status) if status.success() => {
            tracing::info!("intercept.py {} (exit=0)", action);
        }
        Ok(status) => {
            tracing::warn!("intercept.py {} exit={}", action, status);
        }
        Err(e) => {
            tracing::warn!("intercept.py {} failed: {}", action, e);
        }
    }
}

#[tauri::command]
pub async fn get_proxy_status() -> Result<bool, String> {
    Ok(appstate().inner.lock().unwrap().proxy_running)
}

#[tauri::command]
pub async fn install_config(port: u16) -> Result<String, String> {
    let key = appstate().config.load_api_key();
    appstate().config.install_codex_config(port, &key)
        .map(|_| format!("Proxy provider merged into config (port {})", port))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_config() -> Result<String, String> {
    appstate().config.restore_original_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_config_folder() -> Result<bool, String> {
    appstate().config.open_config_folder();
    Ok(true)
}

#[tauri::command]
pub async fn open_logs() -> Result<bool, String> {
    appstate().config.open_logs();
    Ok(true)
}

#[tauri::command]
pub async fn launch_codex_cli() -> Result<bool, String> {
    appstate().config.launch_codex_cli();
    Ok(true)
}

#[tauri::command]
pub async fn launch_codex_app() -> Result<bool, String> {
    appstate().config.launch_codex_app();
    Ok(true)
}

#[tauri::command]
pub async fn store_key(preset: String, key: String) -> Result<bool, String> {
    // Save to api_key file (last used key)
    appstate().config.save_api_key(&key).ok();
    Ok(true)
}

#[tauri::command]
pub async fn get_key(preset: String) -> Result<String, String> {
    Ok(appstate().config.load_api_key())
}

#[tauri::command]
pub async fn save_original_config() -> Result<String, String> {
    Ok(appstate().config.install_codex_config(appstate().config.load_settings().port, &appstate().config.load_api_key())
        .map(|_| "Original config saved".to_string())
        .unwrap_or_else(|_| "Already saved".to_string()))
}

#[tauri::command]
pub async fn get_presets_list() -> Result<Vec<String>, String> {
    let presets = get_presets();
    let mut keys: Vec<String> = presets.into_keys().map(|s| s.to_string()).collect();
    keys.push("Custom".to_string());
    Ok(keys)
}

#[tauri::command]
pub async fn get_preset_url(preset_name: String) -> Result<String, String> {
    let presets = get_presets();
    presets.get(preset_name.as_str()).map(|p| p.url.to_string()).ok_or_else(|| "Unknown preset".to_string())
}
