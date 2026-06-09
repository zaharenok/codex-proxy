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

// MARK: - Theme

struct T {
    static let bg       = NSColor(red: 0.067, green: 0.067, blue: 0.067, alpha: 1)
    static let surface  = NSColor(red: 0.102, green: 0.102, blue: 0.102, alpha: 1)
    static let surface2 = NSColor(red: 0.145, green: 0.145, blue: 0.145, alpha: 1)
    static let fg       = NSColor(red: 0.910, green: 0.910, blue: 0.910, alpha: 1)
    static let muted    = NSColor(red: 0.533, green: 0.533, blue: 0.533, alpha: 1)
    static let border   = NSColor(red: 0.800, green: 0.800, blue: 0.800, alpha: 1)
    static let accent   = NSColor(red: 0.784, green: 1.000, blue: 0.000, alpha: 1)
    static let danger   = NSColor(red: 1.000, green: 0.267, blue: 0.267, alpha: 1)
    static let bw: CGFloat = 2
}

// MARK: - Settings Window

class SettingsWindowController: NSObject, NSWindowDelegate {
    var window: NSWindow!

    var presetPopup: NSPopUpButton!
    var upstreamField: NSTextField!
    var apiKeyField: NSSecureTextField!
    var apiKeyVisibleField: NSTextField!
    var showKeyBtn: NSButton!
    var validateBtn: NSButton!
    var validateLabel: NSTextField!
    var modelPopup: NSPopUpButton!
    var portField: NSTextField!
    var statusDot: NSView!
    var statusLabel: NSTextField!
    var statusPortLabel: NSTextField!
    var startBtn: NSButton!
    var stopBtn: NSButton!
    var logView: NSTextView!

    var presets: [String: [String: Any]] = [:]
    var isKeyVisible = false
    var currentApiKey = ""
    var availableModels: [String] = []

