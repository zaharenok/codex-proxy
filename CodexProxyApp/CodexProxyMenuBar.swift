#!/usr/bin/env swift
import Cocoa
import Foundation

@main
struct CodexProxyMenuBar {
    static func main() {
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem?
    var stateTimer: Timer?
    
    // Paths
    let projectDir: String = {
        let path = "\(#file)"
        return (path as NSString).deletingLastPathComponent.deletingLastPathComponent
    }()
    
    lazy var pythonPath: String = "\(projectDir)/app_native.py"
    lazy var configDir: URL = {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(".codexproxy")
    }()
    
    func applicationDidFinishLaunching(_ notification: Notification) {
        // Setup status bar item
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        
        if let button = statusItem?.button {
            button.image = NSImage(systemSymbolName: "bolt.fill", accessibilityDescription: "Codex Proxy")
            button.image?.isTemplate = true
        }
        
        setupMenu()
        startStateTimer()
    }
    
    func setupMenu() {
        let menu = NSMenu()
        
        // Status item (dynamic)
        let statusItem = NSMenuItem(title: "Status: Checking...", action: nil, keyEquivalent: "")
        statusItem.tag = 100
        statusItem.isEnabled = false
        menu.addItem(statusItem)
        
        menu.addItem(NSMenuItem.separator())
        
        // Proxy controls
        menu.addItem(withTitle: "▶ Start Proxy", action: #selector(startProxy), keyEquivalent: "s")
        menu.addItem(withTitle: "⏹ Stop Proxy", action: #selector(stopProxy), keyEquivalent: "x")
        
        menu.addItem(NSMenuItem.separator())
        
        // Configuration
        menu.addItem(withTitle: "⚙ Settings...", action: #selector(showSettings), keyEquivalent: ",")
        menu.addItem(withTitle: "📋 Install Codex Config", action: #selector(installConfig), keyEquivalent: "i")
        
        menu.addItem(NSMenuItem.separator())
        
        // Utilities
        menu.addItem(withTitle: "📖 Open Logs", action: #selector(openLogs), keyEquivalent: "l")
        menu.addItem(withTitle: "📂 Config Folder", action: #selector(openConfig), keyEquivalent: "c")
        
        menu.addItem(NSMenuItem.separator())
        
        // About and quit
        menu.addItem(withTitle: "About CodexProxy", action: #selector(showAbout), keyEquivalent: "")
        menu.addItem(withTitle: "Quit CodexProxy", action: #selector(quitApp), keyEquivalent: "q")
        
        statusItem?.menu = menu
    }
    
    func startStateTimer() {
        stateTimer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
            self?.updateStatus()
        }
    }
    
    func updateStatus() {
        guard let menu = statusItem?.menu else { return }
        
        // Check proxy status by reading state file
        let stateFile = configDir.appendingPathComponent("app_state.json")
        
        if let data = try? Data(contentsOf: stateFile),
           let state = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let isRunning = state["proxy_running"] as? Bool {
            
            let statusItem = menu.item(withTag: 100)
            if isRunning {
                statusItem?.title = "● Running"
                if let button = self.statusItem?.button {
                    button.image = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "Running")
                }
            } else {
                statusItem?.title = "○ Stopped"
                if let button = self.statusItem?.button {
                    button.image = NSImage(systemSymbolName: "bolt.fill", accessibilityDescription: "Stopped")
                }
            }
        }
    }
    
    @objc func startProxy() {
        executePython(command: "start")
    }
    
    @objc func stopProxy() {
        executePython(command: "stop")
    }
    
    @objc func showSettings() {
        // Create settings window
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        
        window.title = "Codex Proxy Settings"
        window.center()
        
        // Create settings view
        let contentView = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        window.contentView = contentView
        
        // Add settings controls here...
        let label = NSTextField(labelWithString: "Settings - Work in Progress")
        label.frame = NSRect(x: 20, y: 250, width: 360, height: 20)
        contentView.addSubview(label)
        
        let close = NSButton(frame: NSRect(x: 300, y: 10, width: 80, height: 30))
        close.title = "Close"
        close.bezelStyle = .rounded
        close.target = window
        close.action = #selector(NSWindow.close)
        contentView.addSubview(close)
        
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
    
    @objc func installConfig() {
        executePython(command: "install")
        showNotification(title: "Config Installed", message: "Codex config installed successfully")
    }
    
    @objc func openLogs() {
        NSWorkspace.shared.open(FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent(".codexproxy/proxy.log"))
    }
    
    @objc func openConfig() {
        NSWorkspace.shared.open(configDir)
    }
    
    @objc func showAbout() {
        let alert = NSAlert()
        alert.messageText = "CodexProxy"
        alert.informativeText = """Native macOS Menu Bar App
        
Version: 1.0.0
Proxy: OpenAI Responses → Chat Completions

Saves 30-50x on API costs by routing through DeepSeek/GLM"""
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
    
    @objc func quitApp() {
        // Stop proxy before quitting
        executePython(command: "stop")
        NSApplication.shared.terminate(self)
    }
    
    func executePython(command: String) {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        task.arguments = [pythonPath, command]
        
        do {
            try task.run()
        } catch {
            showNotification(title: "Error", message: "Failed to execute: \(error.localizedDescription)")
        }
    }
    
    func showNotification(title: String, message: String) {
        let notification = NSUserNotification()
        notification.title = title
        notification.informativeText = message
        notification.soundName = NSUserNotificationDefaultSoundName
        NSUserNotificationCenter.default.deliver(notification)
    }
}
