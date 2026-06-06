import Cocoa
import Foundation
import UserNotifications

@main
struct CodexProxyApp {
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

    // Use Bundle.main to find resources inside the app bundle
    let bundleDir: String = {
        Bundle.main.bundlePath + "/Contents/MacOS"
    }()

    lazy var pythonPath: String = bundleDir + "/app_native.py"
    lazy var configDir: URL = {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".codexproxy")
    }()

    func applicationDidFinishLaunching(_ notification: Notification) {
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

        let statusMenuItem = NSMenuItem(title: "Status: Checking...", action: nil, keyEquivalent: "")
        statusMenuItem.tag = 100
        statusMenuItem.isEnabled = false
        menu.addItem(statusMenuItem)

        menu.addItem(NSMenuItem.separator())

        menu.addItem(withTitle: "▶ Start Proxy", action: #selector(startProxy), keyEquivalent: "s")
        menu.addItem(withTitle: "⏹ Stop Proxy", action: #selector(stopProxy), keyEquivalent: "x")

        menu.addItem(NSMenuItem.separator())

        menu.addItem(withTitle: "⚙ Settings...", action: #selector(showSettings), keyEquivalent: ",")
        menu.addItem(withTitle: "📋 Install Codex Config", action: #selector(installConfig), keyEquivalent: "i")

        menu.addItem(NSMenuItem.separator())

        menu.addItem(withTitle: "📖 Open Logs", action: #selector(openLogs), keyEquivalent: "l")
        menu.addItem(withTitle: "📂 Config Folder", action: #selector(openConfig), keyEquivalent: "c")

        menu.addItem(NSMenuItem.separator())

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

        let stateFile = configDir.appendingPathComponent("app_state.json")

        if let data = try? Data(contentsOf: stateFile),
           let state = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let isRunning = state["proxy_running"] as? Bool {

            let statusMenuItem = menu.item(withTag: 100)
            if isRunning {
                statusMenuItem?.title = "● Running"
                if let button = self.statusItem?.button {
                    button.image = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "Running")
                }
            } else {
                statusMenuItem?.title = "○ Stopped"
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
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 400, height: 300),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )

        window.title = "Codex Proxy Settings"
        window.center()

        let contentView = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        window.contentView = contentView

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
        // Open log file directly in Console.app
        let logPath = configDir.appendingPathComponent("proxy.log")
        // Also touch it so it exists
        if !FileManager.default.fileExists(atPath: logPath.path) {
            try? Data().write(to: logPath)
        }
        NSWorkspace.shared.open(logPath)
    }

    @objc func openConfig() {
        // Ensure dir exists then open in Finder
        try? FileManager.default.createDirectory(at: configDir, withIntermediateDirectories: true)
        NSWorkspace.shared.open(configDir)
    }

    @objc func showAbout() {
        let alert = NSAlert()
        alert.messageText = "CodexProxy"
        alert.informativeText = """
Native macOS Menu Bar App

Version: 1.0.0
Proxy: OpenAI Responses \u{2192} Chat Completions

Saves 30-50x on API costs by routing through DeepSeek/GLM
"""
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }

    @objc func quitApp() {
        executePython(command: "stop")
        NSApplication.shared.terminate(self)
    }

    func executePython(command: String) {
        let task = Process()
        task.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        task.arguments = [pythonPath, command]
        // Set working directory to bundle so Python can find config.py / proxy.py
        task.currentDirectoryURL = URL(fileURLWithPath: bundleDir)

        do {
            try task.run()
        } catch {
            showNotification(title: "Error", message: "Failed to execute: \(error.localizedDescription)")
        }
    }

    func showNotification(title: String, message: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = message
        content.sound = .default
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
