#!/bin/bash
# Build script for CodexProxy macOS app

set -e

echo "Building CodexProxy.app..."

# Clean build directory
rm -rf build
mkdir -p build/CodexProxy.app/Contents/MacOS
mkdir -p build/CodexProxy.app/Contents/Resources

# Copy Info.plist
cp CodexProxyApp/Info.plist build/CodexProxy.app/Contents/

# Compile Swift app
echo "Compiling Swift..."
swiftc -o build/CodexProxy.app/Contents/MacOS/CodexProxy     CodexProxyApp/CodexProxyMenuBar.swift     -framework Cocoa

# Copy Python files
echo "Copying Python files..."
cp app_native.py build/CodexProxy.app/Contents/MacOS/
cp proxy.py build/CodexProxy.app/Contents/MacOS/
cp config.py build/CodexProxy.app/Contents/MacOS/

# Copy scripts
cp Scripts/start_proxy.sh build/CodexProxy.app/Contents/MacOS/
cp Scripts/stop_proxy.sh build/CodexProxy.app/Contents/MacOS/
chmod +x build/CodexProxy.app/Contents/MacOS/*.sh

# Set permissions
chmod +x build/CodexProxy.app/Contents/MacOS/CodexProxy

echo "Build complete: build/CodexProxy.app"
echo "Install: cp -r build/CodexProxy.app /Applications/"
