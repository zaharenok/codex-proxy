
# Codex Proxy — Native macOS Menu Bar App

## Структура

### Swift Menu Bar App (CodexProxyMenuBar.swift)
- Нативный menu bar на Swift/Cocoa
- Интеграция с System Settings
- Нативные нотификации
- Shortcuts поддержка

### Python Backend (app_native.py)
- Flask proxy server
- Config management
- API key storage

### Коммуникация
- Swift → Python через JSON API
- State sync через ~/.codexproxy/app_state.json
- Process management через Process/NSPipe

## Build

```bash
# Build Swift app
swiftc -o CodexProxy CodexProxyMenuBar.swift -framework Cocoa

# Или как .app bundle
swift build --configuration release
```

## Install

```bash
# Copy to /Applications
cp -r CodexProxy.app /Applications/

# Add to Login Items
defaults write loginwindow AutoLaunchArray -array-add "/Applications/CodexProxy.app"
```

## Features

✅ Нативный menu bar
✅ Нативные диалоги и нотификации  
✅ Shortcuts интеграция
✅ Auto-start через LaunchAgent
✅ Proper .app bundle
✅ Code signing (опционально)
