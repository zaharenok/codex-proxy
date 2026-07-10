use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub preset: String,
    pub upstream: String,
    pub port: u16,
    pub host: String,
    pub api_type: String,
    pub selected_model: Option<String>,
    pub model_map: std::collections::HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        let mut model_map = std::collections::HashMap::new();
        model_map.insert("gpt-5.4".to_string(), "claude-sonnet-4-20250514".to_string());
        model_map.insert("gpt-5.4-mini".to_string(), "glm-5.1".to_string());
        Self {
            preset: "z.ai".to_string(),
            upstream: "https://api.z.ai/api/anthropic".to_string(),
            port: 9090,
            host: "127.0.0.1".to_string(),
            api_type: "anthropic".to_string(),
            selected_model: None,
            model_map,
        }
    }
}

pub struct Config {
    pub config_dir: PathBuf,
    pub settings_file: PathBuf,
    pub api_key_file: PathBuf,
    pub codex_config: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("HOME not found");
        let config_dir = home.join(".codexproxy");
        let settings_file = config_dir.join("settings.json");
        let api_key_file = config_dir.join(".api_key");
        let codex_config = home.join(".codex").join("config.toml");

        Self {
            config_dir,
            settings_file,
            api_key_file,
            codex_config,
        }
    }

    pub fn ensure_config_dir(&self) -> io::Result<()> {
        fs::create_dir_all(&self.config_dir)
    }

    pub fn load_settings(&self) -> Settings {
        self.ensure_config_dir().ok();
        match fs::read_to_string(&self.settings_file) {
            Ok(content) => {
                serde_json::from_str(&content).unwrap_or_default()
            }
            Err(_) => Settings::default(),
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> io::Result<()> {
        self.ensure_config_dir()?;
        let content = serde_json::to_string_pretty(settings)?;
        fs::write(&self.settings_file, content)
    }

    pub fn load_api_key(&self) -> String {
        fs::read_to_string(&self.api_key_file)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    pub fn save_api_key(&self, key: &str) -> io::Result<()> {
        self.ensure_config_dir()?;
        let mut file = fs::File::create(&self.api_key_file)?;
        file.write_all(key.as_bytes())?;
        // chmod 600
        let metadata = file.metadata()?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&self.api_key_file, perms)?;
        Ok(())
    }

    pub fn install_codex_config(&self, port: u16, _api_key: &str) -> io::Result<bool> {
        let codex_dir = self.codex_config.parent().unwrap();
        fs::create_dir_all(codex_dir)?;

        let existing = if self.codex_config.exists() {
            self.save_original_config()?;
            fs::read_to_string(&self.codex_config)?
        } else {
            String::new()
        };

        let mut doc = if existing.is_empty() {
            toml_edit::DocumentMut::new()
        } else {
            existing.parse::<toml_edit::DocumentMut>()
                .unwrap_or_else(|_| toml_edit::DocumentMut::new())
        };

        // Remove old proxy sections
        let provider_key = format!("model_providers.codex-proxy");
        let profile_key = format!("profiles.proxy");

        doc.remove(&provider_key);
        doc.remove(&profile_key);
        // Also remove top-level model_provider = "codex-proxy"
        if let Some(val) = doc.get("model_provider") {
            if val.as_str() == Some("codex-proxy") {
                doc.remove("model_provider");
            }
        }

        // Set model_provider at top level
        doc["model_provider"] = toml_edit::value("codex-proxy");

        // Add proxy section as inline table
        let mut provider_table = toml_edit::InlineTable::new();
        provider_table.insert("name", "Codex Proxy".into());
        provider_table.insert("base_url", format!("http://localhost:{}/v1", port).into());
        provider_table.insert("wire_api", "responses".into());
        provider_table.insert("requires_openai_auth", true.into());
        provider_table.insert("env_key", "CODEX_PROXY_API_KEY".into());

        doc[&provider_key] = toml_edit::value(provider_table);

        // Add profile section
        let mut profile_table = toml_edit::InlineTable::new();
        profile_table.insert("model", "gpt-5.4".into());
        profile_table.insert("model_provider", "codex-proxy".into());

        doc[&profile_key] = toml_edit::value(profile_table);

        fs::write(&self.codex_config, doc.to_string())?;
        Ok(true)
    }

    pub fn restore_original_config(&self) -> io::Result<String> {
        if !self.codex_config.exists() {
            return Ok("No Codex config found".to_string());
        }
        let content = fs::read_to_string(&self.codex_config)?;
        let mut doc: toml_edit::DocumentMut = content.parse()
            .unwrap_or_else(|_| toml_edit::DocumentMut::new());

        doc.remove("model_providers.codex-proxy");
        doc.remove("profiles.proxy");

        if let Some(val) = doc.get("model_provider") {
            if val.as_str() == Some("codex-proxy") {
                doc.remove("model_provider");
            }
        }

        fs::write(&self.codex_config, doc.to_string())?;
        Ok("Proxy sections removed".to_string())
    }

    fn save_original_config(&self) -> io::Result<()> {
        let original = self.config_dir.join("config.toml.original");
        if original.exists() {
            let data = fs::read_to_string(&original)?;
            if data.contains("codex-proxy") {
                // Find a clean backup
                let codex_dir = self.codex_config.parent().unwrap();
                if let Ok(entries) = fs::read_dir(codex_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with("config.toml.bak-") {
                            if let Ok(data) = fs::read_to_string(entry.path()) {
                                if !data.contains("codex-proxy") {
                                    fs::copy(entry.path(), &original)?;
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
            return Ok(());
        }
        if self.codex_config.exists() {
            let data = fs::read_to_string(&self.codex_config).unwrap_or_default();
            if !data.contains("codex-proxy") {
                fs::copy(&self.codex_config, &original)?;
            }
        }
        Ok(())
    }

    pub fn open_config_folder(&self) {
        let path = self.config_dir.to_string_lossy().to_string();
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .ok();
    }

    pub fn open_logs(&self) {
        let log_path = self.config_dir.join("proxy.log");
        std::process::Command::new("open")
            .arg("-a")
            .arg("Console")
            .arg(&log_path)
            .spawn()
            .ok();
    }

    pub fn launch_codex_cli(&self) {
        let key = self.load_api_key();
        let script = std::env::current_dir()
            .unwrap_or_default()
            .join("launch-codex.sh");
        std::process::Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(&script)
            .env("CODEX_PROXY_API_KEY", key)
            .spawn()
            .ok();
    }

    pub fn launch_codex_app(&self) {
        std::process::Command::new("open")
            .arg("/Applications/Codex.app")
            .spawn()
            .ok();
    }
}

pub struct AppStateInner {
    pub proxy_running: bool,
    pub proxy_handle: Option<tokio::sync::oneshot::Sender<()>>,
}

pub struct AppState {
    pub inner: Mutex<AppStateInner>,
    pub config: Config,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(AppStateInner {
                proxy_running: false,
                proxy_handle: None,
            }),
            config: Config::new(),
        }
    }
}
