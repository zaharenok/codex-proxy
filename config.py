#!/usr/bin/env python3
"""
Config management for Codex Proxy.

Settings: ~/.codexproxy/settings.json
API key:  ~/.codexproxy/.api_key
Codex:    ~/.codex/config.toml (MERGED, never overwritten)
"""

import json
import os
import shutil
from datetime import datetime
from pathlib import Path

CONFIG_DIR = Path.home() / ".codexproxy"
SETTINGS_FILE = CONFIG_DIR / "settings.json"
API_KEY_FILE = CONFIG_DIR / ".api_key"
CODEX_CONFIG = Path.home() / ".codex" / "config.toml"

DEFAULT_SETTINGS = {
    "preset": "z.ai",
    "upstream": "https://api.z.ai/api/anthropic",
    "port": 9090,
    "host": "127.0.0.1",
    "api_type": "anthropic",
    "model_map": {
        "gpt-5.4": "claude-sonnet-4-20250514",
        "gpt-5.4-mini": "glm-5.1",
    },
}

PROXY_PROVIDER_ID = "codex-proxy"
PROXY_PROFILE_ID = "proxy"


def ensure_config_dir():
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)


def load_settings() -> dict:
    """Load settings from disk, return defaults if not found."""
    ensure_config_dir()
    try:
        with open(SETTINGS_FILE, "r", encoding="utf-8") as f:
            settings = json.load(f)
        merged = {**DEFAULT_SETTINGS, **settings}
        return merged
    except (FileNotFoundError, json.JSONDecodeError):
        return DEFAULT_SETTINGS.copy()


def save_settings(settings: dict):
    """Persist settings to disk."""
    ensure_config_dir()
    with open(SETTINGS_FILE, "w", encoding="utf-8") as f:
        json.dump(settings, f, indent=2, ensure_ascii=False)


def load_api_key() -> str:
    """Load API key from disk."""
    try:
        return API_KEY_FILE.read_text(encoding="utf-8").strip()
    except FileNotFoundError:
        return ""


def save_api_key(key: str):
    """Save API key to disk."""
    ensure_config_dir()
    API_KEY_FILE.write_text(key, encoding="utf-8")
    os.chmod(API_KEY_FILE, 0o600)


def _find_section_lines(lines: list[str], section_header: str) -> tuple[int, int]:
    """Find start and end line indices of a TOML section."""
    start = None
    for i, line in enumerate(lines):
        if line.strip().startswith(section_header):
            start = i
            continue
        if start is not None and line.strip().startswith("[") and not line.strip().startswith("[["):
            return start, i
    if start is not None:
        return start, len(lines)
    return -1, -1


def _remove_proxy_sections(lines: list[str]) -> list[str]:
    """Remove proxy-related sections, top-level model_provider, and header comments."""
    new_lines = []
    skip_until_next = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith(f"[model_providers.{PROXY_PROVIDER_ID}]") and stripped.endswith("]"):
            skip_until_next = True
            continue
        if stripped.startswith(f"[profiles.{PROXY_PROFILE_ID}]") and stripped.endswith("]"):
            skip_until_next = True
            continue
        if stripped.startswith("# --- CODEX PROXY"):
            continue
        if stripped.startswith(f'model_provider = "{PROXY_PROVIDER_ID}"'):
            continue
        if skip_until_next:
            if stripped.startswith("[") and not stripped.startswith("[["):
                skip_until_next = False
                new_lines.append(line)
            continue
        new_lines.append(line)
    while new_lines and new_lines[-1].strip() == "":
        new_lines.pop()
    return new_lines


