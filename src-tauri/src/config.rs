use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

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
        Self { config_dir, settings_file, api_key_file, codex_config }
    }

    pub fn ensure_config_dir(&self) -> io::Result<()> {
        fs::create_dir_all(&self.config_dir)
    }

    pub fn load_settings(&self) -> Settings {
        self.ensure_config_dir().ok();
        match fs::read_to_string(&self.settings_file) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save_settings(&self, settings: &Settings) -> io::Result<()> {
        self.ensure_config_dir()?;
        fs::write(&self.settings_file, serde_json::to_string_pretty(settings)?)
    }

    pub fn load_api_key(&self) -> String {
        fs::read_to_string(&self.api_key_file).map(|s| s.trim().to_string()).unwrap_or_default()
    }

    pub fn save_api_key(&self, key: &str) -> io::Result<()> {
        self.ensure_config_dir()?;
        let mut file = fs::File::create(&self.api_key_file)?;
        file.write_all(key.as_bytes())?;
        let metadata = file.metadata()?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&self.api_key_file, perms)?;
        Ok(())
    }

    /// Install Codex proxy config — raw text manipulation (like Python version)
    /// Overrides top-level `model = "gpt-4o"` so Codex (with ChatGPT auth) accepts
    /// the model name; the proxy's model_map translates gpt-4o → upstream model.
    pub fn install_codex_config(&self, port: u16, _api_key: &str) -> io::Result<bool> {
        let codex_dir = self.codex_config.parent().unwrap();
        fs::create_dir_all(codex_dir)?;

        // Save a pristine backup of the original Codex config the first time we touch it.
        // This lets restore_original_config put the user's config back exactly as it was.
        self.save_original_config()?;

        // Read existing config
        let existing = if self.codex_config.exists() {
            fs::read_to_string(&self.codex_config).unwrap_or_default()
        } else {
            String::new()
        };

        // Remove old proxy sections by filtering lines
        let mut new_lines: Vec<String> = Vec::new();
        let mut skip_until_next_section = false;
        let mut found_proxy_provider = false;
        let mut found_top_level_model = false;
        let mut replaced_top_level_model = false;

        for line in existing.lines() {
            let trimmed = line.trim();

            // Skip proxy section headers and their content
            if trimmed == "[model_providers.codex-proxy]" || trimmed.starts_with("[model_providers.codex-proxy]") {
                skip_until_next_section = true;
                continue;
            }
            if trimmed == "[profiles.proxy]" || trimmed.starts_with("[profiles.proxy]") {
                skip_until_next_section = true;
                continue;
            }
            if trimmed.starts_with("# --- CODEX PROXY") {
                skip_until_next_section = true;
                continue;
            }
            if trimmed == r#"model_provider = "codex-proxy""# {
                found_proxy_provider = true;
                continue;
            }

            // Stop skipping when we hit a new section
            if skip_until_next_section {
                if trimmed.starts_with('[') && !trimmed.starts_with("[[" ) {
                    skip_until_next_section = false;
                    new_lines.push(line.to_string());
                }
                continue;
            }

            // Override top-level `model = "..."` (only the first one we see at top level)
            if trimmed.starts_with("model =") && !found_top_level_model {
                found_top_level_model = true;
                new_lines.push(r#"model = "gpt-4o""#.to_string());
                replaced_top_level_model = true;
                continue;
            }
            // First top-level key — if no model line, we'll add it
            if trimmed.starts_with('[') && !trimmed.starts_with("[[" ) {
                found_top_level_model = true; // we're past the top-level zone
            }

            new_lines.push(line.to_string());
        }

        // If the file had no top-level model at all, prepend one
        if !replaced_top_level_model && !found_top_level_model {
            new_lines.insert(0, r#"model = "gpt-4o""#.to_string());
        }

        // Remove trailing empty lines
        while let Some(last) = new_lines.last() {
            if last.trim().is_empty() { new_lines.pop(); } else { break; }
        }

        // Add proxy config
        if !found_proxy_provider {
            new_lines.push(r#"model_provider = "codex-proxy""#.to_string());
        }

        let proxy_section = format!(
            r#"
# --- CODEX PROXY (auto-added, merge-safe) ---
[model_providers.codex-proxy]
name = "Codex Proxy"
base_url = "http://localhost:{}/v1"
wire_api = "responses"
env_key = "CODEX_PROXY_API_KEY"
requires_openai_auth = true

[profiles.proxy]
model = "gpt-4o"
model_provider = "codex-proxy"
"#,
            port
        );

        let result = new_lines.join("\n") + "\n" + &proxy_section;
        fs::write(&self.codex_config, &result)?;
        Ok(true)
    }

    /// Save a pristine copy of Codex config before any proxy modifications.
    /// If we already have a clean backup, don't overwrite it.
    fn save_original_config(&self) -> io::Result<()> {
        let backup = self.config_dir.join("config.toml.original");
        // Don't overwrite an existing clean backup
        if backup.exists() {
            let data = fs::read_to_string(&backup)?;
            if !data.contains("codex-proxy") {
                return Ok(());
            }
        }
        if !self.codex_config.exists() {
            return Ok(());
        }
        let data = fs::read_to_string(&self.codex_config)?;
        if data.contains("codex-proxy") {
            // Find a clean timestamped backup in ~/.codex
            let codex_dir = self.codex_config.parent().unwrap();
            if let Ok(entries) = fs::read_dir(codex_dir) {
                let mut candidates: Vec<std::fs::DirEntry> = entries
                    .flatten()
                    .filter(|e: &std::fs::DirEntry| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with("config.toml.bak-")
                    })
                    .collect();
                candidates.sort_by_key(|e: &std::fs::DirEntry| e.file_name().to_string_lossy().to_string());
                for entry in candidates.iter().rev() {
                    let path: std::path::PathBuf = entry.path();
                    if let Ok(d) = fs::read_to_string(&path) {
                        if !d.contains("codex-proxy") {
                            fs::copy(&path, &backup)?;
                            return Ok(());
                        }
                    }
                }
            }
            return Ok(());
        }
        fs::copy(&self.codex_config, &backup)?;
        Ok(())
    }

    pub fn restore_original_config(&self) -> io::Result<String> {
        let backup = self.config_dir.join("config.toml.original");
        if backup.exists() {
            let data = fs::read_to_string(&backup)?;
            if !data.contains("codex-proxy") {
                // Restore the pristine original
                fs::copy(&backup, &self.codex_config)?;
                return Ok("Original config restored from backup".to_string());
            }
        }
        // No backup — just strip proxy sections in place
        if !self.codex_config.exists() {
            return Ok("No Codex config found".to_string());
        }
        let content = fs::read_to_string(&self.codex_config)?;
        let mut new_lines: Vec<&str> = Vec::new();
        let mut skip = false;
        for line in content.lines() {
            let t = line.trim();
            if t == "[model_providers.codex-proxy]" || t == "[profiles.proxy]" || t.starts_with("# --- CODEX PROXY") {
                skip = true; continue;
            }
            if skip {
                if t.starts_with('[') && !t.starts_with("[[" ) { skip = false; }
                else { continue; }
            }
            if t == r#"model_provider = "codex-proxy""# { continue; }
            new_lines.push(line);
        }
        while let Some(last) = new_lines.last() {
            if last.trim().is_empty() { new_lines.pop(); } else { break; }
        }
        fs::write(&self.codex_config, new_lines.join("\n") + "\n")?;
        Ok("Proxy sections removed".to_string())
    }

    pub fn open_config_folder(&self) {
        let path = self.config_dir.to_string_lossy().to_string();
        std::process::Command::new("open").arg(&path).spawn().ok();
    }

    pub fn open_logs(&self) {
        let log_path = self.config_dir.join("proxy.log");
        std::process::Command::new("open").arg("-a").arg("Console").arg(&log_path).spawn().ok();
    }

    pub fn launch_codex_cli(&self) {
        let key = self.load_api_key();
        let script = std::env::current_dir().unwrap_or_default().join("launch-codex.sh");
        std::process::Command::new("open").arg("-a").arg("Terminal").arg(&script)
            .env("CODEX_PROXY_API_KEY", key).spawn().ok();
    }

    pub fn launch_codex_app(&self) {
        std::process::Command::new("open").arg("/Applications/Codex.app").spawn().ok();
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
            inner: Mutex::new(AppStateInner { proxy_running: false, proxy_handle: None }),
            config: Config::new(),
        }
    }

    pub fn global() -> &'static AppState {
        static GLOBAL: OnceLock<AppState> = OnceLock::new();
        GLOBAL.get_or_init(|| AppState::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config_in(dir: &std::path::Path) -> Config {
        Config {
            config_dir: dir.to_path_buf(),
            settings_file: dir.join("settings.json"),
            api_key_file: dir.join(".api_key"),
            codex_config: dir.join("config.toml"),
        }
    }

    #[test]
    fn install_overrides_top_level_model_to_gpt4o() {
        let tmp = tempdir();
        let cfg = make_config_in(&tmp);
        let toml = r#"model = "gpt-5.4"
model_reasoning_effort = "high"
notify = ["x"]
"#;
        std::fs::write(&cfg.codex_config, toml).unwrap();

        cfg.install_codex_config(9090, "fake_key").unwrap();
        let result = std::fs::read_to_string(&cfg.codex_config).unwrap();

        assert!(result.contains(r#"model = "gpt-4o""#), "expected top-level model override, got:\n{}", result);
        assert!(!result.contains(r#"model = "gpt-5.4""#), "old gpt-5.4 should be gone");
        assert!(result.contains(r#"model_provider = "codex-proxy""#));
        assert!(result.contains("[model_providers.codex-proxy]"));
        assert!(result.contains("[profiles.proxy]"));
        assert!(result.contains(r#"base_url = "http://localhost:9090/v1""#));
    }

    #[test]
    fn install_preserves_existing_user_sections() {
        let tmp = tempdir();
        let cfg = make_config_in(&tmp);
        let toml = r#"model = "gpt-5.4"
model_provider = "openai"

[projects."/some/path"]
trust_level = "trusted"

[plugins."github@openai-curated"]
enabled = true
"#;
        std::fs::write(&cfg.codex_config, toml).unwrap();

        cfg.install_codex_config(9090, "fake_key").unwrap();
        let result = std::fs::read_to_string(&cfg.codex_config).unwrap();

        assert!(result.contains(r#"model = "gpt-4o""#));
        assert!(result.contains(r#"[projects."/some/path"]"#));
        assert!(result.contains(r#"[plugins."github@openai-curated"]"#));
        assert!(result.contains("trust_level = \"trusted\""));
    }

    #[test]
    fn install_then_restore_restores_original() {
        let tmp = tempdir();
        let cfg = make_config_in(&tmp);
        let toml = "model = \"gpt-5.4\"\nnotify = [\"x\"]\n";
        std::fs::write(&cfg.codex_config, toml).unwrap();

        cfg.install_codex_config(9090, "fake_key").unwrap();
        let after_install = std::fs::read_to_string(&cfg.codex_config).unwrap();
        assert!(after_install.contains("# --- CODEX PROXY"));

        cfg.restore_original_config().unwrap();
        let after_restore = std::fs::read_to_string(&cfg.codex_config).unwrap();

        assert!(!after_restore.contains("codex-proxy"), "proxy section should be removed, got:\n{}", after_restore);
        assert!(!after_restore.contains("# --- CODEX PROXY"));
        assert!(after_restore.contains("model = \"gpt-5.4\""), "original gpt-5.4 should be restored");
        assert!(after_restore.contains("notify = [\"x\"]"));
    }

    #[test]
    fn install_does_not_overwrite_existing_clean_backup() {
        let tmp = tempdir();
        let cfg = make_config_in(&tmp);
        let backup = tmp.join("config.toml.original");
        std::fs::write(&backup, "model = \"gpt-5.4\"\nnotify = [\"x\"]\n").unwrap();

        let toml = "model = \"gpt-5.4\"\nnotify = [\"x\"]\n";
        std::fs::write(&cfg.codex_config, toml).unwrap();
        cfg.install_codex_config(9090, "fake_key").unwrap();

        let backup_data = std::fs::read_to_string(&backup).unwrap();
        assert!(backup_data.contains("model = \"gpt-5.4\""));
        assert!(!backup_data.contains("codex-proxy"));
    }

    fn tempdir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("codex-proxy-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
