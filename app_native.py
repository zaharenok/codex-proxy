#!/usr/bin/env python3
"""
Codex Proxy — Native macOS Backend.

CLI interface for Swift menu bar app.
Manages proxy server, config, and settings.
"""

import json
import logging
import os
import signal
import subprocess
import sys
from pathlib import Path

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
)
log = logging.getLogger(__name__)

from config import (
    install_codex_config,
    load_api_key,
    load_settings,
    save_api_key,
    save_settings,
)
from proxy import PRESETS

# Paths
CONFIG_DIR = Path.home() / ".codexproxy"
STATE_FILE = CONFIG_DIR / "app_state.json"
PID_FILE = CONFIG_DIR / "proxy.pid"
PROXY_LOG = CONFIG_DIR / "proxy.log"


def ensure_config_dir():
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)


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


def _get_proxy_pid():
    """Read PID file, return PID if process alive, else None."""
    try:
        pid = int(PID_FILE.read_text().strip())
        os.kill(pid, 0)  # Check if alive
        return pid
    except (FileNotFoundError, ValueError, ProcessLookupError, PermissionError):
        return None


def cmd_start():
    # Check if already running
    pid = _get_proxy_pid()
    if pid:
        log.info("Proxy already running (PID %d)", pid)
        show_notification("Already Running", f"Proxy running on PID {pid}")
        return 0

    settings = load_settings()
    api_key = load_api_key()

    if not api_key:
        show_alert("No API Key", "Set API key: open ~/.codexproxy/.api_key and paste your key")
        return 1

    preset = settings.get("preset", "")
    upstream = settings.get("upstream", "")
    port = settings.get("port", 9090)

    # Build command line for proxy.py standalone mode
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

    # Make API key available as env var for Codex Desktop App
    proxy_env = os.environ.copy()
    proxy_env["CODEX_PROXY_API_KEY"] = api_key
    try:
        subprocess.run(["launchctl", "setenv", "CODEX_PROXY_API_KEY", api_key], capture_output=True, timeout=5)
    except Exception:
        pass

    # Launch proxy as detached subprocess
    log_file = open(str(PROXY_LOG), "a")
    proc = subprocess.Popen(
        cmd,
        stdout=log_file,
        stderr=log_file,
        stdin=subprocess.DEVNULL,
        start_new_session=True,
        env=proxy_env,
    )

    # Save PID
    PID_FILE.write_text(str(proc.pid))

    # Save state
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

    # Clean up
    try:
        PID_FILE.unlink()
    except FileNotFoundError:
        pass

    try:
        subprocess.run(["launchctl", "unsetenv", "CODEX_PROXY_API_KEY"], capture_output=True, timeout=5)
    except Exception:
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
    log_path = PROXY_LOG
    try:
        subprocess.run(["open", "-a", "Console", str(log_path)], check=True)
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


def main():
    if len(sys.argv) < 2:
        print("Usage: app_native.py <command>")
        print("Commands: start, stop, status, install, logs, config")
        return 1

    command = sys.argv[1]

    commands = {
        "start": cmd_start,
        "stop": cmd_stop,
        "status": cmd_status,
        "install": cmd_install,
        "logs": cmd_open_logs,
        "config": cmd_open_config,
    }

    if command in commands:
        return commands[command]()
    else:
        print(f"Unknown command: {command}")
        print("Available: " + ", ".join(commands.keys()))
        return 1


if __name__ == "__main__":
    sys.exit(main())
