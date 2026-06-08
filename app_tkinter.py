#!/usr/bin/env python3
"""
Codex Proxy — macOS Desktop App.

Polished Tkinter window to configure and run the proxy server.
"""

import os
import subprocess
import sys
import tkinter as tk
from tkinter import messagebox, ttk

from config import install_codex_config, load_api_key, load_settings, save_api_key, save_settings
from proxy import PRESETS, configure, start_server, stop_server

# ── Color palette (Catppuccin Mocha) ────────────────────────────────────────
BG          = "#1e1e2e"
SURFACE     = "#313244"
SURFACE2    = "#45475a"
OVERLAY     = "#6c7086"
TEXT        = "#cdd6f4"
SUBTEXT     = "#a6adc8"
BLUE        = "#89b4fa"
GREEN       = "#a6e3a1"
RED         = "#f38ba8"
YELLOW      = "#f9e2af"
PEACH       = "#fab387"


class CodexProxyApp:
    def __init__(self):
        self.root = tk.Tk()
        self.root.title("Codex Proxy")
        self.root.geometry("480x560")
        self.root.resizable(False, False)
        self.root.configure(bg=BG)
        self.root.attributes("-transparent", False)

        # Remove default title bar styling
        try:
            self.root.tk.call("tk", "scaling", 2.0)
        except Exception:
            pass

        self.settings = load_settings()
        self._is_running = False
        self._build_ui()
        self._load_settings_to_ui()
        self._refresh_status()

    # ── Build UI ──────────────────────────────────────────────────────────────

    def _build_ui(self):
        # Header
        header = tk.Frame(self.root, bg=BG)
        header.pack(fill="x", padx=28, pady=(24, 0))

        tk.Label(header, text="⚡", font=("Apple Color Emoji", 28), bg=BG, fg=BLUE).pack(side="left")
        title_frame = tk.Frame(header, bg=BG)
        title_frame.pack(side="left", padx=(12, 0))
        tk.Label(title_frame, text="Codex Proxy", font=("SF Pro Display", 22, "bold"), bg=BG, fg=TEXT).pack(anchor="w")
        tk.Label(title_frame, text="Responses API ↔ Chat Completions / Anthropic", font=("SF Pro", 10), bg=BG, fg=SUBTEXT).pack(anchor="w")

        # Separator
        tk.Frame(self.root, bg=SURFACE2, height=1).pack(fill="x", padx=28, pady=(16, 0))

        # Main form
        form = tk.Frame(self.root, bg=BG)
        form.pack(fill="x", padx=28, pady=(16, 0))

        # Provider
        self._label(form, "PROVIDER").grid(row=0, column=0, sticky="w", pady=(0, 2))
        self.preset_var = tk.StringVar()
        preset_names = list(PRESETS.keys()) + ["Custom"]
        self.preset_combo = tk.OptionMenu(form, self.preset_var, *preset_names, command=self._on_preset_change)
        self.preset_combo.configure(
            font=("SF Pro", 12), bg=SURFACE, fg=TEXT, activebackground=SURFACE2,
            activeforeground=TEXT, highlightthickness=0, relief="flat", bd=0,
            anchor="w", cursor="hand2",
        )
        self.preset_combo["highlightbackground"] = SURFACE
        self.preset_combo["menu"].configure(bg=SURFACE, fg=TEXT, activebackground=SURFACE2, activeforeground=TEXT)
        self.preset_combo.grid(row=1, column=0, sticky="ew", pady=(0, 12), ipady=6)

        # Upstream URL
        self._label(form, "UPSTREAM URL").grid(row=2, column=0, sticky="w", pady=(0, 2))
        self.upstream_var = tk.StringVar()
        self.upstream_entry = self._entry(form, self.upstream_var)
        self.upstream_entry.grid(row=3, column=0, sticky="ew", pady=(0, 12), ipady=6)

        # API Key
        self._label(form, "API KEY").grid(row=4, column=0, sticky="w", pady=(0, 2))
        key_frame = tk.Frame(form, bg=SURFACE, highlightthickness=1, highlightbackground=SURFACE2)
        key_frame.grid(row=5, column=0, sticky="ew", pady=(0, 12))
        self.key_var = tk.StringVar()
        self.key_entry = tk.Entry(
            key_frame, textvariable=self.key_var, show="•",
            font=("SF Mono", 12), bg=SURFACE, fg=TEXT, insertbackground=TEXT,
            relief="flat", bd=0, highlightthickness=0,
        )
        self.key_entry.pack(side="left", fill="x", expand=True, padx=(12, 4), pady=8)
        self._toggle_btn = tk.Button(
            key_frame, text="👁", font=("Apple Color Emoji", 14), bg=SURFACE, fg=SUBTEXT,
            relief="flat", bd=0, cursor="hand2", activebackground=SURFACE2,
            command=self._toggle_key_visibility,
        )
        self._toggle_btn.pack(side="right", padx=(4, 8))
        self._key_visible = False

        # Port
        port_label_frame = tk.Frame(form, bg=BG)
        port_label_frame.grid(row=6, column=0, sticky="w", pady=(0, 2))
        self._label(port_label_frame, "PORT").pack(side="left")

        self.port_var = tk.StringVar(value="9090")
        self.port_entry = self._entry(form, self.port_var, width=8)
        self.port_entry.grid(row=7, column=0, sticky="w", pady=(0, 8), ipady=6)

        form.columnconfigure(0, weight=1)

        # Status bar
        self.status_frame = tk.Frame(self.root, bg=SURFACE, padx=16, pady=10)
        self.status_frame.pack(fill="x", padx=28, pady=(12, 0))

        self.status_dot = tk.Canvas(self.status_frame, width=10, height=10, bg=SURFACE, highlightthickness=0)
        self.status_dot.pack(side="left", padx=(0, 8))
        self._dot_id = self.status_dot.create_oval(1, 1, 10, 10, fill=OVERLAY, outline="")

        self.status_var = tk.StringVar(value="Stopped")
        tk.Label(self.status_frame, textvariable=self.status_var, font=("SF Pro", 11), bg=SURFACE, fg=SUBTEXT).pack(side="left")

        # Buttons row
        btn_frame = tk.Frame(self.root, bg=BG)
        btn_frame.pack(fill="x", padx=28, pady=(16, 0))

        self.start_btn = self._button(btn_frame, "▶  Start", GREEN, self._on_start)
        self.start_btn.pack(side="left", expand=True, fill="x", padx=(0, 6), ipady=6)

        self.stop_btn = self._button(btn_frame, "⏹  Stop", RED, self._on_stop)
        self.stop_btn.pack(side="left", expand=True, fill="x", padx=(6, 0), ipady=6)
        self.stop_btn.configure(state="disabled", bg=SURFACE2)

        # Bottom links
        bottom = tk.Frame(self.root, bg=BG)
        bottom.pack(fill="x", padx=28, pady=(16, 20))

        self._link_button(bottom, "📋 Install Codex Config", self._on_install_config).pack(side="left", padx=(0, 16))
        self._link_button(bottom, "📖 Logs", self._on_open_logs).pack(side="left", padx=(0, 16))
        self._link_button(bottom, "⚙ Config", self._on_open_config).pack(side="left")

    # ── Widget helpers ────────────────────────────────────────────────────────

    def _label(self, parent, text):
        return tk.Label(parent, text=text, font=("SF Pro", 9, "bold"), bg=BG, fg=OVERLAY)

    def _entry(self, parent, var, **kw):
        return tk.Entry(
            parent, textvariable=var,
            font=("SF Pro", 12), bg=SURFACE, fg=TEXT, insertbackground=BLUE,
            relief="flat", highlightthickness=1, highlightbackground=SURFACE2,
            highlightcolor=BLUE, **kw,
        )

    def _button(self, parent, text, color, cmd):
        return tk.Button(
            parent, text=text, font=("SF Pro", 13, "bold"),
            bg=color, fg="#1e1e2e", activebackground=color,
            relief="flat", bd=0, cursor="hand2", command=cmd,
        )

    def _link_button(self, parent, text, cmd):
        return tk.Button(
            parent, text=text, font=("SF Pro", 10),
            bg=BG, fg=SUBTEXT, activebackground=BG, activeforeground=TEXT,
            relief="flat", bd=0, cursor="hand2", command=cmd,
        )

    def _toggle_key_visibility(self):
        self._key_visible = not self._key_visible
        self.key_entry.configure(show="" if self._key_visible else "•")

    # ── Settings ──────────────────────────────────────────────────────────────

    def _load_settings_to_ui(self):
        preset = self.settings.get("preset", "DeepSeek V4 Pro")
        self.preset_var.set(preset)
        self.upstream_var.set(self.settings.get("upstream", "https://api.deepseek.com"))
        self.key_var.set(load_api_key())
        self.port_var.set(str(self.settings.get("port", 9090)))

    def _on_preset_change(self, _=None):
        name = self.preset_var.get()
        if name in PRESETS:
            self.upstream_var.set(PRESETS[name]["url"])

    def _save_current_settings(self):
        self.settings["preset"] = self.preset_var.get()
        self.settings["upstream"] = self.upstream_var.get().strip()
        self.settings["port"] = int(self.port_var.get().strip() or "9090")

        name = self.preset_var.get()
        if name in PRESETS:
            self.settings["model_map"] = PRESETS[name]["models"]
            self.settings["api_type"] = PRESETS[name].get("api_type", "openai")
        else:
            self.settings["api_type"] = "anthropic" if "/anthropic" in self.settings["upstream"] else "openai"

        save_settings(self.settings)

        key = self.key_var.get().strip()
        if key:
            save_api_key(key)

    # ── Actions ───────────────────────────────────────────────────────────────

    def _on_start(self):
        if self._is_running:
            return

        key = self.key_var.get().strip()
        if not key:
            messagebox.showwarning("No API Key", "Enter your API key first.")
            return

        self._save_current_settings()

        upstream = self.settings["upstream"]
        port = self.settings["port"]
        model_map = self.settings.get("model_map", {})
        api_type = self.settings.get("api_type", "openai")

        configure(upstream, model_map, key, api_type)
        start_server(port=port)
        self._is_running = True

        self.start_btn.configure(state="disabled", bg=SURFACE2)
        self.stop_btn.configure(state="normal", bg=RED)
        self.status_var.set(f"Running → localhost:{port} → {upstream}")
        self.status_dot.itemconfig(self._dot_id, fill=GREEN)

    def _on_stop(self):
        if not self._is_running:
            return
        stop_server()
        self._is_running = False
        self.start_btn.configure(state="normal", bg=GREEN)
        self.stop_btn.configure(state="disabled", bg=SURFACE2)
        self.status_var.set("Stopped")
        self.status_dot.itemconfig(self._dot_id, fill=OVERLAY)

    def _on_install_config(self):
        port = int(self.port_var.get().strip() or "9090")
        try:
            install_codex_config(port)
            messagebox.showinfo("Installed", "Codex config written to ~/.codex/config.toml")
        except Exception as e:
            messagebox.showerror("Error", str(e))

    def _on_open_logs(self):
        log_path = os.path.expanduser("~/.codexproxy/proxy.log")
        subprocess.run(["open", "-a", "Console", log_path])

    def _on_open_config(self):
        subprocess.run(["open", os.path.expanduser("~/.codexproxy")])

    def _refresh_status(self):
        if self._is_running:
            try:
                import urllib.request
                urllib.request.urlopen(f"http://127.0.0.1:{self.settings.get('port', 9090)}/health", timeout=2)
            except Exception:
                self._is_running = False
                self.start_btn.configure(state="normal", bg=GREEN)
                self.stop_btn.configure(state="disabled", bg=SURFACE2)
                self.status_var.set("Connection lost")
                self.status_dot.itemconfig(self._dot_id, fill=RED)
        self.root.after(5000, self._refresh_status)

    def run(self):
        self.root.mainloop()


def main():
    app = CodexProxyApp()
    app.run()


if __name__ == "__main__":
    main()
