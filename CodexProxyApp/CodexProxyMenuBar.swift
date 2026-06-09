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

// MARK: - Layout helpers

extension NSView {
    func themedBg(_ c: NSColor) {
        wantsLayer = true
        layer?.backgroundColor = c.cgColor
    }
    func bordered(_ c: NSColor = T.border, _ w: CGFloat = T.bw) {
        wantsLayer = true
        layer?.borderColor = c.cgColor
        layer?.borderWidth = w
    }
}

func makeLabel(_ text: String, font: NSFont, color: NSColor) -> NSTextField {
    let l = NSTextField(labelWithString: text)
    l.font = font
    l.textColor = color
    l.translatesAutoresizingMaskIntoConstraints = false
    return l
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

    let fieldH: CGFloat = 30
    let pad: CGFloat = 20
    let mono = NSFont(name: "Menlo", size: 12) ?? NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    let mono10 = NSFont(name: "Menlo", size: 10) ?? NSFont.monospacedSystemFont(ofSize: 10, weight: .regular)
    let mono10b = NSFont(name: "Menlo", size: 10) ?? NSFont.monospacedSystemFont(ofSize: 10, weight: .bold)
    let mono12b = NSFont(name: "Menlo", size: 12) ?? NSFont.monospacedSystemFont(ofSize: 12, weight: .bold)
    let mono13b = NSFont(name: "Menlo", size: 13) ?? NSFont.monospacedSystemFont(ofSize: 13, weight: .bold)

    override init() {
        super.init()
        build()
    }

    // MARK: - Build UI (Auto Layout / NSStackView)

    func build() {
        let W: CGFloat = 480
        let H: CGFloat = 700

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
        window.minSize = NSSize(width: 420, height: 600)
        window.backgroundColor = T.bg

        let root = NSView()
        root.wantsLayer = true
        root.layer?.backgroundColor = T.bg.cgColor
        root.translatesAutoresizingMaskIntoConstraints = false
        window.contentView = root

        // Main vertical stack fills root
        let mainStack = NSStackView()
        mainStack.orientation = .vertical
        mainStack.spacing = 0
        mainStack.alignment = .fill
        mainStack.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(mainStack)
        NSLayoutConstraint.activate([
            mainStack.topAnchor.constraint(equalTo: root.topAnchor),
            mainStack.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            mainStack.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            mainStack.trailingAnchor.constraint(equalTo: root.trailingAnchor),
        ])

        // ═══ HEADER ═══
        let header = NSView()
        header.translatesAutoresizingMaskIntoConstraints = false
        header.themedBg(T.surface)

        let hdrBorder = NSView()
        hdrBorder.translatesAutoresizingMaskIntoConstraints = false
        hdrBorder.themedBg(T.border)
        header.addSubview(hdrBorder)
        NSLayoutConstraint.activate([
            hdrBorder.leadingAnchor.constraint(equalTo: header.leadingAnchor),
            hdrBorder.trailingAnchor.constraint(equalTo: header.trailingAnchor),
            hdrBorder.bottomAnchor.constraint(equalTo: header.bottomAnchor),
            hdrBorder.heightAnchor.constraint(equalToConstant: T.bw),
        ])

        let iconShadow = NSView()
        iconShadow.translatesAutoresizingMaskIntoConstraints = false
        iconShadow.themedBg(T.fg)
        header.addSubview(iconShadow)
        NSLayoutConstraint.activate([
            iconShadow.leadingAnchor.constraint(equalTo: header.leadingAnchor, constant: pad + 4),
            iconShadow.topAnchor.constraint(equalTo: header.topAnchor, constant: 10),
            iconShadow.widthAnchor.constraint(equalToConstant: 36),
            iconShadow.heightAnchor.constraint(equalToConstant: 36),
        ])

        let iconBox = NSView()
        iconBox.translatesAutoresizingMaskIntoConstraints = false
        iconBox.themedBg(T.accent)
        iconBox.bordered(T.fg)
        header.addSubview(iconBox)
        NSLayoutConstraint.activate([
            iconBox.leadingAnchor.constraint(equalTo: header.leadingAnchor, constant: pad),
            iconBox.topAnchor.constraint(equalTo: header.topAnchor, constant: 6),
            iconBox.widthAnchor.constraint(equalToConstant: 36),
            iconBox.heightAnchor.constraint(equalToConstant: 36),
        ])

        let bolt = NSTextField(labelWithString: "\u{26A1}")
        bolt.font = NSFont.systemFont(ofSize: 16)
        bolt.textColor = NSColor(red: 0.04, green: 0.04, blue: 0.04, alpha: 1)
        bolt.alignment = .center
        bolt.translatesAutoresizingMaskIntoConstraints = false
        iconBox.addSubview(bolt)
        NSLayoutConstraint.activate([
            bolt.centerXAnchor.constraint(equalTo: iconBox.centerXAnchor),
            bolt.centerYAnchor.constraint(equalTo: iconBox.centerYAnchor),
        ])

        let hTitle = makeLabel("CODEX PROXY", font: NSFont(name: "Menlo", size: 16) ?? mono13b, color: T.fg)
        let hSub = makeLabel("RESPONSES API \u{2194} CHAT COMPLETIONS / ANTHROPIC", font: mono10, color: T.muted)
        let headerText = NSStackView(views: [hTitle, hSub])
        headerText.orientation = .vertical
        headerText.spacing = 1
        headerText.translatesAutoresizingMaskIntoConstraints = false
        header.addSubview(headerText)
        NSLayoutConstraint.activate([
            headerText.leadingAnchor.constraint(equalTo: iconBox.trailingAnchor, constant: 14),
            headerText.centerYAnchor.constraint(equalTo: iconBox.centerYAnchor),
        ])

        header.translatesAutoresizingMaskIntoConstraints = false
        let headerH: CGFloat = 56
        header.heightAnchor.constraint(equalToConstant: headerH).isActive = true
        mainStack.addArrangedSubview(header)

        // ═══ MAIN FORM ═══
        let formStack = NSStackView()
        formStack.orientation = .vertical
        formStack.spacing = 12
        formStack.alignment = .fill
        formStack.translatesAutoresizingMaskIntoConstraints = false

        let formWrap = NSView()
        formWrap.translatesAutoresizingMaskIntoConstraints = false
        formWrap.addSubview(formStack)
        NSLayoutConstraint.activate([
            formStack.topAnchor.constraint(equalTo: formWrap.topAnchor, constant: 16),
            formStack.bottomAnchor.constraint(equalTo: formWrap.bottomAnchor, constant: -16),
            formStack.leadingAnchor.constraint(equalTo: formWrap.leadingAnchor, constant: pad),
            formStack.trailingAnchor.constraint(equalTo: formWrap.trailingAnchor, constant: -pad),
        ])
        mainStack.addArrangedSubview(formWrap)

        // -- Provider + Port row --
        presetPopup = makeCombo()
        let providerCol = fieldColumn("PROVIDER", view: presetPopup)

        portField = makeInput()
        portField.stringValue = "9090"
        let portCol = fieldColumn("PORT", view: portField)
        portField.widthAnchor.constraint(equalToConstant: 100).isActive = true

        let providerRow = NSStackView(views: [providerCol, portCol])
        providerRow.orientation = .horizontal
        providerRow.spacing = 10
        providerRow.alignment = .top
        providerRow.translatesAutoresizingMaskIntoConstraints = false
        providerCol.setContentHuggingPriority(.defaultLow, for: .horizontal)
        portCol.setContentHuggingPriority(.required, for: .horizontal)
        formStack.addArrangedSubview(providerRow)

        // -- Upstream --
        upstreamField = makeInput()
        upstreamField.isEditable = false
        formStack.addArrangedSubview(fieldColumn("UPSTREAM URL", view: upstreamField))

        // -- API Key + Validate row --
        let keyWrap = NSView()
        keyWrap.translatesAutoresizingMaskIntoConstraints = false
        keyWrap.themedBg(T.bg)
        keyWrap.bordered()

        apiKeyField = NSSecureTextField()
        styleInput(apiKeyField)
        apiKeyField.placeholderString = "sk-..."
        apiKeyField.translatesAutoresizingMaskIntoConstraints = false
        keyWrap.addSubview(apiKeyField)

        apiKeyVisibleField = NSTextField()
        styleInput(apiKeyVisibleField)
        apiKeyVisibleField.drawsBackground = false
        apiKeyVisibleField.placeholderString = "sk-..."
        apiKeyVisibleField.isHidden = true
        apiKeyVisibleField.translatesAutoresizingMaskIntoConstraints = false
        keyWrap.addSubview(apiKeyVisibleField)

        showKeyBtn = NSButton(title: "\u{1F441}", target: self, action: #selector(toggleKeyVis))
        showKeyBtn.font = NSFont(name: "Apple Color Emoji", size: 12)
        showKeyBtn.bezelStyle = .inline
        showKeyBtn.isBordered = false
        showKeyBtn.translatesAutoresizingMaskIntoConstraints = false
        keyWrap.addSubview(showKeyBtn)

        NSLayoutConstraint.activate([
            apiKeyField.leadingAnchor.constraint(equalTo: keyWrap.leadingAnchor, constant: 8),
            apiKeyField.trailingAnchor.constraint(equalTo: showKeyBtn.leadingAnchor, constant: -4),
            apiKeyField.topAnchor.constraint(equalTo: keyWrap.topAnchor),
            apiKeyField.bottomAnchor.constraint(equalTo: keyWrap.bottomAnchor),
            apiKeyVisibleField.leadingAnchor.constraint(equalTo: apiKeyField.leadingAnchor),
            apiKeyVisibleField.trailingAnchor.constraint(equalTo: apiKeyField.trailingAnchor),
            apiKeyVisibleField.topAnchor.constraint(equalTo: apiKeyField.topAnchor),
            apiKeyVisibleField.bottomAnchor.constraint(equalTo: apiKeyField.bottomAnchor),
            showKeyBtn.trailingAnchor.constraint(equalTo: keyWrap.trailingAnchor, constant: -4),
            showKeyBtn.centerYAnchor.constraint(equalTo: keyWrap.centerYAnchor),
            showKeyBtn.widthAnchor.constraint(equalToConstant: 30),
            keyWrap.heightAnchor.constraint(equalToConstant: fieldH),
        ])

        validateBtn = makeBtn(title: "VALIDATE", bg: T.surface, fg: T.fg, border: T.border)
        validateBtn.font = mono10b
        validateBtn.target = self
        validateBtn.action = #selector(onValidateKey)
        validateBtn.widthAnchor.constraint(equalToConstant: 110).isActive = true

        let keyRow = NSStackView(views: [keyWrap, validateBtn])
        keyRow.orientation = .horizontal
        keyRow.spacing = 10
        keyRow.alignment = .centerY
        keyRow.translatesAutoresizingMaskIntoConstraints = false
        keyWrap.setContentHuggingPriority(.defaultLow, for: .horizontal)
        validateBtn.setContentHuggingPriority(.required, for: .horizontal)
        formStack.addArrangedSubview(fieldColumn("API KEY", view: keyRow))

        validateLabel = makeLabel("", font: mono10, color: T.muted)
        formStack.addArrangedSubview(validateLabel)

        // -- Model --
        modelPopup = makeCombo()
        modelPopup.addItem(withTitle: "\u{2014} validate key to load \u{2014}")
        formStack.addArrangedSubview(fieldColumn("MODEL", view: modelPopup))

        // ═══ STATUS BAR ═══
        let stWrap = NSView()
        stWrap.translatesAutoresizingMaskIntoConstraints = false
        stWrap.themedBg(T.surface)
        stWrap.bordered()

        let stShadow = NSView()
        stShadow.translatesAutoresizingMaskIntoConstraints = false
        stShadow.themedBg(T.border)
        let statusOuter = NSView()
        statusOuter.translatesAutoresizingMaskIntoConstraints = false
        statusOuter.addSubview(stShadow)
        statusOuter.addSubview(stWrap)
        NSLayoutConstraint.activate([
            stShadow.leadingAnchor.constraint(equalTo: statusOuter.leadingAnchor, constant: 5),
            stShadow.trailingAnchor.constraint(equalTo: statusOuter.trailingAnchor, constant: -5),
            stShadow.topAnchor.constraint(equalTo: statusOuter.topAnchor),
            stShadow.heightAnchor.constraint(equalToConstant: 36),
            stWrap.leadingAnchor.constraint(equalTo: statusOuter.leadingAnchor),
            stWrap.trailingAnchor.constraint(equalTo: statusOuter.trailingAnchor),
            stWrap.topAnchor.constraint(equalTo: statusOuter.topAnchor, constant: 5),
            stWrap.bottomAnchor.constraint(equalTo: statusOuter.bottomAnchor),
            statusOuter.heightAnchor.constraint(equalToConstant: 41),
        ])

        statusDot = NSView()
        statusDot.translatesAutoresizingMaskIntoConstraints = false
        statusDot.themedBg(T.muted)
        stWrap.addSubview(statusDot)
        NSLayoutConstraint.activate([
            statusDot.leadingAnchor.constraint(equalTo: stWrap.leadingAnchor, constant: 14),
            statusDot.centerYAnchor.constraint(equalTo: stWrap.centerYAnchor),
            statusDot.widthAnchor.constraint(equalToConstant: 12),
            statusDot.heightAnchor.constraint(equalToConstant: 12),
        ])

        statusLabel = makeLabel("STOPPED", font: mono13b, color: T.muted)
        stWrap.addSubview(statusLabel)
        NSLayoutConstraint.activate([
            statusLabel.leadingAnchor.constraint(equalTo: statusDot.trailingAnchor, constant: 8),
            statusLabel.centerYAnchor.constraint(equalTo: stWrap.centerYAnchor),
        ])

        let portBadge = NSView()
        portBadge.translatesAutoresizingMaskIntoConstraints = false
        portBadge.themedBg(T.bg)
        portBadge.bordered()
        stWrap.addSubview(portBadge)
        NSLayoutConstraint.activate([
            portBadge.trailingAnchor.constraint(equalTo: stWrap.trailingAnchor, constant: -14),
            portBadge.centerYAnchor.constraint(equalTo: stWrap.centerYAnchor),
            portBadge.widthAnchor.constraint(equalToConstant: 52),
            portBadge.heightAnchor.constraint(equalToConstant: 20),
        ])

        statusPortLabel = makeLabel(":9090", font: mono12b, color: T.fg)
        statusPortLabel.alignment = .center
        portBadge.addSubview(statusPortLabel)
        NSLayoutConstraint.activate([
            statusPortLabel.centerXAnchor.constraint(equalTo: portBadge.centerXAnchor),
            statusPortLabel.centerYAnchor.constraint(equalTo: portBadge.centerYAnchor),
        ])

        let statusSection = NSView()
        statusSection.translatesAutoresizingMaskIntoConstraints = false
        statusSection.addSubview(statusOuter)
        NSLayoutConstraint.activate([
            statusOuter.leadingAnchor.constraint(equalTo: statusSection.leadingAnchor, constant: pad),
            statusOuter.trailingAnchor.constraint(equalTo: statusSection.trailingAnchor, constant: -pad),
            statusOuter.topAnchor.constraint(equalTo: statusSection.topAnchor),
            statusOuter.bottomAnchor.constraint(equalTo: statusSection.bottomAnchor),
        ])
        mainStack.addArrangedSubview(statusSection)

        // spacer
        let spacer1 = NSView()
        spacer1.translatesAutoresizingMaskIntoConstraints = false
        spacer1.heightAnchor.constraint(equalToConstant: 10).isActive = true
        mainStack.addArrangedSubview(spacer1)

        // ═══ CONTROLS ═══
        startBtn = makeBtn(title: "\u{25B6} START", bg: T.accent, fg: NSColor(red: 0.04, green: 0.04, blue: 0.04, alpha: 1), border: T.fg)
        startBtn.font = mono12b
        startBtn.target = self
        startBtn.action = #selector(onStart)

        stopBtn = makeBtn(title: "\u{23F9} STOP", bg: T.danger.withAlphaComponent(0.18), fg: T.danger, border: T.danger)
        stopBtn.font = mono12b
        stopBtn.isEnabled = false
        stopBtn.target = self
        stopBtn.action = #selector(onStop)

        let controlsRow = NSStackView(views: [startBtn, stopBtn])
        controlsRow.orientation = .horizontal
        controlsRow.spacing = 10
        controlsRow.alignment = .fill
        controlsRow.translatesAutoresizingMaskIntoConstraints = false
        controlsRow.heightAnchor.constraint(equalToConstant: 38).isActive = true
        startBtn.widthAnchor.constraint(equalTo: stopBtn.widthAnchor).isActive = true

        let controlsWrap = NSView()
        controlsWrap.translatesAutoresizingMaskIntoConstraints = false
        controlsWrap.addSubview(controlsRow)
        NSLayoutConstraint.activate([
            controlsRow.topAnchor.constraint(equalTo: controlsWrap.topAnchor),
            controlsRow.bottomAnchor.constraint(equalTo: controlsWrap.bottomAnchor),
            controlsRow.leadingAnchor.constraint(equalTo: controlsWrap.leadingAnchor, constant: pad),
            controlsRow.trailingAnchor.constraint(equalTo: controlsWrap.trailingAnchor, constant: -pad),
        ])
        mainStack.addArrangedSubview(controlsWrap)

        // spacer
        let spacer2 = NSView()
        spacer2.translatesAutoresizingMaskIntoConstraints = false
        spacer2.heightAnchor.constraint(equalToConstant: 10).isActive = true
        mainStack.addArrangedSubview(spacer2)

        // ═══ LOG PANEL ═══
        let logWrap = NSView()
        logWrap.translatesAutoresizingMaskIntoConstraints = false
        logWrap.themedBg(T.bg)
        logWrap.bordered()

        let logHdr = NSView()
        logHdr.translatesAutoresizingMaskIntoConstraints = false
        logHdr.themedBg(T.surface)
        logWrap.addSubview(logHdr)
        NSLayoutConstraint.activate([
            logHdr.leadingAnchor.constraint(equalTo: logWrap.leadingAnchor),
            logHdr.trailingAnchor.constraint(equalTo: logWrap.trailingAnchor),
            logHdr.topAnchor.constraint(equalTo: logWrap.topAnchor),
            logHdr.heightAnchor.constraint(equalToConstant: 24),
        ])

        let logBorder = NSView()
        logBorder.translatesAutoresizingMaskIntoConstraints = false
        logBorder.themedBg(T.border)
        logWrap.addSubview(logBorder)
        NSLayoutConstraint.activate([
            logBorder.leadingAnchor.constraint(equalTo: logWrap.leadingAnchor),
            logBorder.trailingAnchor.constraint(equalTo: logWrap.trailingAnchor),
            logBorder.topAnchor.constraint(equalTo: logHdr.bottomAnchor),
            logBorder.heightAnchor.constraint(equalToConstant: T.bw),
        ])

        let logTitle = makeLabel("\u{25B8} LOGS", font: mono10b, color: T.accent)
        logHdr.addSubview(logTitle)
        NSLayoutConstraint.activate([
            logTitle.leadingAnchor.constraint(equalTo: logHdr.leadingAnchor, constant: 10),
            logTitle.centerYAnchor.constraint(equalTo: logHdr.centerYAnchor),
        ])

        logView = NSTextView()
        logView.font = mono10
        logView.textColor = T.muted
        logView.backgroundColor = T.bg
        logView.drawsBackground = true
        logView.isEditable = false
        logView.isSelectable = true
        logView.textContainerInset = NSSize(width: 8, height: 4)
        logView.translatesAutoresizingMaskIntoConstraints = false
        logWrap.addSubview(logView)
        NSLayoutConstraint.activate([
            logView.leadingAnchor.constraint(equalTo: logWrap.leadingAnchor, constant: 2),
            logView.trailingAnchor.constraint(equalTo: logWrap.trailingAnchor, constant: -2),
            logView.topAnchor.constraint(equalTo: logBorder.bottomAnchor, constant: 2),
            logView.bottomAnchor.constraint(equalTo: logWrap.bottomAnchor, constant: -2),
        ])

        let logSection = NSView()
        logSection.translatesAutoresizingMaskIntoConstraints = false
        logSection.addSubview(logWrap)
        NSLayoutConstraint.activate([
            logWrap.leadingAnchor.constraint(equalTo: logSection.leadingAnchor, constant: pad),
            logWrap.trailingAnchor.constraint(equalTo: logSection.trailingAnchor, constant: -pad),
            logWrap.topAnchor.constraint(equalTo: logSection.topAnchor),
            logWrap.bottomAnchor.constraint(equalTo: logSection.bottomAnchor),
            logSection.heightAnchor.constraint(equalToConstant: 120),
        ])
        mainStack.addArrangedSubview(logSection)

        // spacer
        let spacer3 = NSView()
        spacer3.translatesAutoresizingMaskIntoConstraints = false
        spacer3.heightAnchor.constraint(equalToConstant: 6).isActive = true
        mainStack.addArrangedSubview(spacer3)

        // ═══ FOOTER ═══
        let footer = NSView()
        footer.translatesAutoresizingMaskIntoConstraints = false
        footer.themedBg(T.surface)

        let footBorder = NSView()
        footBorder.translatesAutoresizingMaskIntoConstraints = false
        footBorder.themedBg(T.border)
        footer.addSubview(footBorder)
        NSLayoutConstraint.activate([
            footBorder.leadingAnchor.constraint(equalTo: footer.leadingAnchor),
            footBorder.trailingAnchor.constraint(equalTo: footer.trailingAnchor),
            footBorder.topAnchor.constraint(equalTo: footer.topAnchor),
            footBorder.heightAnchor.constraint(equalToConstant: T.bw),
        ])

        let footActions: [(String, NSColor, NSColor, Selector)] = [
            ("INSTALL",    T.fg,     T.border, #selector(onInstallConfig)),
            ("RESTORE",    T.fg,     T.border, #selector(onRestoreConfig)),
            ("ORIGINAL",   T.muted,  T.muted,  #selector(onRestoreOrig)),
            ("CODEX CLI",  T.accent, T.accent, #selector(onLaunchCLI)),
            ("DESKTOP",    T.accent, T.accent, #selector(onLaunchDesktop)),
            ("CONFIG",     T.fg,     T.border, #selector(onOpenConfig)),
        ]
        let footerBtns = NSStackView()
        footerBtns.orientation = .horizontal
        footerBtns.spacing = 6
        footerBtns.alignment = .centerY
        footerBtns.translatesAutoresizingMaskIntoConstraints = false
        for (title, fg, brd, action) in footActions {
            let btn = NSButton()
            btn.bezelStyle = .regularSquare
            btn.isBordered = true
            btn.target = self
            btn.action = action
            btn.wantsLayer = true
            btn.layer?.borderColor = brd.cgColor
            btn.layer?.borderWidth = T.bw
            btn.layer?.backgroundColor = T.surface.cgColor
            btn.font = mono10b
            let attrs: [NSAttributedString.Key: Any] = [.foregroundColor: fg, .font: mono10b]
            btn.attributedTitle = NSAttributedString(string: title, attributes: attrs)
            btn.sizeToFit()
            btn.translatesAutoresizingMaskIntoConstraints = false
            var f = btn.frame
            f.size.width += 10
            f.size.height = 24
            btn.setFrameSize(f.size)
            footerBtns.addArrangedSubview(btn)
        }
        footer.addSubview(footerBtns)
        NSLayoutConstraint.activate([
            footerBtns.leadingAnchor.constraint(equalTo: footer.leadingAnchor, constant: pad - 4),
            footerBtns.topAnchor.constraint(equalTo: footer.topAnchor, constant: T.bw + 4),
            footerBtns.heightAnchor.constraint(equalToConstant: 24),
        ])
        footer.heightAnchor.constraint(equalToConstant: 36).isActive = true
        mainStack.addArrangedSubview(footer)
    }

    // MARK: - UI helpers

    func fieldColumn(_ labelText: String, view: NSView) -> NSStackView {
        let lbl = makeLabel(labelText, font: mono10b, color: T.accent)
        view.translatesAutoresizingMaskIntoConstraints = false
        if let ctrl = view as? NSControl {
            ctrl.heightAnchor.constraint(equalToConstant: fieldH).isActive = true
        }
        let col = NSStackView(views: [lbl, view])
        col.orientation = .vertical
        col.spacing = 5
        col.alignment = .fill
        col.translatesAutoresizingMaskIntoConstraints = false
        return col
    }

    func makeInput() -> NSTextField {
        let f = NSTextField()
        styleInput(f)
        return f
    }

    func styleInput(_ f: NSTextField) {
        f.font = mono
        f.textColor = T.fg
        f.backgroundColor = T.bg
        f.drawsBackground = true
        f.isBezeled = false
        f.wantsLayer = true
        f.layer?.borderColor = T.border.cgColor
        f.layer?.borderWidth = T.bw
        f.cell?.sendsActionOnEndEditing = true
    }

    func makeCombo() -> NSPopUpButton {
        let p = NSPopUpButton()
        p.font = mono
        p.wantsLayer = true
        p.layer?.borderColor = T.border.cgColor
        p.layer?.borderWidth = T.bw
        p.layer?.backgroundColor = T.bg.cgColor
        p.translatesAutoresizingMaskIntoConstraints = false
        p.heightAnchor.constraint(equalToConstant: fieldH).isActive = true
        if let c = p.cell as? NSPopUpButtonCell {
            c.bezelStyle = .inline
            c.isBordered = false
            c.backgroundColor = T.bg
            c.arrowPosition = .arrowAtBottom
        }
        return p
    }

    func makeBtn(title: String, bg: NSColor, fg: NSColor, border: NSColor) -> NSButton {
        let b = NSButton()
        b.wantsLayer = true
        b.layer?.backgroundColor = bg.cgColor
        b.layer?.borderColor = border.cgColor
        b.layer?.borderWidth = T.bw
        b.bezelStyle = .regularSquare
        b.isBordered = false
        b.translatesAutoresizingMaskIntoConstraints = false
        let attrs: [NSAttributedString.Key: Any] = [.foregroundColor: fg, .font: mono12b]
        b.attributedTitle = NSAttributedString(string: title, attributes: attrs)
        return b
    }

    func log(_ msg: String, ok: Bool = true) {
        let ts = DateFormatter()
        ts.dateFormat = "HH:mm:ss"
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
            startBtn.isEnabled = false
            startBtn.layer?.backgroundColor = T.surface2.cgColor
            stopBtn.isEnabled = true
            stopBtn.layer?.backgroundColor = T.danger.cgColor
            setBtnTitle(stopBtn, "\u{23F9} STOP", fg: .white)
            statusDot.layer?.backgroundColor = T.accent.cgColor
            statusLabel.stringValue = "RUNNING"
            statusLabel.textColor = T.accent
            statusPortLabel.stringValue = ":\(port)"
        } else {
            startBtn.isEnabled = true
            startBtn.layer?.backgroundColor = T.accent.cgColor
            setBtnTitle(startBtn, "\u{25B6} START", fg: NSColor(red: 0.04, green: 0.04, blue: 0.04, alpha: 1))
            stopBtn.isEnabled = false
            stopBtn.layer?.backgroundColor = T.danger.withAlphaComponent(0.18).cgColor
            setBtnTitle(stopBtn, "\u{23F9} STOP", fg: T.danger)
            statusDot.layer?.backgroundColor = T.muted.cgColor
            statusLabel.stringValue = "STOPPED"
            statusLabel.textColor = T.muted
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
        log("Ready. Select provider \u{2192} validate \u{2192} start", ok: true)
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
            modelPopup.removeAllItems(); modelPopup.addItem(withTitle: "\u{2014} validate key to load \u{2014}")
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
        if key.isEmpty { validateLabel.stringValue = "\u{26A0} ENTER KEY FIRST"; validateLabel.textColor = T.fg; return }
        validateLabel.stringValue = "VALIDATING..."; validateLabel.textColor = T.muted
        validateBtn.isEnabled = false

        runPythonJSONAsync(command: "validate-key", args: ["--preset", preset, "--api-key", key]) { [weak self] result in
            guard let self = self else { return }
            if let r = result, r["valid"] as? Bool == true {
                self.validateLabel.stringValue = "\u{2713} VALID"; self.validateLabel.textColor = T.accent
                _ = self.runPythonJSON(command: "store-key", args: ["--preset", preset, "--api-key", key])
                self.currentApiKey = key
                self.log("\u{2713} Key valid for \(preset)")

                self.validateLabel.stringValue = "LOADING MODELS..."
                self.runPythonJSONAsync(command: "fetch-models", args: ["--preset", preset, "--api-key", key]) { [weak self] mr in
                    guard let self = self else { return }
                    self.validateBtn.isEnabled = true
                    if let models = mr?["models"] as? [String], !models.isEmpty {
                        self.availableModels = models
                        self.modelPopup.removeAllItems(); self.modelPopup.addItems(withTitles: models)
                        self.validateLabel.stringValue = "\u{2713} \(models.count) MODELS"; self.validateLabel.textColor = T.accent
                        self.log("\u{2713} \(models.count) models loaded for \(preset)")
                    } else {
                        self.validateLabel.stringValue = "\u{2713} VALID"; self.validateLabel.textColor = T.accent
                    }
                }
            } else {
                self.validateBtn.isEnabled = true
                self.validateLabel.stringValue = "\u{2717} INVALID"; self.validateLabel.textColor = T.danger
                self.log("\u{2717} Key validation failed", ok: false)
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
        log("Proxy started \u{2192} \(preset) on :\(port)")
    }

    @objc func onStop() {
        _ = runPythonJSON(command: "stop")
        setRunning(false, port: 0, name: "")
        log("Proxy stopped \u{2014} config restored")
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
        let smi = NSMenuItem(title: "\u{25CB} STOPPED", action: nil, keyEquivalent: "")
        smi.tag = 100; smi.isEnabled = false; m.addItem(smi)
        m.addItem(NSMenuItem.separator())
        m.addItem(withTitle: "\u{25B6} Start", action: #selector(startProxy), keyEquivalent: "s")
        m.addItem(withTitle: "\u{23F9} Stop", action: #selector(stopProxy), keyEquivalent: "x")
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
                mi?.title = "\u{25CF} RUNNING \u{00B7} localhost:\(s["port"] as? Int ?? 9090)"
                statusItem?.button?.image = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "Running")
            } else {
                mi?.title = "\u{25CB} STOPPED"
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
        a.informativeText = "Responses API \u{2192} Chat Completions / Anthropic\n\nRoute Codex Desktop through alternative providers."
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
