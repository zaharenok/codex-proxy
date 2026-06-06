#!/usr/bin/env python3
"""
Codex Proxy — Native macOS Backend.

CLI interface for Swift menu bar app.
Manages proxy server, config, and settings.
"""

import json
import logging
import os
import subprocess
import sys
from pathlib import Path

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [%(levelname)s] %(message)s',
)
log = logging.getLogger(__name__)

# Import from existing modules
from config import (
    install_codex_config, 
    load_api_key, 
    load_settings, 
    save_api_key, 
    save_settings
)
from proxy import (
    PRESETS, 
    configure, 
    start_server, 
    stop_server
)

# Paths
CONFIG_DIR = Path.home() / ".codexproxy"
STATE_FILE = CONFIG_DIR / "app_state.json"

def ensure_config_dir():
    """Ensure config directory exists."""
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)

def load_state():
    """Load application state."""
    try:
        if STATE_FILE.exists():
            return json.loads(STATE_FILE.read_text())
    except Exception as e:
        log.warning(f"Failed to load state: {e}"")
    return {"proxy_running": False}

def save_state(state):
    """Save application state."""
    ensure_config_dir()
    try:
        STATE_FILE.write_text(json.dumps(state, indent=2))
    except Exception as e:
        log.error(f"Failed to save state: {e}"")

def show_notification(title, message):
    """Show macOS notification."""
    script = f"""osascript -e 'display notification "{message}" with title "{title}"'"""
    try:
        subprocess.run(script, shell=True, check=True)
    except Exception as e:
        log.warning(f"Failed to show notification: {e}"")

def show_alert(title, message, style="informational""):
    """Show macOS alert dialog."""
    buttons = "\"OK\""  # Escape quotes for AppleScript
    script = f"""osascript -e 'tell app "System Events" to display dialog "{message}" with title "{title}" buttons {{"OK"}} default button "OK"'"""
    try:
        subprocess.run(script, shell=True, check=True)
    except Exception as e:
        log.warning(f"Failed to show alert: {e}"")

def cmd_start():
    """Start proxy server."""
    state = load_state()
    if state.get("proxy_running""):
        log.info("Proxy already running")
        show_notification("Already Running", "Proxy is already running")
        return 1
    
    settings = load_settings()
    api_key = load_api_key()
    
    if not api_key:
        show_alert("No API Key", "Please set API key in settings first", "warning")
        return 1
    
    try:
        preset = settings.get("preset", "")
        upstream = settings.get("upstream", "")
        port = settings.get("port", 9090)
        
        if preset in PRESETS:
            p = PRESETS[preset]
            upstream = p["url""]["models"]
            api_type = p.get("api_type", "openai")
        else:
            model_map = {}
            api_type = "anthropic" if "/anthropic" in upstream else "openai"
        
        configure(upstream, model_map, api_key, api_type)
        start_server(port=port)
        
        state["proxy_running"] = True
        state["upstream"] = upstream
        state["port"] = port
        save_state(state)
        
        log.info(f"Proxy started on port {port} -> {upstream}")
        show_notification("Proxy Started", f"Running on localhost:{port}")
        return 0
        
    except Exception as e:
        log.error(f"Failed to start: {e}"" )
        show_alert("Failed to Start", str(e), "warning")
        return 1

def cmd_stop():
    """Stop proxy server."""
    try:
        stop_server()
        state = load_state()
        state["proxy_running"] = False
        save_state(state)
        
        log.info("Proxy stopped")
        show_notification("Proxy Stopped", "Proxy server stopped")
        return 0
    except Exception as e:
        log.error(f"Failed to stop: {e}"" )
        return 1

def cmd_install():
    """Install Codex config."""
    try:
        settings = load_settings()
        api_key = load_api_key()
        port = settings.get("port", 9090)
        
        install_codex_config(port, api_key)
        
        log.info(f"Codex config installed (port {port})")
        show_notification("Config Installed", f"Codex config installed on port {port}")
        return 0
    except Exception as e:
        log.error(f"Failed to install config: {e}"" )
        show_alert("Installation Failed", str(e), "warning")
        return 1

def cmd_status():
    """Print proxy status."""
    state = load_state()
    status = "Running" if state.get("proxy_running"") else "Stopped"
    print(f"Status: {status}")
    if state.get("proxy_running"):
        print(f"Port: {state.get("port", 9090)}")
        print(f"Upstream: {state.get("upstream", "unknown"}"" )
    return 0

def cmd_open_logs():
    """Open logs in Console.app."""
    log_path = CONFIG_DIR / "proxy.log"
    try:
        subprocess.run(["open", "-a", "Console", str(log_path)], check=True)
        log.info(f"Opened logs: {log_path}")
        return 0
    except Exception as e:
        log.error(f"Failed to open logs: {e}"" )
        return 1

def cmd_open_config():
    """Open config folder in Finder."""
    try:
        subprocess.run(["open", str(CONFIG_DIR)], check=True)
        log.info(f"Opened config folder: {CONFIG_DIR}")
        return 0
    except Exception as e:
        log.error(f"Failed to open config: {e}"" )
        return 1

def cmd_set_api_key():
    """Set API key via dialog."""
    # This would be called by Swift settings dialog
    # For now, just show alert
    show_alert("Set API Key", "Use the settings dialog in the menu bar app", "informational")
    return 0

def main():
    if len(sys.argv) < 2:
        print("Usage: app_native.py <command>")
        print("Commands: start, stop, status, install, logs, config, set-key")
        return 1
    
    command = sys.argv[1]
    
    commands = {
        "start": cmd_start,
        "stop": cmd_stop,
        "status": cmd_status,
        "install": cmd_install,
        "logs": cmd_open_logs,
        "config": cmd_open_config,
        "set-key": cmd_set_api_key,
    }
    
    if command in commands:
        return commands[command]()
    else:
        print(f"Unknown command: {command}")
        print("Available: " + ", ".join(commands.keys()))
        return 1

if __name__ == "__main__":
    sys.exit(main())
