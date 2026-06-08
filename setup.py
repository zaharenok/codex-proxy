"""
py2app setup for packaging CodexProxy as macOS .app bundle.

Build:
    python setup.py py2app
"""

from setuptools import setup

APP = ["app.py"]
DATA_FILES = []
OPTIONS = {
    "argv_emulation": False,
    "iconfile": "icon.icns",
    "packages": ["flask", "requests", "rumps", "objc"],
    "includes": ["proxy", "config"],
    "plist": {
        "CFBundleName": "CodexProxy",
        "CFBundleDisplayName": "Codex Proxy",
        "CFBundleIdentifier": "com.codexproxy.app",
        "CFBundleVersion": "1.0.0",
        "CFBundleShortVersionString": "1.0.0",
        "LSUIElement": True,  # Menu bar only, no dock icon
        "LSMinimumSystemVersion": "13.0",
    },
}

setup(
    name="CodexProxy",
    app=APP,
    data_files=DATA_FILES,
    options={"py2app": OPTIONS},
    setup_requires=["py2app"],
)