def install_codex_config(port: int = 9090, api_key: str = "") -> bool:
    """MERGE proxy provider into existing ~/.codex/config.toml. Never overwrite."""
    save_original_config()
    codex_dir = CODEX_CONFIG.parent
    codex_dir.mkdir(parents=True, exist_ok=True)

    # Read existing config (or empty)
    existing = ""
    if CODEX_CONFIG.exists():
        existing = CODEX_CONFIG.read_text(encoding="utf-8")
        # Backup
        backup = CODEX_CONFIG.with_suffix(f".toml.bak-{datetime.now().strftime('%Y%m%d%H%M%S')}")
        shutil.copy2(CODEX_CONFIG, backup)

    lines = existing.split("\n") if existing else []

    # Remove old proxy sections if present
    new_lines = _remove_proxy_sections(lines)

    # Set model_provider at top level so Desktop App routes through proxy
    mp_set = False
    for i, line in enumerate(new_lines):
        if line.strip().startswith("model_provider ="):
            new_lines[i] = f'model_provider = "{PROXY_PROVIDER_ID}"'
            mp_set = True
            break
    if not mp_set:
        for i, line in enumerate(new_lines):
            if line.strip().startswith("model ="):
                new_lines.insert(i + 1, f'model_provider = "{PROXY_PROVIDER_ID}"')
                break

    # Add our proxy section
    key_line = f'env_key = "CODEX_PROXY_API_KEY"'
    proxy_section = f"""
# --- CODEX PROXY (auto-added, merge-safe) ---
[model_providers.{PROXY_PROVIDER_ID}]
name = "Codex Proxy"
base_url = "http://localhost:{port}/v1"
wire_api = "responses"
requires_openai_auth = true
{key_line}

[profiles.{PROXY_PROFILE_ID}]
model = "gpt-5.4"
model_provider = "{PROXY_PROVIDER_ID}"
"""
    new_lines.append(proxy_section)

    CODEX_CONFIG.write_text("\n".join(new_lines), encoding="utf-8")
    return True


def update_api_key_in_config(api_key: str) -> bool:
    """Update only the bearer token in config.toml without touching anything else."""
    if not CODEX_CONFIG.exists():
        return False

    lines = CODEX_CONFIG.read_text(encoding="utf-8").split("\n")
    in_proxy_section = False
    updated = False

    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped == f"[model_providers.{PROXY_PROVIDER_ID}]":
            in_proxy_section = True
            continue
        if in_proxy_section and stripped.startswith("[") and not stripped.startswith("[["):
            in_proxy_section = False
            continue
        if in_proxy_section and ("env_key" in stripped or "experimental_bearer_token" in stripped):
            lines[i] = 'env_key = "CODEX_PROXY_API_KEY"'
            updated = True
            break

    if updated:
        CODEX_CONFIG.write_text("\n".join(lines), encoding="utf-8")
    return updated


def restore_config() -> str:
    """Restore backup (prefer clean ones without proxy content). Returns message."""
    import glob
    backup_dir = str(CODEX_CONFIG.parent)
    backups = sorted(glob.glob(os.path.join(backup_dir, "config.toml.bak-*")))
    if not backups:
        return "No backup found"

    # Prefer clean backups (no proxy content), fallback to biggest
    clean = [f for f in backups if PROXY_PROVIDER_ID not in Path(f).read_text(encoding="utf-8", errors="ignore")]
    target = max(clean, key=os.path.getsize) if clean else max(backups, key=os.path.getsize)

    shutil.copy2(target, CODEX_CONFIG)
    return f"Restored from {os.path.basename(target)}"


ORIGINAL_CONFIG = CONFIG_DIR / "config.toml.original"


def _is_contaminated(path: Path) -> bool:
    return PROXY_PROVIDER_ID in path.read_text(encoding="utf-8", errors="ignore")

def _find_clean_backup() -> str | None:
    import glob
    backup_dir = str(CODEX_CONFIG.parent)
    backups = sorted(glob.glob(os.path.join(backup_dir, "config.toml.bak-*")))
    clean = [f for f in backups if not _is_contaminated(Path(f))]
    return str(clean[-1]) if clean else None


def save_original_config() -> str:
    """Save a pristine copy of Codex config before any proxy modifications."""
    if ORIGINAL_CONFIG.exists() and not _is_contaminated(ORIGINAL_CONFIG):
        return "Original already saved"
    if not CODEX_CONFIG.exists():
        return "No Codex config found"

    if _is_contaminated(CODEX_CONFIG):
        src = _find_clean_backup()
        if src is None:
            return "No clean original config found"
    else:
        src = str(CODEX_CONFIG)

    shutil.copy2(src, ORIGINAL_CONFIG)
    return f"Saved original config from {src}"


def restore_original_config() -> str:
    """Remove proxy sections from current config preserving all other content."""
    if not CODEX_CONFIG.exists():
        return "No Codex config found"
    lines = CODEX_CONFIG.read_text(encoding="utf-8").split("\n")
    new_lines = _remove_proxy_sections(lines)
    if len(new_lines) == len(lines):
        return "No proxy sections found to remove"
    CODEX_CONFIG.write_text("\n".join(new_lines), encoding="utf-8")
    return "Proxy sections removed, original content preserved"


def get_codex_config_path() -> str:
    return str(CODEX_CONFIG)
