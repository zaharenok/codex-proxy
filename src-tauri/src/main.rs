// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod proxy;

use commands::*;
use config::AppState;
use proxy::ProxyState;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "codex_proxy=info".into()),
        )
        .init();

    let app_state = Arc::new(AppState::new());
    let proxy_state = Arc::new(ProxyState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state.clone())
        .manage(proxy_state.clone())
        .setup(move |app| {
            // Build system tray
            let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
            let start_i = MenuItem::with_id(app, "start", "▶ Start Proxy", true, None::<&str>)?;
            let stop_i = MenuItem::with_id(app, "stop", "⏹ Stop Proxy", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&show_i, &start_i, &stop_i, &quit_i])?;

            TrayIconBuilder::new()
                .menu(&menu)
                .icon_as_template(true)
                .on_menu_event(move |app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "start" => {
                            // Start proxy with current settings
                            let state = app.state::<AppState>();
                            let ps = app.state::<ProxyState>();
                            let settings = state.config.load_settings();
                            let key = state.config.load_api_key();

                            if !key.is_empty() {
                                let api_type = if settings.upstream.contains("/anthropic") {
                                    "anthropic".to_string()
                                } else {
                                    "openai".to_string()
                                };

                                tauri::async_runtime::block_on(async {
                                    let mut base = ps.upstream_base.write().await;
                                    *base = settings.upstream.trim_end_matches('/').to_string();
                                    let mut mm = ps.model_map.write().await;
                                    mm.clear();
                                    mm.extend(settings.model_map.clone());
                                    let mut k = ps.api_key_override.write().await;
                                    *k = key.clone();
                                    let mut at = ps.upstream_api_type.write().await;
                                    *at = api_type;
                                });

                                let mut inner = state.inner.lock().unwrap();
                                if !inner.proxy_running {
                                    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
                                    inner.proxy_handle = Some(tx);
                                    inner.proxy_running = true;

                                    let state_clone = (*ps).clone();
                                    let port = settings.port;
                                    tauri::async_runtime::spawn(async move {
                                        let router = proxy::build_router(state_clone);
                                        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await;
                                        match listener {
                                            Ok(listener) => {
                                                tracing::info!("Proxy server started on port {}", port);
                                                axum::serve(listener, router)
                                                    .with_graceful_shutdown(async move {
                                                        rx.await.ok();
                                                    })
                                                    .await.ok();
                                                tracing::info!("Proxy server stopped");
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to start proxy: {}", e);
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        "stop" => {
                            let state = app.state::<AppState>();
                            let ps = app.state::<ProxyState>();
                            {
                                let mut inner = state.inner.lock().unwrap();
                                if let Some(handle) = inner.proxy_handle.take() {
                                    let _ = handle.send(());
                                }
                                inner.proxy_running = false;
                            }
                            {
                                let ps_clone = ps.inner().clone();
                                tauri::async_runtime::block_on(async {
                                    let mut base = ps_clone.upstream_base.write().await;
                                    *base = String::new();
                                });
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            start_proxy,
            stop_proxy,
            validate_key,
            fetch_models,
            get_proxy_status,
            install_config,
            restore_config,
            open_config_folder,
            open_logs,
            launch_codex_cli,
            launch_codex_app,
            get_presets_list,
            get_preset_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
