
#!/usr/bin/env swift
// Codex Proxy — Native macOS Menu Bar App
// Pure Swift with Cocoa bindings

import Cocoa
import Foundation

class CodexProxyApp: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem?
    var proxyProcess: Process?
    var stateFile: URL {
        let paths = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)
        return paths[0].appendingPathComponent(".codexproxy/app_state.json")
    }
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Create status item in menu bar
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        
        if let button = statusItem?.button {
            button.image = NSImage(systemSymbolName: "bolt.fill", accessibilityDescription: "Codex Proxy")
            button.image?.isTemplate = true  // Makes it adapt to light/dark mode
        }
        
        setupMenu()
    }
    
    func setupMenu() {
        let menu = NSMenu()
        
        menu.addItem(NSMenuItem.separator())
        
        // Proxy controls
        let startItem = NSMenuItem(title: "Start Proxy", action: #selector(startProxy), keyEquivalent: "s")
        startItem.target = self
        menu.addItem(startItem)
        
        let stopItem = NSMenuItem(title: "Stop Proxy", action: #selector(stopProxy), keyEquivalent: "x")
        stopItem.target = self
        menu.addItem(stopItem)
        
        menu.addItem(NSMenuItem.separator())
        
        // Configuration
        let settingsItem = NSMenuItem(title: "Settings...", action: #selector(showSettings), keyEquivalent: ",")
        settingsItem.target = self
        menu.addItem(settingsItem)
        
        let installItem = NSMenuItem(title: "Install Codex Config", action: #selector(installConfig), keyEquivalent: "i")
        installItem.target = self
        menu.addItem(installItem)
        
        menu.addItem(NSMenuItem.separator())
        
        // Utilities
        let logsItem = NSMenuItem(title: "Open Logs", action: #selector(openLogs), keyEquivalent: "l")
        logsItem.target = self
        menu.addItem(logsItem)
        
        let configItem = NSMenuItem(title: "Open Config Folder", action: #selector(openConfig), keyEquivalent: "c")
        configItem.target = self
        menu.addItem(configItem)
        
        menu.addItem(NSMenuItem.separator())
        
        // Quit
        let quitItem = NSMenuItem(title: "Quit CodexProxy", action: #selector(quit), keyEquivalent: "q")
        quitItem.target = self
        menu.addItem(quitItem)
        
        statusItem?.menu = menu
    }
    
    @objc func startProxy() {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/local/bin/python3")
        task.arguments = ["/path/to/codex_proxy/app_native.py", "start"]
        
        do {
            try task.run()
            proxyProcess = task
            showNotification(title: "Proxy Started", message: "Codex Proxy is running")
        } catch {
            showAlert(title: "Error", message: "Failed to start proxy: \(error.localizedDescription)")
        }
    }
    
    @objc func stopProxy() {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/local/bin/python3")
        task.arguments = ["/path/to/codex_proxy/app_native.py", "stop"]
        
        do {
            try task.run()
            proxyProcess = nil
            showNotification(title: "Proxy Stopped", message: "Codex Proxy stopped")
        } catch {
            showAlert(title: "Error", message: "Failed to stop proxy: \(error.localizedDescription)")
        }
    }
    
    @objc func showSettings() {
        // Show native settings window
        let alert = NSAlert()
        alert.messageText = "Settings"
        alert.informativeText = "Configure your Codex Proxy settings"
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
    
    @objc func installConfig() {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/local/bin/python3")
        task.arguments = ["/path/to/codex_proxy/app_native.py", "install"]
        
        do {
            try task.run()
            showNotification(title: "Config Installed", message: "Codex config installed successfully")
        } catch {
            showAlert(title: "Error", message: "Failed to install config: \(error.localizedDescription)")
        }
    }
    
    @objc func openLogs() {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        task.arguments = ["-a", "Console", "~/.codexproxy/proxy.log"]
        task.launch()
    }
    
    @objc func openConfig() {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        task.arguments = ["~/.codexproxy"]
        task.launch()
    }
    
    @objc func quit() {
        NSApplication.shared.terminate(self)
    }
    
    func showNotification(title: String, message: String) {
        let notification = NSUserNotification()
        notification.title = title
        notification.informativeText = message
        NSUserNotificationCenter.default.deliver(notification)
    }
    
    func showAlert(title: String, message: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
}

// Start the app
let app = NSApplication.shared
let delegate = CodexProxyApp()
app.delegate = delegate
app.run()
