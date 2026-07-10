use crate::config::{AppState, Settings};
use crate::proxy::{get_presets, ProxyState};
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct StartProxyArgs {
    pub preset: String,
    pub upstream: String,
    pub api_key: String,
    pub port: u16,
    pub selected_model: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidateKeyArgs {
    pub preset: String,
    pub api_key: String,
    pub upstream: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[tauri::command]
pub async fn load_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.config.load_settings())
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<bool, String> {
    state.config.save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn start_proxy(
    app_state: State<'_, AppState>,
    proxy_state: State<'_, ProxyState>,
    args: StartProxyArgs,
) -> Result<ProxyResult, String> {
    if args.api_key.is_empty() {
        return Ok(ProxyResult {
            ok: false,
            error: Some("No API key provided".to_string()),
            upstream: None,
        });
    }

    let presets = get_presets();
    let (upstream, model_map, api_type) = if let Some(p) = presets.get(args.preset.as_str()) {
        let mut map = std::collections::HashMap::new();
        for (k, v) in p.models {
            map.insert(k.to_string(), v.to_string());
        }
        (p.url.to_string(), map, p.api_type.to_string())
    } else {
        // Custom upstream
        let api_type = if args.upstream.contains("/anthropic") {
            "anthropic".to_string()
        } else {
            "openai".to_string()
        };
        let target = args.selected_model.clone().unwrap_or_else(|| "auto".to_string());
        let mut map = std::collections::HashMap::new();
        for k in &["gpt-5.4", "gpt-5.4-mini", "gpt-4o", "gpt-4o-mini"] {
            map.insert(k.to_string(), target.clone());
        }
        (args.upstream.clone(), map, api_type)
    };

    // Override all models if selected_model is provided
    let model_map = if let Some(ref sm) = args.selected_model {
        model_map.into_iter().map(|(k, _)| (k, sm.clone())).collect()
    } else {
        model_map
    };

    // Configure proxy state
    {
        let mut base = proxy_state.upstream_base.write().await;
        *base = upstream.trim_end_matches('/').to_string();
    }
    {
        let mut mm = proxy_state.model_map.write().await;
        mm.clear();
        mm.extend(model_map.clone());
    }
    {
        let mut key = proxy_state.api_key_override.write().await;
        *key = args.api_key.clone();
    }
    {
        let mut at = proxy_state.upstream_api_type.write().await;
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
    app_state.config.save_settings(&settings).ok();
    app_state.config.save_api_key(&args.api_key).ok();

    // Install Codex config
    app_state.config.install_codex_config(args.port, &args.api_key).ok();

    // Set env var via launchctl
    std::process::Command::new("launchctl")
        .arg("setenv")
        .arg("CODEX_PROXY_API_KEY")
        .arg(&args.api_key)
        .spawn()
        .ok();

    Ok(ProxyResult {
        ok: true,
        error: None,
        upstream: Some(upstream),
    })
}

#[tauri::command]
pub async fn stop_proxy(
    app_state: State<'_, AppState>,
    proxy_state: State<'_, ProxyState>,
) -> Result<bool, String> {
    // Clear the running flag
    {
        let mut inner = app_state.inner.lock().unwrap();
        if let Some(handle) = inner.proxy_handle.take() {
            let _ = handle.send(());
        }
        inner.proxy_running = false;
    }

    // Reset upstream
    {
        let mut base = proxy_state.upstream_base.write().await;
        *base = String::new();
    }

    // Restore Codex config
    app_state.config.restore_original_config().ok();

    // Unset env var
    std::process::Command::new("launchctl")
        .arg("unsetenv")
        .arg("CODEX_PROXY_API_KEY")
        .spawn()
        .ok();

    Ok(true)
}

#[tauri::command]
pub async fn validate_key(
    args: ValidateKeyArgs,
) -> Result<KeyValidationResult, String> {
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
pub async fn fetch_models(
    args: FetchModelsArgs,
) -> Result<ModelsResult, String> {
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
                .get("data")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.get("id").and_then(|id| id.as_str()).map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let mut result = ModelsResult {
                models,
                error: None,
            };

            // If custom preset and "auto" not in models, add it
            let presets = get_presets();
            if !presets.contains_key(args.preset.as_str()) && !result.models.contains(&"auto".to_string()) {
                result.models.insert(0, "auto".to_string());
            }

            Ok(result)
        }
        Ok(r) => Ok(ModelsResult {
            models: vec![],
            error: Some(format!("HTTP {}", r.status().as_u16())),
        }),
        Err(e) => Ok(ModelsResult {
            models: vec![],
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
pub async fn get_proxy_status(state: State<'_, AppState>) -> Result<bool, String> {
    let inner = state.inner.lock().unwrap();
    Ok(inner.proxy_running)
}

#[tauri::command]
pub async fn install_config(state: State<'_, AppState>, port: u16) -> Result<String, String> {
    let key = state.config.load_api_key();
    state.config.install_codex_config(port, &key)
        .map(|_| format!("Proxy provider merged into config (port {})", port))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_config(state: State<'_, AppState>) -> Result<String, String> {
    state.config.restore_original_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_config_folder(state: State<'_, AppState>) -> Result<bool, String> {
    state.config.open_config_folder();
    Ok(true)
}

#[tauri::command]
pub async fn open_logs(state: State<'_, AppState>) -> Result<bool, String> {
    state.config.open_logs();
    Ok(true)
}

#[tauri::command]
pub async fn launch_codex_cli(state: State<'_, AppState>) -> Result<bool, String> {
    state.config.launch_codex_cli();
    Ok(true)
}

#[tauri::command]
pub async fn launch_codex_app(state: State<'_, AppState>) -> Result<bool, String> {
    state.config.launch_codex_app();
    Ok(true)
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
    presets
        .get(preset_name.as_str())
        .map(|p| p.url.to_string())
        .ok_or_else(|| "Unknown preset".to_string())
}
