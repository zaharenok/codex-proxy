#!/usr/bin/env python3
"""
Codex Proxy — pywebview Desktop App.

Uses HTML/CSS/JS UI rendered via macOS WebKit.
"""

import json
import os
import pickle
import subprocess
import threading

import requests as http_requests
import webview

from config import install_codex_config, load_api_key, load_settings, restore_original_config, save_api_key, save_settings
from proxy import PRESETS, configure, start_server, stop_server


class Api:
    """Bridge between JS frontend and Python backend."""

    def __init__(self, window):
        self.window = window
        self._key_cache = {}
        self._key_cache_path = os.path.expanduser("~/.codexproxy/.key_cache.pkl")
        self._load_key_cache()

    def _load_key_cache(self):
        try:
            if os.path.exists(self._key_cache_path):
                with open(self._key_cache_path, "rb") as f:
                    self._key_cache = pickle.load(f)
        except Exception:
            self._key_cache = {}

    def _save_key_cache(self):
        try:
            os.makedirs(os.path.dirname(self._key_cache_path), exist_ok=True)
            with open(self._key_cache_path, "wb") as f:
                pickle.dump(self._key_cache, f)
            os.chmod(self._key_cache_path, 0o600)
        except Exception:
            pass

    def store_key(self, preset, api_key):
        self._key_cache[preset] = api_key
        self._save_key_cache()
        return {"ok": True}

    def get_key(self, preset):
        key = self._key_cache.get(preset)
        if not key and not self._key_cache:
            key = load_api_key()
        return key or ""

    def load_settings(self):
        s = load_settings()
        return {
            "preset": s.get("preset", "z.ai"),
            "upstream": s.get("upstream", "https://api.z.ai/api/anthropic"),
            "port": s.get("port", 9090),
            "selected_model": s.get("selected_model"),
        }

    def _models_base(self, preset):
        p = PRESETS.get(preset)
        if not p:
            return None
        return (p.get("models_url") or p["url"]).rstrip("/")

    def validate_key(self, preset, api_key):
        base = self._models_base(preset)
        if not base:
            return {"valid": False, "error": "Unknown preset"}
        try:
            resp = http_requests.get(
                f"{base}/models",
                headers={"Authorization": f"Bearer {api_key}"},
                timeout=10
            )
            return {"valid": resp.status_code == 200, "status": resp.status_code}
        except Exception as e:
            return {"valid": False, "error": str(e)}

    def fetch_models(self, preset, api_key):
        base = self._models_base(preset)
        if not base:
            return {"models": [], "error": "Unknown preset"}
        try:
            resp = http_requests.get(
                f"{base}/models",
                headers={"Authorization": f"Bearer {api_key}"},
                timeout=15
            )
            if resp.status_code == 200:
                data = resp.json()
                models = sorted(m["id"] for m in data.get("data", []))
                return {"models": models}
            return {"models": [], "error": f"HTTP {resp.status_code}"}
        except Exception as e:
            return {"models": [], "error": str(e)}

    def start_proxy(self, preset, upstream, api_key, port, selected_model=None):
        if not api_key:
            return {"error": "No API key provided"}

        model_map = {}
        api_type = "openai"

        if preset in PRESETS:
            p = PRESETS[preset]
            upstream = p["url"]
            model_map = dict(p["models"])
            api_type = p.get("api_type", "openai")
        else:
            api_type = "anthropic" if "/anthropic" in upstream else "openai"

        if selected_model and model_map:
            for k in model_map:
                model_map[k] = selected_model

        configure(upstream, model_map, api_key, api_type)
        start_server(port=port)

        # Persist settings
        settings = {
            "preset": preset,
            "upstream": upstream,
            "port": port,
            "model_map": model_map,
            "api_type": api_type,
            "selected_model": selected_model,
        }
        save_settings(settings)
        save_api_key(api_key)

        # Update bearer token in config.toml (merge-safe)
        from config import update_api_key_in_config, install_codex_config
        if not update_api_key_in_config(api_key):
            install_codex_config(port, api_key)

        # Make API key available as env var for Codex Desktop App
        os.environ["CODEX_PROXY_API_KEY"] = api_key
        try:
            subprocess.run(["launchctl", "setenv", "CODEX_PROXY_API_KEY", api_key], capture_output=True, timeout=5)
        except Exception:
            pass

        return {"ok": True, "upstream": upstream}

    def stop_proxy(self):
        stop_server()
        restore_original_config()
        os.environ.pop("CODEX_PROXY_API_KEY", None)
        try:
            subprocess.run(["launchctl", "unsetenv", "CODEX_PROXY_API_KEY"], capture_output=True, timeout=5)
        except Exception:
            pass
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

    def launch_codex(self):
        """Launch Codex CLI with proxy profile, bypassing GUI subscription check."""
        script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "launch-codex.sh")
        key = load_api_key()
        env = os.environ.copy()
        env["CODEX_PROXY_API_KEY"] = key
        subprocess.Popen(
            ["open", "-a", "Terminal", script],
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return {"ok": True}

    def launch_codex_app(self):
        """Launch Codex native macOS app."""
        subprocess.Popen(
            ["open", "/Applications/Codex.app"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return {"ok": True}

    def restore_config(self):
        from config import restore_config as do_restore
        msg = do_restore()
        has_backup = "No backup" not in msg
        return {"ok": has_backup, "message": msg}

    def restore_original_config(self):
        from config import restore_original_config as do_restore
        msg = do_restore()
        return {"ok": "Restored" in msg, "message": msg}

    def save_original_config(self):
        from config import save_original_config as do_save
        msg = do_save()
        return {"ok": True, "message": msg}


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
    window.expose(api.load_settings, api.start_proxy, api.stop_proxy, api.install_config, api.restore_config, api.open_config, api.launch_codex, api.launch_codex_app, api.validate_key, api.fetch_models, api.restore_original_config, api.save_original_config, api.store_key, api.get_key)

    webview.start(debug=False)


if __name__ == "__main__":
    main()
