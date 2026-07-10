#!/usr/bin/env python3
"""
HTTPS interception for Codex Desktop.

Redirects api.openai.com → localhost via /etc/hosts + pf + socat SSL,
so ALL Codex Desktop traffic goes through our proxy on port 9090.

Callers: app.py (pywebview) and app_native.py (Swift backend).
"""

import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

SSL_DIR = Path.home() / ".codexproxy" / "ssl"
CERT_FILE = SSL_DIR / "api.openai.com.crt"
KEY_FILE = SSL_DIR / "api.openai.com.key"
HOSTS_ENTRY = "127.0.0.1 api.openai.com"
SOCAT_SSL_PORT = 9443  # socat SSL listener → proxy:9090
PF_ANCHOR = "codex-proxy"


def _ensure_cert():
    """Generate a self-signed cert for api.openai.com if missing."""
    SSL_DIR.mkdir(parents=True, exist_ok=True)
    if CERT_FILE.exists() and KEY_FILE.exists():
        # Check if cert is still valid
        r = subprocess.run(
            ["openssl", "x509", "-checkend", "2592000", "-in", str(CERT_FILE)],
            capture_output=True, text=True,
        )
        if r.returncode == 0:
            return  # cert still valid (30+ days)
    subprocess.run(
        [
            "openssl", "req", "-x509", "-newkey", "rsa:2048",
            "-keyout", str(KEY_FILE), "-out", str(CERT_FILE),
            "-days", "365", "-nodes",
            "-subj", "/CN=api.openai.com",
        ],
        check=True, capture_output=True,
    )
    KEY_FILE.chmod(0o600)
    print(f"[intercept] Generated SSL cert for api.openai.com")


def _install_cert():
    """Install the self-signed cert as trusted (system keychain)."""
    _ensure_cert()
    # Check if already installed
    r = subprocess.run(
        ["security", "find-certificate", "-c", "api.openai.com"],
        capture_output=True,
    )
    if r.returncode == 0 and b"api.openai.com" in r.stdout:
        print("[intercept] Cert already trusted")
        return True
    # Install — macOS will prompt for password via GUI
    r = subprocess.run([
        "sudo", "security", "add-trusted-cert", "-d", "-r", "trustRoot",
        "-k", "/Library/Keychains/System.keychain",
        str(CERT_FILE),
    ], capture_output=True, text=True)
    if r.returncode == 0:
        print("[intercept] Cert installed as trusted")
        return True
    print(f"[intercept] WARN: cert install failed: {r.stderr.strip()}")
    return False


def _ensure_hosts():
    """Add api.openai.com → 127.0.0.1 to /etc/hosts if missing."""
    try:
        with open("/etc/hosts") as f:
            content = f.read()
    except PermissionError:
        print("[intercept] Cannot read /etc/hosts (no permission)")
        return False
    if HOSTS_ENTRY in content:
        print("[intercept] hosts entry already present")
        return True
    # Append
    r = subprocess.run(
        ["sudo", "bash", "-c", f'echo "{HOSTS_ENTRY}" >> /etc/hosts'],
        capture_output=True, text=True,
    )
    if r.returncode == 0:
        print("[intercept] Added hosts entry")
        return True
    print(f"[intercept] WARN: hosts add failed: {r.stderr.strip()}")
    return False


def _remove_hosts():
    """Remove api.openai.com from /etc/hosts."""
    r = subprocess.run(
        ["sudo", "sed", "-i", "", f'/{HOSTS_ENTRY}/d', "/etc/hosts"],
        capture_output=True, text=True,
    )
    if r.returncode == 0:
        print("[intercept] Removed hosts entry")
        return True
    print(f"[intercept] WARN: hosts remove failed: {r.stderr.strip()}")
    return False


def _ensure_socat():
    """Start socat SSL → proxy forwarder if not already running."""
    # Check if socat is already listening on our port
    r = subprocess.run(
        ["lsof", "-i", f":{SOCAT_SSL_PORT}", "-P", "-n"],
        capture_output=True, text=True,
    )
    if "socat" in r.stdout:
        print(f"[intercept] socat already running on {SOCAT_SSL_PORT}")
        return True
    # Check socat is installed
    if not shutil.which("socat"):
        print("[intercept] ERROR: socat not found. Install: brew install socat")
        return False
    subprocess.Popen(
        [
            "socat",
            f"OPENSSL-LISTEN:{SOCAT_SSL_PORT}",
            f"cert={CERT_FILE}",
            f"key={KEY_FILE}",
            "verify=0", "fork", "reuseaddr",
            "TCP:127.0.0.1:9090",
        ],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    time.sleep(0.5)
    print(f"[intercept] socat started on :{SOCAT_SSL_PORT} → :9090")
    return True


def _kill_socat():
    """Kill socat processes."""
    r = subprocess.run(["pkill", "-f", "socat.*OPENSSL-LISTEN"], capture_output=True)
    if r.returncode == 0:
        print("[intercept] socat killed")
    else:
        print("[intercept] no socat to kill")


def _ensure_pf():
    """Set up pf redirect: port 443 → SOCAT_SSL_PORT."""
    # Create pf anchor rules
    rule = (
        f"rdr pass on lo0 inet proto tcp from any to 127.0.0.1 port 443 "
        f"-> 127.0.0.1 port {SOCAT_SSL_PORT}"
    )
    r = subprocess.run(
        ["sudo", "pfctl", "-a", PF_ANCHOR, "-f", "-"],
        input=rule + "\n", capture_output=True, text=True,
    )
    if r.returncode == 0:
        print(f"[intercept] pf redirect 443 → {SOCAT_SSL_PORT}")
        # Ensure pf is enabled
        subprocess.run(["sudo", "pfctl", "-e"], capture_output=True)
        return True
    print(f"[intercept] WARN: pf setup failed: {r.stderr.strip()}")
    return False


def _remove_pf():
    """Remove pf anchor."""
    subprocess.run(
        ["sudo", "pfctl", "-a", PF_ANCHOR, "-F", "all"],
        capture_output=True,
    )
    print("[intercept] pf anchor cleared")


def start_interception():
    """
    Set up full HTTPS interception:
    1. Generate + install SSL cert
    2. Add hosts entry
    3. Start socat SSL forwarder
    4. Set up pf redirect
    Returns True on success, False on failure.
    """
    _ensure_cert()

    # Install cert (asks for sudo password once)
    if not _install_cert():
        print("[intercept] ⚠ Cert install failed — interception may not work")

    ok = True
    if not _ensure_hosts():
        ok = False
    if not _ensure_socat():
        ok = False
    if not _ensure_pf():
        ok = False

    if ok:
        print("[intercept] ✅ HTTPS interception active — Codex Desktop routed through proxy")
    else:
        print("[intercept] ⚠ Some steps failed")
    return ok


def stop_interception():
    """
    Tear down HTTPS interception:
    1. Remove hosts entry
    2. Kill socat
    3. Remove pf rules
    """
    _remove_hosts()
    _kill_socat()
    _remove_pf()
    print("[intercept] ✅ Interception stopped — Codex Desktop goes direct again")


if __name__ == "__main__":
    if len(sys.argv) > 1:
        if sys.argv[1] == "start":
            start_interception()
        elif sys.argv[1] == "stop":
            stop_interception()
        else:
            print("Usage: intercept.py [start|stop]")
    else:
        print("Usage: intercept.py [start|stop]")
