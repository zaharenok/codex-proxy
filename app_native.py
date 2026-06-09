#!/usr/bin/env python3
"""
Codex Proxy — Native macOS Backend.

CLI interface for Swift menu bar app.
Manages proxy server, config, and settings.

Commands:
  start, stop, status, install, logs, config        — legacy (human-readable)
  get-presets, get-settings, save-settings,          — JSON (Swift-facing)
  validate-key, fetch-models, restore-config,
  launch-codex-cli, launch-codex-app,
  store-key, get-key
"""

import json
import logging
import os
import signal
import subprocess
import sys
from pathlib import Path

# Logging to stderr so stdout is clean JSON for Swift
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    stream=sys.stderr,
)
log = logging.getLogger(__name__)

from config import (
    install_codex_config,
    load_api_key,
    load_settings,
    restore_original_config,
    save_api_key,
    save_settings,
)
from proxy import PRESETS

# Paths
CONFIG_DIR = Path.home() / ".codexproxy"
STATE_FILE = CONFIG_DIR / "app_state.json"
PID_FILE = CONFIG_DIR / "proxy.pid"
PROXY_LOG = CONFIG_DIR / "proxy.log"
KEY_CACHE_FILE = CONFIG_DIR / ".key_cache.json"


def ensure_config_dir():
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)


# ── State management ──────────────────────────────────────────────────────

def load_state():
    try:
        if STATE_FILE.exists():
            return json.loads(STATE_FILE.read_text())
    except Exception as e:
        log.warning("Failed to load state: %s", e)
    return {"proxy_running": False}


def save_state(state):
    ensure_config_dir()
    try:
        STATE_FILE.write_text(json.dumps(state, indent=2))
    except Exception as e:
        log.error("Failed to save state: %s", e)


# ── Key cache (JSON) ─────────────────────────────────────────────────────

def _load_key_cache():
    try:
        if KEY_CACHE_FILE.exists():
            return json.loads(KEY_CACHE_FILE.read_text())
    except Exception:
        pass
    return {}


def _save_key_cache(cache):
    ensure_config_dir()
    try:
        KEY_CACHE_FILE.write_text(json.dumps(cache, indent=2))
        os.chmod(str(KEY_CACHE_FILE), 0o600)
    except Exception as e:
        log.error("Failed to save key cache: %s", e)


# ── macOS notifications ──────────────────────────────────────────────────

def show_notification(title, message):
    try:
        subprocess.run(
            ["osascript", "-e", f'display notification "{message}" with title "{title}"'],
            check=True,
        )
    except Exception as e:
        log.warning("Notification failed: %s", e)


def show_alert(title, message):
    try:
        subprocess.run(
            ["osascript", "-e",
             f'tell app "System Events" to display dialog "{message}" '
             f'with title "{title}" buttons {{"OK"}} default button "OK"'],
            check=True,
        )
    except Exception as e:
        log.warning("Alert failed: %s", e)


# ── Process management ───────────────────────────────────────────────────

def _get_proxy_pid():
    """Read PID file, return PID if process alive, else None."""
    try:
        pid = int(PID_FILE.read_text().strip())
        os.kill(pid, 0)
        return pid
    except (FileNotFoundError, ValueError, ProcessLookupError, PermissionError):
        return None


# ═══════════════════════════════════════════════════════════════════════════
# LEGACY COMMANDS (human-readable output)
# ═══════════════════════════════════════════════════════════════════════════

def cmd_start():
    pid = _get_proxy_pid()
    if pid:
        log.info("Proxy already running (PID %d)", pid)
        show_notification("Already Running", f"Proxy running on PID {pid}")
        return 0

    settings = load_settings()
    api_key = load_api_key()

    if not api_key:
        show_alert("No API Key", "Set API key in Settings first")
        return 1

    preset = settings.get("preset", "")
    upstream = settings.get("upstream", "")
    port = settings.get("port", 9090)

    proxy_py = Path(__file__).parent / "proxy.py"
    cmd = [sys.executable, str(proxy_py)]
    cmd += ["--port", str(port)]
    cmd += ["--host", settings.get("host", "127.0.0.1")]
    cmd += ["--api-key", api_key]

    if preset and preset in PRESETS:
        cmd += ["--preset", preset]
    elif upstream:
        cmd += ["--upstream", upstream]
    else:
        show_alert("No Upstream", "Configure preset or upstream URL in settings")
        return 1

    ensure_config_dir()

    log_file = open(str(PROXY_LOG), "a")
    proc = subprocess.Popen(
        cmd,
        stdout=log_file,
        stderr=log_file,
        stdin=subprocess.DEVNULL,
        start_new_session=True,
    )

    PID_FILE.write_text(str(proc.pid))

    state = {"proxy_running": True, "pid": proc.pid, "port": port, "upstream": upstream or preset}
    save_state(state)

    log.info("Proxy started PID=%d port=%d -> %s", proc.pid, port, preset or upstream)
    show_notification("Proxy Started", f"Running on localhost:{port} (PID {proc.pid})")
    return 0


