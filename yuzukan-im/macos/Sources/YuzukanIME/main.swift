import Cocoa
import Darwin
import InputMethodKit

// Writing to a dead yuzukan-imserver must not kill the IME with SIGPIPE.
signal(SIGPIPE, SIG_IGN)

private func setupApplicationMenu() {
    // NSApp must be initialized before touching mainMenu, or this crashes.
    let app = NSApplication.shared

    let mainMenu = NSMenu()
    let appMenuItem = NSMenuItem()
    mainMenu.addItem(appMenuItem)

    let editMenuItem = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
    let editMenu = NSMenu(title: "Edit")
    editMenu.addItem(NSMenuItem(title: "Undo", action: Selector(("undo:")), keyEquivalent: "z"))
    editMenu.addItem(NSMenuItem(title: "Redo", action: Selector(("redo:")), keyEquivalent: "Z"))
    editMenu.addItem(NSMenuItem.separator())
    editMenu.addItem(NSMenuItem(title: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x"))
    editMenu.addItem(
        NSMenuItem(title: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c"))
    editMenu.addItem(
        NSMenuItem(title: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v"))
    editMenu.addItem(
        NSMenuItem(title: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a"))
    editMenuItem.submenu = editMenu
    mainMenu.addItem(editMenuItem)

    app.mainMenu = mainMenu
}

private func setupLogging() {
    let logDir = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Logs/YuzukanIME")
    try? FileManager.default.createDirectory(at: logDir, withIntermediateDirectories: true)

    let logFile = logDir.appendingPathComponent("yuzukan-ime.log")
    // 追記し続けて肥大化しないよう、起動時に 512 KiB 以上なら 1 世代だけ退避する。
    if let size = try? FileManager.default.attributesOfItem(atPath: logFile.path)[.size] as? UInt64,
        size >= 512 * 1024
    {
        let rotated = logDir.appendingPathComponent("yuzukan-ime.log.1")
        try? FileManager.default.removeItem(at: rotated)
        try? FileManager.default.moveItem(at: logFile, to: rotated)
    }
    if !FileManager.default.fileExists(atPath: logFile.path) {
        FileManager.default.createFile(atPath: logFile.path, contents: nil)
    }

    if let handle = FileHandle(forWritingAtPath: logFile.path) {
        handle.seekToEndOfFile()
        // stderr (NSLog and yuzukan-imserver tracing) goes to the log file.
        dup2(handle.fileDescriptor, STDERR_FILENO)
    }
}

private func connectionName() -> String {
    if let name = Bundle.main.object(forInfoDictionaryKey: "InputMethodConnectionName") as? String {
        return name
    }
    return (Bundle.main.bundleIdentifier ?? "dev.otofu1024.inputmethod.yuzukan") + "_Connection"
}

setupLogging()
setupApplicationMenu()
NSLog("YuzukanIME: starting")

guard
    let server = IMKServer(name: connectionName(), bundleIdentifier: Bundle.main.bundleIdentifier)
else {
    NSLog("YuzukanIME: failed to create IMKServer")
    exit(1)
}
_ = server  // keep the IMKServer alive

let engineProcess = EngineProcess()
let engineClient = EngineClient(serverProcess: engineProcess)
engineClient.onInit = { result in
    let theme = CandidateTheme(configName: result.theme)
    DispatchQueue.main.async {
        YuzukanInputController.candidateWindow.setTheme(theme)
    }
}

engineProcess.start()
engineClient.startReaderLoop()
// Model loading takes seconds (and downloads from HuggingFace on the very
// first run), so initialize in the background instead of on the first key.
engineClient.initAsync()

NSLog("YuzukanIME: IMKServer created")

// macOS can discard pipe connections during sleep; restart the engine
// process on wake to recover.
NSWorkspace.shared.notificationCenter.addObserver(
    forName: NSWorkspace.didWakeNotification,
    object: nil,
    queue: .main
) { _ in
    NSLog("YuzukanIME: wake from sleep — restarting yuzukan-imserver")
    engineProcess.restart()
}

NSApplication.shared.run()
