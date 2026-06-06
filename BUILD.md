# Codex Proxy — Native macOS Build Guide

## Requirements

- macOS 12.0+ 
- Xcode Command Line Tools (for swiftc)
- Python 3.10+

## Install Xcode Tools

xcode-select --install

## Build App

chmod +x build.sh
./build.sh

## Install

cp -r build/CodexProxy.app /Applications/
open /Applications/CodexProxy.app

## Manual Build (if build.sh fails)

mkdir -p CodexProxy.app/Contents/MacOS
mkdir -p CodexProxy.app/Contents/Resources

cp CodexProxyApp/Info.plist CodexProxy.app/Contents/

swiftc -o CodexProxy.app/Contents/MacOS/CodexProxy CodexProxyApp/CodexProxyMenuBar.swift -framework Cocoa

cp app_native.py CodexProxy.app/Contents/MacOS/
cp proxy.py CodexProxy.app/Contents/MacOS/
cp config.py CodexProxy.app/Contents/MacOS/

cp Scripts/*.sh CodexProxy.app/Contents/MacOS/
chmod +x CodexProxy.app/Contents/MacOS/*.sh

chmod +x CodexProxy.app/Contents/MacOS/CodexProxy

cp -r CodexProxy.app /Applications/

## Development

### Test Python Backend

python3 app_native.py status
python3 app_native.py start
python3 app_native.py stop

### Debug Swift App

swiftc -o test_app CodexProxyApp/CodexProxyMenuBar.swift -framework Cocoa
./test_app

## Troubleshooting

### Swift not found
Install Xcode Command Line Tools:
xcode-select --install

### Python modules missing
pip3 install -r requirements.txt

### App wont launch
Check Console.app for crash logs

## Auto-start (Optional)

Create LaunchAgent: ~/Library/LaunchAgents/com.codexproxy.plist

Load it:
launchctl load ~/Library/LaunchAgents/com.codexproxy.plist
