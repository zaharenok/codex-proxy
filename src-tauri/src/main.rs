#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod proxy;

use commands::*;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn main() {
    // Set up a layered subscriber: stderr (always) + ~/.codexproxy/proxy.log
    use std::io::Write;
    let log_path = dirs::home_dir()
        .map(|h| h.join(".codexproxy").join("proxy.log"))
        .unwrap_or_else(|| std::path::PathBuf::from("proxy.log"));

    // Truncate log on each start so we don't accumulate from old sessions
    let _ = std::fs::File::create(&log_path);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "codex_proxy=info".into());

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr);

    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();
    let file_layer = file.map(|f| {
        tracing_subscriber::fmt::layer()
            .with_writer(move || f.try_clone().expect("log file clone"))
            .with_ansi(false)
    });

    if let Some(fl) = file_layer {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .with(fl)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(stderr_layer)
            .init();
    }

    eprintln!("[codex-proxy] Starting... (log: {})", log_path.display());
    tracing::info!("=== codex-proxy started, log file: {} ===", log_path.display());

    // Initialize global state
    config::AppState::global();
    proxy::ProxyState::global();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
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
                        "start" | "stop" => {
                            // These are handled via the WebView UI commands
                        }
                        "quit" => { app.exit(0); }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
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
            store_key,
            get_key,
            save_original_config,
            get_presets_list,
            get_preset_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