def cmd_stop():
    pid = _get_proxy_pid()
    if not pid:
        log.info("Proxy not running")
        save_state({"proxy_running": False})
        show_notification("Proxy Stopped", "Was not running")
        return 0

    try:
        os.kill(pid, signal.SIGTERM)
        log.info("Sent SIGTERM to PID %d", pid)
    except ProcessLookupError:
        pass

    try:
        PID_FILE.unlink()
    except FileNotFoundError:
        pass

    save_state({"proxy_running": False})
    show_notification("Proxy Stopped", "Proxy server stopped")
    return 0


def cmd_install():
    try:
        settings = load_settings()
        api_key = load_api_key()
        port = settings.get("port", 9090)

        install_codex_config(port, api_key)

        log.info("Codex config installed (port %d)", port)
        show_notification("Config Installed", f"Codex config installed on port {port}")
        return 0
    except Exception as e:
        log.error("Failed to install config: %s", e)
        show_alert("Installation Failed", str(e))
        return 1


def cmd_status():
    pid = _get_proxy_pid()
    state = load_state()
    if pid:
        print(f"Running (PID {pid})")
        print(f"Port: {state.get('port', 9090)}")
        print(f"Upstream: {state.get('upstream', 'unknown')}")
    else:
        print("Stopped")
    return 0


def cmd_open_logs():
    try:
        subprocess.run(["open", "-a", "Console", str(PROXY_LOG)], check=True)
        return 0
    except Exception as e:
        log.error("Failed to open logs: %s", e)
        return 1


def cmd_open_config():
    ensure_config_dir()
    try:
        subprocess.run(["open", str(CONFIG_DIR)], check=True)
        return 0
    except Exception as e:
        log.error("Failed to open config: %s", e)
        return 1


# ═══════════════════════════════════════════════════════════════════════════
# JSON COMMANDS (for Swift menu bar app)
# ═══════════════════════════════════════════════════════════════════════════

def json_out(data):
    """Print JSON to stdout (Swift reads this via pipe)."""
    print(json.dumps(data, ensure_ascii=False))


def _models_base(preset_name):
    """Get the /models endpoint base URL for a preset."""
    p = PRESETS.get(preset_name)
    if not p:
        return None
    return (p.get("models_url") or p["url"]).rstrip("/")


def cmd_get_presets():
    """Return all presets with their config."""
    try:
        presets = {}
        for name, p in PRESETS.items():
            presets[name] = {
                "url": p["url"],
                "api_type": p.get("api_type", "openai"),
                "models": p["models"],
            }
        json_out({"ok": True, "presets": presets})
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


def cmd_get_settings():
    """Return current settings + API key presence info."""
    try:
        settings = load_settings()
        api_key = load_api_key()
        has_key = bool(api_key)
        key_preview = ""
        if api_key:
            if len(api_key) > 7:
                key_preview = api_key[:3] + "..." + api_key[-4:]
            else:
                key_preview = "***"

        json_out({
            "ok": True,
            "preset": settings.get("preset", ""),
            "upstream": settings.get("upstream", ""),
            "port": settings.get("port", 9090),
            "selected_model": settings.get("selected_model", ""),
            "has_api_key": has_key,
            "api_key_preview": key_preview,
            "presets": list(PRESETS.keys()),
        })
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


def cmd_save_settings():
    """Save settings from --json argument. Usage: save-settings --json '{...}'"""
    try:
        # Find --json argument
        data = None
        for i, arg in enumerate(sys.argv):
            if arg == "--json" and i + 1 < len(sys.argv):
                data = json.loads(sys.argv[i + 1])
                break

        if not data:
            json_out({"ok": False, "error": "No --json argument provided"})
            return 1

        preset = data.get("preset", "")
        upstream = data.get("upstream", "")
        port = data.get("port", 9090)
        api_key = data.get("api_key", "")
        selected_model = data.get("selected_model", "")

        # Resolve model_map and api_type from preset
        model_map = {}
        api_type = "openai"
        if preset in PRESETS:
            p = PRESETS[preset]
            upstream = p["url"]
            model_map = dict(p["models"])
            api_type = p.get("api_type", "openai")
        elif upstream:
            api_type = "anthropic" if "/anthropic" in upstream else "openai"

        settings = {
            "preset": preset,
            "upstream": upstream,
            "port": int(port),
            "model_map": model_map,
            "api_type": api_type,
            "selected_model": selected_model,
        }
        save_settings(settings)

        if api_key:
            save_api_key(api_key)

        # Update Codex config.toml
        if api_key:
            from config import update_api_key_in_config
            if not update_api_key_in_config(api_key):
                install_codex_config(int(port), api_key)

        json_out({"ok": True})
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


