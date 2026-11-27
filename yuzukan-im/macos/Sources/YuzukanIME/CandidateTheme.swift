import Cocoa

/// 候補ウィンドウ用の配色セット。
struct CandidatePalette {
    let background: NSColor
    let border: NSColor
    let selectionBackground: NSColor
    let text: NSColor
    let selectedText: NSColor
    let descriptionText: NSColor
    let selectedDescriptionText: NSColor
    let numberText: NSColor
    let selectedNumberText: NSColor
    let footerText: NSColor
    let separator: NSColor

    private static let yuzuYellow = NSColor(hex: 0xF9D030)
    private static let creamWhite = NSColor(hex: 0xFDFBF2)
    private static let charcoalBrown = NSColor(hex: 0x4A3E31)
    private static let softAmber = NSColor(hex: 0xFFAA00)

    static let yuzuLight = CandidatePalette(
        background: NSColor(hex: 0xFDFBF2, alpha: 0.98),
        border: NSColor(hex: 0xE3D9BD),
        selectionBackground: NSColor(hex: 0xF9D030, alpha: 0.42),
        text: charcoalBrown,
        selectedText: charcoalBrown,
        descriptionText: NSColor(hex: 0x4A3E31, alpha: 0.65),
        selectedDescriptionText: NSColor(hex: 0x4A3E31, alpha: 0.78),
        numberText: NSColor(hex: 0x4A3E31, alpha: 0.50),
        selectedNumberText: NSColor(hex: 0xB36B00),
        footerText: NSColor(hex: 0x4A3E31, alpha: 0.68),
        separator: NSColor(hex: 0x4A3E31, alpha: 0.14)
    )

    static let yuzuDark = CandidatePalette(
        background: NSColor(hex: 0x241E18, alpha: 0.98),
        border: charcoalBrown,
        selectionBackground: NSColor(hex: 0xF9D030, alpha: 0.24),
        text: creamWhite,
        selectedText: creamWhite,
        descriptionText: NSColor(hex: 0xFDFBF2, alpha: 0.62),
        selectedDescriptionText: NSColor(hex: 0xFDFBF2, alpha: 0.75),
        numberText: NSColor(hex: 0xFDFBF2, alpha: 0.48),
        selectedNumberText: softAmber,
        footerText: NSColor(hex: 0xFDFBF2, alpha: 0.66),
        separator: NSColor(hex: 0xFDFBF2, alpha: 0.14)
    )

    static let yuzuLeafLight = CandidatePalette(
        background: NSColor(hex: 0xFDFBF2, alpha: 0.98),
        border: NSColor(hex: 0x84A93E, alpha: 0.40),
        selectionBackground: NSColor(hex: 0x84A93E, alpha: 0.26),
        text: charcoalBrown,
        selectedText: charcoalBrown,
        descriptionText: NSColor(hex: 0x4A3E31, alpha: 0.65),
        selectedDescriptionText: NSColor(hex: 0x4A3E31, alpha: 0.78),
        numberText: NSColor(hex: 0x4A3E31, alpha: 0.50),
        selectedNumberText: NSColor(hex: 0x4D6A1F),
        footerText: NSColor(hex: 0x4A3E31, alpha: 0.68),
        separator: NSColor(hex: 0x4A3E31, alpha: 0.14)
    )

    static let yuzuLeafDark = CandidatePalette(
        background: NSColor(hex: 0x1E2219, alpha: 0.98),
        border: NSColor(hex: 0x3D4A2B),
        selectionBackground: NSColor(hex: 0x84A93E, alpha: 0.30),
        text: creamWhite,
        selectedText: creamWhite,
        descriptionText: NSColor(hex: 0xFDFBF2, alpha: 0.62),
        selectedDescriptionText: NSColor(hex: 0xFDFBF2, alpha: 0.75),
        numberText: NSColor(hex: 0xFDFBF2, alpha: 0.48),
        selectedNumberText: NSColor(hex: 0xA4C95A),
        footerText: NSColor(hex: 0xFDFBF2, alpha: 0.66),
        separator: NSColor(hex: 0xFDFBF2, alpha: 0.14)
    )

