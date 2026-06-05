#!/usr/bin/env python3
"""
Codex Proxy — pywebview Desktop App.

Uses HTML/CSS/JS UI rendered via macOS WebKit.
"""

import json
import os
import subprocess
import threading

import webview

from config import install_codex_config, load_api_key, load_settings, save_api_key, save_settings
from proxy import PRESETS, configure, start_server, stop_server


class Api:
    """Bridge between JS frontend and Python backend."""

    def __init__(self, window):
        self.window = window

    def load_settings(self):
        s = load_settings()
        key = load_api_key()
        return {
            "preset": s.get("preset", "z.ai"),
            "upstream": s.get("upstream", "https://api.z.ai/api/anthropic"),
            "port": s.get("port", 9090),
            "api_key": key,
        }

    def start_proxy(self, preset, upstream, api_key, port):
        if not api_key:
            return {"error": "No API key provided"}

        model_map = {}
        api_type = "openai"

        if preset in PRESETS:
            p = PRESETS[preset]
            upstream = p["url"]
            model_map = p["models"]
            api_type = p.get("api_type", "openai")
        else:
            api_type = "anthropic" if "/anthropic" in upstream else "openai"

        configure(upstream, model_map, api_key, api_type)
        start_server(port=port)

        # Persist settings
        settings = {
            "preset": preset,
            "upstream": upstream,
            "port": port,
            "model_map": model_map,
            "api_type": api_type,
        }
        save_settings(settings)
        save_api_key(api_key)

        # Update bearer token in config.toml (merge-safe)
        from config import update_api_key_in_config, install_codex_config
        if not update_api_key_in_config(api_key):
            install_codex_config(port, api_key)

        return {"ok": True, "upstream": upstream}

    def stop_proxy(self):
        stop_server()
        return {"ok": True}

    def install_config(self, port):
        try:
            key = load_api_key()
            install_codex_config(port, key)
            return {"ok": True, "message": f"Proxy provider merged into config (port {port})"}
        except Exception as e:
            return {"ok": False, "message": str(e)}

    def open_config(self):
        subprocess.run(["open", os.path.expanduser("~/.codexproxy")])
        return {"ok": True}

    def restore_config(self):
        from config import restore_config as do_restore
        msg = do_restore()
        has_backup = "No backup" not in msg
        return {"ok": has_backup, "message": msg}


def main():
    html_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "codex-proxy-redesign.html")

    window = webview.create_window(
        title="Codex Proxy",
        url=html_path,
        width=520,
        height=680,
        resizable=False,
        background_color="#111111",
    )

    api = Api(window)
    window.expose(api.load_settings, api.start_proxy, api.stop_proxy, api.install_config, api.restore_config, api.open_config)

    webview.start(debug=False)


if __name__ == "__main__":
    main()