def cmd_validate_key():
    """Validate API key against provider. Usage: validate-key --preset X --api-key Y"""
    try:
        preset = ""
        api_key = ""
        for i, arg in enumerate(sys.argv):
            if arg == "--preset" and i + 1 < len(sys.argv):
                preset = sys.argv[i + 1]
            elif arg == "--api-key" and i + 1 < len(sys.argv):
                api_key = sys.argv[i + 1]

        if not preset or not api_key:
            json_out({"valid": False, "error": "Missing --preset or --api-key"})
            return 1

        base = _models_base(preset)
        if not base:
            json_out({"valid": False, "error": f"Unknown preset: {preset}"})
            return 1

        import requests
        resp = requests.get(
            f"{base}/models",
            headers={"Authorization": f"Bearer {api_key}"},
            timeout=10,
        )
        json_out({"valid": resp.status_code == 200, "status": resp.status_code})
        return 0
    except Exception as e:
        json_out({"valid": False, "error": str(e)})
        return 1


def cmd_fetch_models():
    """Fetch available models from provider. Usage: fetch-models --preset X --api-key Y"""
    try:
        preset = ""
        api_key = ""
        for i, arg in enumerate(sys.argv):
            if arg == "--preset" and i + 1 < len(sys.argv):
                preset = sys.argv[i + 1]
            elif arg == "--api-key" and i + 1 < len(sys.argv):
                api_key = sys.argv[i + 1]

        if not preset or not api_key:
            json_out({"models": [], "error": "Missing --preset or --api-key"})
            return 1

        base = _models_base(preset)
        if not base:
            json_out({"models": [], "error": f"Unknown preset: {preset}"})
            return 1

        import requests
        resp = requests.get(
            f"{base}/models",
            headers={"Authorization": f"Bearer {api_key}"},
            timeout=15,
        )
        if resp.status_code == 200:
            data = resp.json()
            models = sorted(m["id"] for m in data.get("data", []))
            json_out({"ok": True, "models": models})
        else:
            json_out({"ok": False, "models": [], "error": f"HTTP {resp.status_code}"})
        return 0
    except Exception as e:
        json_out({"ok": False, "models": [], "error": str(e)})
        return 1


def cmd_restore_config():
    """Remove proxy sections from ~/.codex/config.toml."""
    try:
        msg = restore_original_config()
        json_out({"ok": True, "message": msg})
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


def cmd_launch_codex_cli():
    """Launch Codex CLI with proxy profile via Terminal.app."""
    try:
        script = str(Path(__file__).parent / "launch-codex.sh")
        key = load_api_key()
        env = os.environ.copy()
        env["CODEX_PROXY_API_KEY"] = key
        subprocess.Popen(
            ["open", "-a", "Terminal", script],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        json_out({"ok": True})
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


def cmd_launch_codex_app():
    """Launch Codex Desktop.app."""
    try:
        subprocess.Popen(
            ["open", "/Applications/Codex.app"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        json_out({"ok": True})
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


def cmd_store_key():
    """Cache API key per preset. Usage: store-key --preset X --api-key Y"""
    try:
        preset = ""
        api_key = ""
        for i, arg in enumerate(sys.argv):
            if arg == "--preset" and i + 1 < len(sys.argv):
                preset = sys.argv[i + 1]
            elif arg == "--api-key" and i + 1 < len(sys.argv):
                api_key = sys.argv[i + 1]

        if not preset or not api_key:
            json_out({"ok": False, "error": "Missing --preset or --api-key"})
            return 1

        cache = _load_key_cache()
        cache[preset] = api_key
        _save_key_cache(cache)
        json_out({"ok": True})
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


def cmd_get_key():
    """Get cached API key for preset. Usage: get-key --preset X"""
    try:
        preset = ""
        for i, arg in enumerate(sys.argv):
            if arg == "--preset" and i + 1 < len(sys.argv):
                preset = sys.argv[i + 1]

        cache = _load_key_cache()
        key = cache.get(preset, "")
        if not key:
            key = load_api_key()
        json_out({"ok": True, "key": key or ""})
        return 0
    except Exception as e:
        json_out({"ok": False, "error": str(e)})
        return 1


# ═══════════════════════════════════════════════════════════════════════════
# MAIN
# ═══════════════════════════════════════════════════════════════════════════

COMMANDS = {
    # Legacy (human-readable)
    "start": cmd_start,
    "stop": cmd_stop,
    "status": cmd_status,
    "install": cmd_install,
    "logs": cmd_open_logs,
    "config": cmd_open_config,
    # JSON (Swift-facing)
    "get-presets": cmd_get_presets,
    "get-settings": cmd_get_settings,
    "save-settings": cmd_save_settings,
    "validate-key": cmd_validate_key,
    "fetch-models": cmd_fetch_models,
    "restore-config": cmd_restore_config,
    "launch-codex-cli": cmd_launch_codex_cli,
    "launch-codex-app": cmd_launch_codex_app,
    "store-key": cmd_store_key,
    "get-key": cmd_get_key,
}


def main():
    if len(sys.argv) < 2:
        print("Usage: app_native.py <command>")
        print("Commands: " + ", ".join(COMMANDS.keys()))
        return 1

    command = sys.argv[1]

    if command in COMMANDS:
        return COMMANDS[command]()
    else:
        print(f"Unknown command: {command}")
        print("Available: " + ", ".join(COMMANDS.keys()))
        return 1


if __name__ == "__main__":
    sys.exit(main())