    static let systemLight = CandidatePalette(
        background: NSColor(calibratedWhite: 0.985, alpha: 0.98),
        border: NSColor(srgbRed: 0.80, green: 0.82, blue: 0.85, alpha: 1),
        selectionBackground: NSColor(srgbRed: 0.90, green: 0.93, blue: 0.97, alpha: 1),
        text: .labelColor,
        selectedText: .labelColor,
        descriptionText: .secondaryLabelColor,
        selectedDescriptionText: .secondaryLabelColor,
        numberText: .secondaryLabelColor,
        selectedNumberText: .labelColor,
        footerText: .secondaryLabelColor,
        separator: .separatorColor
    )

    static let systemDark = CandidatePalette(
        background: NSColor(srgbRed: 0.07, green: 0.08, blue: 0.09, alpha: 0.98),
        border: NSColor(srgbRed: 0.24, green: 0.27, blue: 0.30, alpha: 1),
        selectionBackground: NSColor(srgbRed: 0.14, green: 0.17, blue: 0.23, alpha: 1),
        text: .labelColor,
        selectedText: .labelColor,
        descriptionText: .secondaryLabelColor,
        selectedDescriptionText: .secondaryLabelColor,
        numberText: .secondaryLabelColor,
        selectedNumberText: .labelColor,
        footerText: .secondaryLabelColor,
        separator: .separatorColor
    )
}

/// 候補ウィンドウのカラーバリエーション。`config.toml` の `[display] theme` に対応する。
enum CandidateTheme: String, CaseIterable, Equatable, Sendable {
    case system = "system"
    case yuzu = "yuzu"
    case yuzuLight = "yuzu-light"
    case yuzuDark = "yuzu-dark"
    case yuzuLeaf = "yuzu-leaf"

    /// 設定文字列（ケバブ・スネークケースやエイリアスを含む）からテーマを解決する。
    init(configName: String?) {
        guard let configName else {
            self = .system
            return
        }
        let normalized =
            configName
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .replacingOccurrences(of: "_", with: "-")
        switch normalized {
        case "", "system", "monochrome":
            self = .system
        case "yuzu":
            self = .yuzu
        case "yuzu-light":
            self = .yuzuLight
        case "yuzu-dark":
            self = .yuzuDark
        case "yuzu-leaf", "yuzu-green":
            self = .yuzuLeaf
        default:
            NSLog("YuzukanIME: unknown theme '\(configName)', falling back to 'system'")
            self = .system
        }
    }

    /// 固定テーマ時にパネルへ適用する外観。自動追従テーマでは nil。
    var forcedAppearance: NSAppearance? {
        switch self {
        case .yuzuLight:
            return NSAppearance(named: .aqua)
        case .yuzuDark:
            return NSAppearance(named: .darkAqua)
        case .yuzu, .yuzuLeaf, .system:
            return nil
        }
    }

    /// 指定した外観に合う配色を返す。固定テーマでは `forcedAppearance` を優先する。
    func palette(for appearance: NSAppearance) -> CandidatePalette {
        let effective = forcedAppearance ?? appearance
        let isDark = effective.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        switch self {
        case .yuzu:
            return isDark ? .yuzuDark : .yuzuLight
        case .yuzuLight:
            return .yuzuLight
        case .yuzuDark:
            return .yuzuDark
        case .yuzuLeaf:
            return isDark ? .yuzuLeafDark : .yuzuLeafLight
        case .system:
            return isDark ? .systemDark : .systemLight
        }
    }
}

private extension NSColor {
    convenience init(hex: UInt32, alpha: CGFloat = 1.0) {
        let red = CGFloat((hex >> 16) & 0xff) / 255.0
        let green = CGFloat((hex >> 8) & 0xff) / 255.0
        let blue = CGFloat(hex & 0xff) / 255.0
        self.init(srgbRed: red, green: green, blue: blue, alpha: alpha)
    }
}