    let W: CGFloat = 480
    let H: CGFloat = 700
    let pad: CGFloat = 20
    let fieldH: CGFloat = 30
    let mono: NSFont = NSFont(name: "Menlo", size: 12) ?? NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    let mono10: NSFont = NSFont(name: "Menlo", size: 10) ?? NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)
    let mono10b: NSFont = NSFont(name: "Menlo", size: 10) ?? NSFont.monospacedSystemFont(ofSize: 10, weight: .bold)
    let mono12b: NSFont = NSFont(name: "Menlo", size: 12) ?? NSFont.monospacedSystemFont(ofSize: 12, weight: .bold)
    let mono13b: NSFont = NSFont(name: "Menlo", size: 13) ?? NSFont.monospacedSystemFont(ofSize: 13, weight: .bold)

    override init() {
        super.init()
        build()
    }

    func build() {
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: W, height: H),
            styleMask: [.titled, .closable],
            backing: .buffered, defer: false
        )
        window.title = ""
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        window.center()
        window.delegate = self
        window.isReleasedWhenClosed = false
        window.backgroundColor = T.bg

        let v = NSView(frame: NSRect(x: 0, y: 0, width: W, height: H))
        v.wantsLayer = true
        v.layer?.backgroundColor = T.bg.cgColor
        window.contentView = v

        let cw = W - pad * 2
        var y = H

        // ═══ HEADER ═══
        let hdrH: CGFloat = 48
        y -= hdrH
        let hdr = boxed(NSRect(x: 0, y: y, width: W, height: hdrH), bg: T.surface)
        hdr.layer?.borderWidth = 0
        addBottomBorder(hdr, T.border)

        // Icon with shadow box
        let iconBox = NSView(frame: NSRect(x: pad, y: 6, width: 36, height: 36))
        iconBox.wantsLayer = true
        iconBox.layer?.backgroundColor = T.accent.cgColor
        iconBox.layer?.borderColor = T.fg.cgColor
        iconBox.layer?.borderWidth = T.bw
        // Shadow offset
        let shadowBox = NSView(frame: NSRect(x: pad + 4, y: 2, width: 36, height: 36))
        shadowBox.wantsLayer = true
        shadowBox.layer?.backgroundColor = T.fg.cgColor
        hdr.addSubview(shadowBox)
        hdr.addSubview(iconBox)

        let bolt = NSTextField(labelWithString: "⚡")
        bolt.font = NSFont.systemFont(ofSize: 16)
        bolt.textColor = NSColor(red: 0.04, green: 0.04, blue: 0.04, alpha: 1)
        bolt.alignment = .center
        bolt.frame = NSRect(x: 0, y: 4, width: 36, height: 28)
        iconBox.addSubview(bolt)

        let hTitle = label("CODEX PROXY", font: NSFont(name: "Menlo", size: 16) ?? mono13b, color: T.fg)
        hTitle.frame = NSRect(x: pad + 50, y: 22, width: 300, height: 20)
        hdr.addSubview(hTitle)

        let hSub = label("RESPONSES API ↔ CHAT COMPLETIONS / ANTHROPIC", font: mono10, color: T.muted)
        hSub.frame = NSRect(x: pad + 50, y: 8, width: 400, height: 14)
        hdr.addSubview(hSub)
        v.addSubview(hdr)

        // ═══ MAIN FORM ═══
        var fy = y - 16 // form start

        // ── Provider + Port row ──
        let row1W = cw - 110
        addFieldLabel(v, y: &fy, text: "PROVIDER", x: pad)
        presetPopup = makeCombo(NSRect(x: pad, y: fy - fieldH, width: row1W, height: fieldH))
        presetPopup.target = self
        presetPopup.action = #selector(onPresetChange)
        v.addSubview(presetPopup)

        addFieldLabel(v, y: &fy, text: "PORT", x: pad + row1W + 10)
        portField = makeInput(NSRect(x: pad + row1W + 10, y: fy - fieldH, width: 100, height: fieldH))
        portField.stringValue = "9090"
        v.addSubview(portField)
        fy -= fieldH + 12

        // ── Upstream ──
        addFieldLabel(v, y: &fy, text: "UPSTREAM URL", x: pad)
        upstreamField = makeInput(NSRect(x: pad, y: fy - fieldH, width: cw, height: fieldH))
        upstreamField.isEditable = false
        v.addSubview(upstreamField)
        fy -= fieldH + 12

        // ── API Key + Validate row ──
        addFieldLabel(v, y: &fy, text: "API KEY", x: pad)
        let keyW = cw - 120
        let keyWrap = boxed(NSRect(x: pad, y: fy - fieldH, width: keyW, height: fieldH), bg: T.bg)
        borderIt(keyWrap, T.border)

        apiKeyField = NSSecureTextField(frame: NSRect(x: 8, y: 0, width: keyW - 48, height: fieldH))
        styleIn(apiKeyField); apiKeyField.drawsBackground = false; apiKeyField.placeholderString = "sk-..."
        keyWrap.addSubview(apiKeyField)

        apiKeyVisibleField = NSTextField(frame: NSRect(x: 8, y: 0, width: keyW - 48, height: fieldH))
        styleIn(apiKeyVisibleField); apiKeyVisibleField.drawsBackground = false; apiKeyVisibleField.placeholderString = "sk-..."
        apiKeyVisibleField.isHidden = true
        keyWrap.addSubview(apiKeyVisibleField)

        showKeyBtn = NSButton(frame: NSRect(x: keyW - 36, y: 2, width: 30, height: fieldH - 4))
        showKeyBtn.title = "👁"; showKeyBtn.font = NSFont(name: "Apple Color Emoji", size: 12)
        showKeyBtn.bezelStyle = .inline; showKeyBtn.isBordered = false
        showKeyBtn.target = self; showKeyBtn.action = #selector(toggleKeyVis)
        keyWrap.addSubview(showKeyBtn)
        v.addSubview(keyWrap)

        // Validate button inline
        validateBtn = makeBtn(NSRect(x: pad + keyW + 10, y: fy - fieldH, width: 110, height: fieldH), title: "VALIDATE", bg: T.surface, fg: T.fg, border: T.border)
        validateBtn.font = mono10b
        validateBtn.target = self; validateBtn.action = #selector(onValidateKey)
        v.addSubview(validateBtn)
        fy -= fieldH + 4

        validateLabel = label("", font: mono10, color: T.muted)
        validateLabel.frame = NSRect(x: pad, y: fy - 14, width: cw, height: 14)
        v.addSubview(validateLabel)
        fy -= 18

        // ── Model ──
        addFieldLabel(v, y: &fy, text: "MODEL", x: pad)
        modelPopup = makeCombo(NSRect(x: pad, y: fy - fieldH, width: cw, height: fieldH))
        modelPopup.addItem(withTitle: "— validate key to load —")
        v.addSubview(modelPopup)
        fy -= fieldH + 14

        // ═══ STATUS BAR (with shadow) ═══
        let stH: CGFloat = 36
        // Shadow layer
        let stShadow = NSView(frame: NSRect(x: pad + 5, y: fy - stH - 5, width: cw, height: stH))
        stShadow.wantsLayer = true; stShadow.layer?.backgroundColor = T.border.cgColor
        v.addSubview(stShadow)

        let stWrap = boxed(NSRect(x: pad, y: fy - stH, width: cw, height: stH), bg: T.surface)
        borderIt(stWrap, T.border)

        statusDot = NSView(frame: NSRect(x: 14, y: 12, width: 12, height: 12))
        statusDot.wantsLayer = true; statusDot.layer?.backgroundColor = T.muted.cgColor
        stWrap.addSubview(statusDot)

        statusLabel = label("STOPPED", font: mono13b, color: T.muted)
        statusLabel.frame = NSRect(x: 34, y: 9, width: 200, height: 18)
        stWrap.addSubview(statusLabel)

        // Port badge
        let portBadge = boxed(NSRect(x: cw - 64, y: 8, width: 52, height: 20), bg: T.bg)
        borderIt(portBadge, T.border)
        statusPortLabel = label(":9090", font: mono12b, color: T.fg)
        statusPortLabel.frame = NSRect(x: 0, y: 2, width: 52, height: 16)
        statusPortLabel.alignment = .center
        portBadge.addSubview(statusPortLabel)
        stWrap.addSubview(portBadge)
        v.addSubview(stWrap)
        fy -= stH + 10

        // ═══ CONTROLS (Start / Stop) ═══
        let btnW = (cw - 10) / 2

        startBtn = makeBtn(NSRect(x: pad, y: fy - 38, width: btnW, height: 38), title: "▶ START", bg: T.accent, fg: NSColor(red: 0.04, green: 0.04, blue: 0.04, alpha: 1), border: T.fg)
        startBtn.font = mono12b
        startBtn.target = self; startBtn.action = #selector(onStart)
        v.addSubview(startBtn)

        stopBtn = makeBtn(NSRect(x: pad + btnW + 10, y: fy - 38, width: btnW, height: 38), title: "⏹ STOP", bg: T.danger.withAlphaComponent(0.18), fg: T.danger, border: T.danger)
        stopBtn.font = mono12b
        stopBtn.isEnabled = false
        stopBtn.target = self; stopBtn.action = #selector(onStop)
        v.addSubview(stopBtn)
        fy -= 38 + 10

        // ═══ LOG PANEL ═══
        let logH: CGFloat = 120
        let logWrap = boxed(NSRect(x: pad, y: fy - logH, width: cw, height: logH), bg: T.bg)
        borderIt(logWrap, T.border)

        // Log header
        let logHdr = boxed(NSRect(x: 0, y: logH - 24, width: cw, height: 24), bg: T.surface)
        addBottomBorder(logHdr, T.border)
        let logTitle = label("▸ LOGS", font: mono10b, color: T.accent)
        logTitle.frame = NSRect(x: 10, y: 5, width: 100, height: 14)
        logHdr.addSubview(logTitle)
        logWrap.addSubview(logHdr)

        logView = NSTextView(frame: NSRect(x: 2, y: 2, width: cw - 4, height: logH - 28))
        logView.font = mono10
        logView.textColor = T.muted
        logView.backgroundColor = T.bg
        logView.drawsBackground = true
        logView.isEditable = false
        logView.isSelectable = true
        logView.textContainerInset = NSSize(width: 8, height: 4)
        logWrap.addSubview(logView)
        v.addSubview(logWrap)
        fy -= logH + 6

        // ═══ FOOTER ═══
        let footH: CGFloat = 36
        let footer = boxed(NSRect(x: 0, y: fy - footH, width: W, height: footH), bg: T.surface)
        addTopBorder(footer, T.border)

        let footActions: [(String, NSColor, NSColor, Selector)] = [
            ("INSTALL",    T.fg,     T.border, #selector(onInstallConfig)),
            ("RESTORE",    T.fg,     T.border, #selector(onRestoreConfig)),
            ("ORIGINAL",   T.muted,  T.muted,  #selector(onRestoreOrig)),
            ("CODEX CLI",  T.accent, T.accent, #selector(onLaunchCLI)),
            ("DESKTOP",    T.accent, T.accent, #selector(onLaunchDesktop)),
            ("CONFIG",     T.fg,     T.border, #selector(onOpenConfig)),
        ]
        var fx: CGFloat = pad - 4
        for (title, fg, brd, action) in footActions {
            let btn = NSButton(frame: .zero)
            btn.bezelStyle = .regularSquare
            btn.isBordered = true
            btn.target = self
            btn.action = action
            btn.wantsLayer = true
            btn.layer?.borderColor = brd.cgColor
            btn.layer?.borderWidth = T.bw
            btn.layer?.backgroundColor = T.surface.cgColor
            btn.font = mono10b
            let a: [NSAttributedString.Key: Any] = [.foregroundColor: fg, .font: mono10b]
            btn.attributedTitle = NSAttributedString(string: title, attributes: a)
            btn.sizeToFit()
            var f = btn.frame
            f.size.width += 10
            f.size.height = 24
            f.origin = NSPoint(x: fx, y: 6)
            btn.frame = f
            footer.addSubview(btn)
            fx += f.width + 6
        }
        v.addSubview(footer)
    }

    // MARK: - UI helpers

    func boxed(_ r: NSRect, bg: NSColor) -> NSView {
        let v = NSView(frame: r); v.wantsLayer = true; v.layer?.backgroundColor = bg.cgColor; return v
    }
    func borderIt(_ v: NSView, _ c: NSColor) {
        v.wantsLayer = true; v.layer?.borderColor = c.cgColor; v.layer?.borderWidth = T.bw
    }
    func addBottomBorder(_ v: NSView, _ c: NSColor) {
        let b = NSView(frame: NSRect(x: 0, y: 0, width: v.frame.width, height: T.bw))
        b.wantsLayer = true; b.layer?.backgroundColor = c.cgColor; v.addSubview(b)
    }
    func addTopBorder(_ v: NSView, _ c: NSColor) {
        let b = NSView(frame: NSRect(x: 0, y: v.frame.height - T.bw, width: v.frame.width, height: T.bw))
        b.wantsLayer = true; b.layer?.backgroundColor = c.cgColor; v.addSubview(b)
    }
    func label(_ text: String, font: NSFont, color: NSColor) -> NSTextField {
        let l = NSTextField(labelWithString: text); l.font = font; l.textColor = color; return l
    }
    func addFieldLabel(_ v: NSView, y: inout CGFloat, text: String, x: CGFloat) {
        let l = label(text, font: mono10b, color: T.accent)
        l.frame = NSRect(x: x, y: y - 14, width: 300, height: 14)
        v.addSubview(l)
        y -= 18
    }
    func makeInput(_ r: NSRect) -> NSTextField {
        let f = NSTextField(frame: r); styleIn(f); return f
    }
    func styleIn(_ f: NSTextField) {
        f.font = mono; f.textColor = T.fg; f.backgroundColor = T.bg
        f.drawsBackground = true; f.isBezeled = false; f.wantsLayer = true
        f.layer?.borderColor = T.border.cgColor; f.layer?.borderWidth = T.bw
        f.cell?.sendsActionOnEndEditing = true
    }
    func makeCombo(_ r: NSRect) -> NSPopUpButton {
        let p = NSPopUpButton(frame: r, pullsDown: false)
        p.font = mono; p.wantsLayer = true
        p.layer?.borderColor = T.border.cgColor; p.layer?.borderWidth = T.bw
        p.layer?.backgroundColor = T.bg.cgColor
        if let c = p.cell as? NSPopUpButtonCell {
            c.bezelStyle = .inline; c.isBordered = false; c.backgroundColor = T.bg; c.arrowPosition = .arrowAtBottom
        }
        return p
    }
    func makeBtn(_ r: NSRect, title: String, bg: NSColor, fg: NSColor, border: NSColor) -> NSButton {
        let b = NSButton(frame: r)
        b.wantsLayer = true; b.layer?.backgroundColor = bg.cgColor
        b.layer?.borderColor = border.cgColor; b.layer?.borderWidth = T.bw
        b.bezelStyle = .regularSquare; b.isBordered = false
        let a: [NSAttributedString.Key: Any] = [.foregroundColor: fg, .font: mono12b]
        b.attributedTitle = NSAttributedString(string: title, attributes: a)
        return b
    }
    func log(_ msg: String, ok: Bool = true) {
        let ts = DateFormatter(); ts.dateFormat = "HH:mm:ss"; ts.string(from: Date())
        let prefix = ts.string(from: Date())
        let color = ok ? T.accent : T.danger
        let full = "[\(prefix)] \(msg)\n"
        if let st = logView.textStorage {
            let attrs: [NSAttributedString.Key: Any] = [.foregroundColor: color, .font: mono10]
            st.append(NSAttributedString(string: full, attributes: attrs))
            logView.scrollToEndOfDocument(nil)
        }
    }

    // MARK: - State

    func setRunning(_ on: Bool, port: Int, name: String) {
        if on {
            startBtn.isEnabled = false; startBtn.layer?.backgroundColor = T.surface2.cgColor
            stopBtn.isEnabled = true; stopBtn.layer?.backgroundColor = T.danger.cgColor
            setBtnTitle(stopBtn, "⏹ STOP", fg: .white)
            statusDot.layer?.backgroundColor = T.accent.cgColor
            statusLabel.stringValue = "RUNNING"; statusLabel.textColor = T.accent
            statusPortLabel.stringValue = ":\(port)"
        } else {
            startBtn.isEnabled = true; startBtn.layer?.backgroundColor = T.accent.cgColor
            setBtnTitle(startBtn, "▶ START", fg: NSColor(red: 0.04, green: 0.04, blue: 0.04, alpha: 1))
            stopBtn.isEnabled = false; stopBtn.layer?.backgroundColor = T.danger.withAlphaComponent(0.18).cgColor
            setBtnTitle(stopBtn, "⏹ STOP", fg: T.danger)
            statusDot.layer?.backgroundColor = T.muted.cgColor
            statusLabel.stringValue = "STOPPED"; statusLabel.textColor = T.muted
        }
    }
    func setBtnTitle(_ b: NSButton, _ t: String, fg: NSColor) {
        let a: [NSAttributedString.Key: Any] = [.foregroundColor: fg, .font: mono12b]
        b.attributedTitle = NSAttributedString(string: t, attributes: a)
    }

    // MARK: - Load

    func loadSettings() {
        if let r = runPythonJSON(command: "get-presets"), let p = r["presets"] as? [String: [String: Any]] {
            presets = p
            var items = Array(p.keys).sorted(); items.append("Custom")
            presetPopup.removeAllItems(); presetPopup.addItems(withTitles: items)
        }
        if let r = runPythonJSON(command: "get-settings") {
            let preset = r["preset"] as? String ?? ""
            if !preset.isEmpty, let i = presetPopup.itemTitles.firstIndex(of: preset) { presetPopup.selectItem(at: i) }
            upstreamField.stringValue = r["upstream"] as? String ?? ""
            let port = r["port"] as? Int ?? 9090
            portField.stringValue = String(port)
            statusPortLabel.stringValue = ":\(port)"

            if let kr = runPythonJSON(command: "get-key", args: ["--preset", preset]),
               let key = kr["key"] as? String, !key.isEmpty {
                currentApiKey = key; apiKeyField.stringValue = key; apiKeyVisibleField.stringValue = key
            }
            if let model = r["selected_model"] as? String, !model.isEmpty {
                modelPopup.removeAllItems(); modelPopup.addItem(withTitle: model); availableModels = [model]
            }
        }
        log("Ready. Select provider → validate → start", ok: true)
    }

    // MARK: - Actions

    @objc func onPresetChange() {
        guard let sel = presetPopup.titleOfSelectedItem else { return }
        if sel != "Custom", let p = presets[sel] {
            upstreamField.stringValue = p["url"] as? String ?? ""
            if let kr = runPythonJSON(command: "get-key", args: ["--preset", sel]),
               let key = kr["key"] as? String, !key.isEmpty {
                currentApiKey = key; apiKeyField.stringValue = key; apiKeyVisibleField.stringValue = key
            } else { currentApiKey = ""; apiKeyField.stringValue = ""; apiKeyVisibleField.stringValue = "" }
            modelPopup.removeAllItems(); modelPopup.addItem(withTitle: "— validate key to load —")
            availableModels = []; validateLabel.stringValue = ""
        }
    }

    @objc func toggleKeyVis() {
        isKeyVisible = !isKeyVisible
        if isKeyVisible {
            apiKeyVisibleField.stringValue = apiKeyField.stringValue
            apiKeyField.isHidden = true; apiKeyVisibleField.isHidden = false
        } else {
            apiKeyField.stringValue = apiKeyVisibleField.stringValue
            apiKeyVisibleField.isHidden = true; apiKeyField.isHidden = false
        }
    }

    @objc func onValidateKey() {
        let preset = presetPopup.titleOfSelectedItem ?? ""
        let key = isKeyVisible ? apiKeyVisibleField.stringValue : apiKeyField.stringValue
        if key.isEmpty { validateLabel.stringValue = "⚠ ENTER KEY FIRST"; validateLabel.textColor = T.fg; return }
        validateLabel.stringValue = "VALIDATING..."; validateLabel.textColor = T.muted
        validateBtn.isEnabled = false

        runPythonJSONAsync(command: "validate-key", args: ["--preset", preset, "--api-key", key]) { [weak self] result in
            guard let self = self else { return }
            if let r = result, r["valid"] as? Bool == true {
                self.validateLabel.stringValue = "✓ VALID"; self.validateLabel.textColor = T.accent
                _ = self.runPythonJSON(command: "store-key", args: ["--preset", preset, "--api-key", key])
                self.currentApiKey = key
                self.log("✓ Key valid for \(preset)")

                self.validateLabel.stringValue = "LOADING MODELS..."
                self.runPythonJSONAsync(command: "fetch-models", args: ["--preset", preset, "--api-key", key]) { [weak self] mr in
                    guard let self = self else { return }
                    self.validateBtn.isEnabled = true
                    if let models = mr?["models"] as? [String], !models.isEmpty {
                        self.availableModels = models
                        self.modelPopup.removeAllItems(); self.modelPopup.addItems(withTitles: models)
                        self.validateLabel.stringValue = "✓ \(models.count) MODELS"; self.validateLabel.textColor = T.accent
                        self.log("✓ \(models.count) models loaded for \(preset)")
                    } else {
                        self.validateLabel.stringValue = "✓ VALID"; self.validateLabel.textColor = T.accent
                    }
                }
            } else {
                self.validateBtn.isEnabled = true
                self.validateLabel.stringValue = "✗ INVALID"; self.validateLabel.textColor = T.danger
                self.log("✗ Key validation failed", ok: false)
            }
        }
    }

    @objc func onStart() {
        let preset = presetPopup.titleOfSelectedItem ?? ""
        let key = isKeyVisible ? apiKeyVisibleField.stringValue : apiKeyField.stringValue
        if key.isEmpty {
            let a = NSAlert(); a.messageText = "NO API KEY"; a.informativeText = "Enter API key first."; a.runModal(); return
        }
        let port = Int(portField.stringValue) ?? 9090
        let model = modelPopup.titleOfSelectedItem ?? ""
        let json: [String: Any] = [
            "preset": preset == "Custom" ? "" : preset, "upstream": upstreamField.stringValue,
            "port": port, "api_key": key, "selected_model": availableModels.contains(model) ? model : ""
        ]
        if let d = try? JSONSerialization.data(withJSONObject: json), let s = String(data: d, encoding: .utf8) {
            _ = runPythonJSON(command: "save-settings", args: ["--json", s])
        }
        _ = runPythonJSON(command: "start")
        setRunning(true, port: port, name: preset)
        log("Proxy started → \(preset) on :\(port)")
    }

    @objc func onStop() {
        _ = runPythonJSON(command: "stop")
        setRunning(false, port: 0, name: "")
        log("Proxy stopped — config restored")
    }

    @objc func onInstallConfig() { _ = runPythonJSON(command: "install"); log("Config installed") }
    @objc func onRestoreConfig() { _ = runPythonJSON(command: "restore-config"); log("Config restored") }
    @objc func onRestoreOrig() { _ = runPythonJSON(command: "restore-config"); log("Original config restored") }
    @objc func onOpenConfig() { _ = runPythonJSON(command: "config") }
    @objc func onLaunchCLI() { _ = runPythonJSON(command: "launch-codex-cli"); log("Codex CLI launched") }
    @objc func onLaunchDesktop() { _ = runPythonJSON(command: "launch-codex-app"); log("Codex Desktop launched") }

    func windowWillClose(_ n: Notification) {
        let preset = presetPopup.titleOfSelectedItem ?? ""
        let key = isKeyVisible ? apiKeyVisibleField.stringValue : apiKeyField.stringValue
        let model = modelPopup.titleOfSelectedItem ?? ""
        let json: [String: Any] = [
            "preset": preset == "Custom" ? "" : preset, "upstream": upstreamField.stringValue,
            "port": Int(portField.stringValue) ?? 9090, "api_key": key,
            "selected_model": availableModels.contains(model) ? model : ""
        ]
        if let d = try? JSONSerialization.data(withJSONObject: json), let s = String(data: d, encoding: .utf8) {
            _ = runPythonJSON(command: "save-settings", args: ["--json", s])
        }
    }

    // MARK: - Python comm

    func runPythonJSON(command: String, args: [String] = []) -> [String: Any]? {
        let task = Process()
        let bd = Bundle.main.bundlePath + "/Contents/MacOS"
        let pyPath: String
        if FileManager.default.fileExists(atPath: bd + "/app_native.py") {
            pyPath = bd + "/app_native.py"; task.currentDirectoryURL = URL(fileURLWithPath: bd)
        } else {
            let src = Bundle.main.bundleURL.deletingLastPathComponent().path
            pyPath = src + "/app_native.py"; task.currentDirectoryURL = URL(fileURLWithPath: src)
        }
        task.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        task.arguments = [pyPath, command] + args
        let pipe = Pipe(); task.standardOutput = pipe; task.standardError = FileHandle.nullDevice
        do {
            try task.run(); task.waitUntilExit()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            if let s = String(data: data, encoding: .utf8), let d = s.data(using: .utf8),
               let j = try? JSONSerialization.jsonObject(with: d) as? [String: Any] { return j }
        } catch let e { NSLog("Python: \(e.localizedDescription)") }
        return nil
    }

    func runPythonJSONAsync(command: String, args: [String] = [], completion: @escaping ([String: Any]?) -> Void) {
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let r = self?.runPythonJSON(command: command, args: args)
            DispatchQueue.main.async { completion(r) }
        }
    }
}

// MARK: - App Delegate

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem?
    var stateTimer: Timer?
    var settingsController: SettingsWindowController?

    lazy var configDir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".codexproxy")

    func applicationDidFinishLaunching(_ n: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let b = statusItem?.button {
            b.image = NSImage(systemSymbolName: "bolt.fill", accessibilityDescription: "Codex Proxy")
            b.image?.isTemplate = true
        }
        setupMenu(); startStateTimer(); showSettings()
    }

    func setupMenu() {
        let m = NSMenu()
        let smi = NSMenuItem(title: "○ STOPPED", action: nil, keyEquivalent: "")
        smi.tag = 100; smi.isEnabled = false; m.addItem(smi)
        m.addItem(NSMenuItem.separator())
        m.addItem(withTitle: "▶ Start", action: #selector(startProxy), keyEquivalent: "s")
        m.addItem(withTitle: "⏹ Stop", action: #selector(stopProxy), keyEquivalent: "x")
        m.addItem(NSMenuItem.separator())
        m.addItem(withTitle: "Settings...", action: #selector(showSettings), keyEquivalent: ",")
        m.addItem(withTitle: "Install Config", action: #selector(installConfig), keyEquivalent: "i")
        m.addItem(withTitle: "Restore Config", action: #selector(restoreConfig), keyEquivalent: "")
        m.addItem(NSMenuItem.separator())
        m.addItem(withTitle: "Open Logs", action: #selector(openLogs), keyEquivalent: "l")
        m.addItem(withTitle: "Config Folder", action: #selector(openConfig), keyEquivalent: "c")
        m.addItem(NSMenuItem.separator())
        m.addItem(withTitle: "Launch Codex CLI", action: #selector(launchCodexCLI), keyEquivalent: "")
        m.addItem(withTitle: "Launch Codex Desktop", action: #selector(launchCodexDesktop), keyEquivalent: "")
        m.addItem(NSMenuItem.separator())
        m.addItem(withTitle: "About", action: #selector(showAbout), keyEquivalent: "")
        m.addItem(withTitle: "Quit", action: #selector(quitApp), keyEquivalent: "q")
        statusItem?.menu = m
    }

    func startStateTimer() {
        stateTimer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in self?.updateStatus() }
    }

    func updateStatus() {
        guard let menu = statusItem?.menu else { return }
        let sf = configDir.appendingPathComponent("app_state.json")
        if let d = try? Data(contentsOf: sf), let s = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
           let on = s["proxy_running"] as? Bool {
            let mi = menu.item(withTag: 100)
            if on {
                mi?.title = "● RUNNING · localhost:\(s["port"] as? Int ?? 9090)"
                statusItem?.button?.image = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "Running")
            } else {
                mi?.title = "○ STOPPED"
                statusItem?.button?.image = NSImage(systemSymbolName: "bolt.fill", accessibilityDescription: "Stopped")
            }
            if let sc = settingsController, sc.window.isVisible {
                sc.setRunning(on, port: s["port"] as? Int ?? 9090, name: s["upstream"] as? String ?? "")
            }
        }
    }

    @objc func startProxy() { execPy("start") }
    @objc func stopProxy() { execPy("stop") }
    @objc func showSettings() {
        if settingsController == nil { settingsController = SettingsWindowController() }
        settingsController!.loadSettings()
        settingsController!.window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
    @objc func installConfig() { execPy("install") }
    @objc func restoreConfig() { execPy("restore-config") }
    @objc func openLogs() {
        let p = configDir.appendingPathComponent("proxy.log")
        if !FileManager.default.fileExists(atPath: p.path) { try? Data().write(to: p) }
        NSWorkspace.shared.open(p)
    }
    @objc func openConfig() {
        try? FileManager.default.createDirectory(at: configDir, withIntermediateDirectories: true)
        NSWorkspace.shared.open(configDir)
    }
    @objc func launchCodexCLI() { execPy("launch-codex-cli") }
    @objc func launchCodexDesktop() { execPy("launch-codex-app") }
    @objc func showAbout() {
        let a = NSAlert()
        a.messageText = "CODEXPROXY 2.0"
        a.informativeText = "Responses API → Chat Completions / Anthropic\n\nRoute Codex Desktop through alternative providers."
        a.alertStyle = .informational; a.addButton(withTitle: "OK"); a.runModal()
    }
    @objc func quitApp() { execPy("stop"); NSApplication.shared.terminate(self) }

    func execPy(_ cmd: String) {
        let task = Process()
        let bd = Bundle.main.bundlePath + "/Contents/MacOS"
        let pyPath: String
        if FileManager.default.fileExists(atPath: bd + "/app_native.py") {
            pyPath = bd + "/app_native.py"; task.currentDirectoryURL = URL(fileURLWithPath: bd)
        } else {
            let src = Bundle.main.bundleURL.deletingLastPathComponent().path
            pyPath = src + "/app_native.py"; task.currentDirectoryURL = URL(fileURLWithPath: src)
        }
        task.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        task.arguments = [pyPath, cmd]
        try? task.run()
    }
}
