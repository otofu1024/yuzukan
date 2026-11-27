import Cocoa

/// Custom candidate window (borderless non-activating NSPanel).
///
/// The engine pre-paginates: `show` receives only the visible page plus
/// page metadata, so this controller just renders rows. An optional aux
/// line (reading hint / model info from the engine) is shown as a footer.
class CandidateWindowController {
    private static let candidateFontSize: CGFloat = 18
    private static let footerFontSize: CGFloat = 13
    private static let numberFontSize: CGFloat = 13
    private static let minPanelWidth: CGFloat = 160
    private static let sourceSymbols = [
        ("📝", "学習", "clock.arrow.circlepath"),
        ("👤", "ユーザー", "person.crop.circle"),
        ("🤖", "AI", "cpu"),
        ("📚", "辞書", "book.closed"),
        ("🔄", "変換", "arrow.triangle.2.circlepath"),
        ("📅", "日付", "calendar"),
    ]

    private struct StyleKey: Equatable {
        let theme: CandidateTheme
        let appearance: NSAppearance.Name?
    }

    private(set) var theme: CandidateTheme = .system
    private let panel: NSPanel
    private let panelBackground: NSBox
    private let stackView: NSStackView
    private let upperPanel: NSPanel
    private let upperPanelBackground: NSBox
    private let upperStackView: NSStackView
    private struct CandidateRow {
        let view: NSStackView
        let numberLabel: NSTextField
        let candidateLabel: NSTextField
        var candidate: CandidateItem?
        var selected: Bool?
    }
    private struct UpperRow {
        let view: NSStackView
        let candidateLabel: NSTextField
        var candidate: CandidateItem?
        var selected: Bool?
    }
    private var rowViews: [CandidateRow] = []
    private var upperRowViews: [UpperRow] = []
    private let footerSeparator = NSBox()
    private let footerLabel = NSTextField(labelWithString: "")
    private var renderedFooter: String?
    private var renderedStyle: StyleKey?
    private var renderedUpperStyle: StyleKey?
    private var symbolImages: [String: NSImage] = [:]
    private var auxText: String?

    private struct PageState {
        let candidates: [CandidateItem]
        let cursor: Int?
        let page: Int
        let totalPages: Int
        let upperCandidates: [CandidateItem]?
        let upperCursor: Int?
    }
    private var pageState: PageState?

    init() {
        let (mainPanel, mainBackground, mainStackView) = Self.makePanel()
        panel = mainPanel
        panelBackground = mainBackground
        stackView = mainStackView
        let (upPanel, upBackground, upStackView) = Self.makePanel()
        upperPanel = upPanel
        upperPanelBackground = upBackground
        upperStackView = upStackView

        footerSeparator.boxType = .custom
        footerSeparator.titlePosition = .noTitle
        footerSeparator.borderWidth = 0
        footerSeparator.contentViewMargins = .zero
        footerSeparator.translatesAutoresizingMaskIntoConstraints = false
        footerSeparator.heightAnchor.constraint(equalToConstant: 1).isActive = true
        footerLabel.translatesAutoresizingMaskIntoConstraints = false
        footerLabel.setContentCompressionResistancePriority(.required, for: .horizontal)

        applyPanelChrome()
    }

    private static func makePanel() -> (NSPanel, NSBox, NSStackView) {
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: 200, height: 100),
            styleMask: [.nonactivatingPanel, .borderless],
            backing: .buffered,
            defer: true
        )
        panel.level = .popUpMenu
        panel.hidesOnDeactivate = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.ignoresMouseEvents = true

        let background = NSBox(frame: panel.contentView?.bounds ?? .zero)
        background.boxType = .custom
        background.titlePosition = .noTitle
        background.contentViewMargins = .zero
        background.cornerRadius = 14
        background.borderWidth = 1
        panel.contentView = background

        let stackView = NSStackView()
        stackView.orientation = .vertical
        stackView.alignment = .leading
        stackView.distribution = .fill
        stackView.spacing = 3
        stackView.edgeInsets = NSEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        stackView.translatesAutoresizingMaskIntoConstraints = false

        if let contentView = background.contentView {
            contentView.addSubview(stackView)
            NSLayoutConstraint.activate([
                stackView.topAnchor.constraint(equalTo: contentView.topAnchor),
                stackView.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
                stackView.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
                stackView.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
            ])
        }
        return (panel, background, stackView)
    }

    var isVisible: Bool { panel.isVisible || upperPanel.isVisible }

    /// カラーテーマを切り替え、表示中であれば即座に再描画する。
    func setTheme(_ newTheme: CandidateTheme) {
        guard theme != newTheme else { return }
        theme = newTheme
        let forced = newTheme.forcedAppearance
        panel.appearance = forced
        upperPanel.appearance = forced
        renderedStyle = nil
        renderedUpperStyle = nil
        symbolImages.removeAll()
        applyPanelChrome()
        if isVisible, pageState != nil {
            render(cursorRect: nil)
        }
    }

    private func resolvedAppearance(for targetPanel: NSPanel) -> NSAppearance {
        theme.forcedAppearance ?? targetPanel.appearance ?? NSApplication.shared.effectiveAppearance
    }

    private func applyPanelChrome() {
        let mainAppearance = resolvedAppearance(for: panel)
        let mainPalette = theme.palette(for: mainAppearance)
        mainAppearance.performAsCurrentDrawingAppearance {
            panelBackground.fillColor = mainPalette.background
            panelBackground.borderColor = mainPalette.border
            footerSeparator.fillColor = mainPalette.separator
        }
        let upperAppearance = resolvedAppearance(for: upperPanel)
        let upperPalette = theme.palette(for: upperAppearance)
        upperAppearance.performAsCurrentDrawingAppearance {
            upperPanelBackground.fillColor = upperPalette.background
            upperPanelBackground.borderColor = upperPalette.border
        }
    }

    /// `cursorRect: nil` reuses the rect from the previous `show` — the
    /// caller can skip its (synchronous, per-keystroke) client IPC while
    /// the panel is already on screen, since the composition anchor
    /// doesn't move mid-composition.
    func show(
        candidates: [CandidateItem],
        cursor: Int?,
        page: Int,
        totalPages: Int,
        upperCandidates: [CandidateItem]? = nil,
        upperCursor: Int? = nil,
        cursorRect: NSRect?
    ) {
        pageState = PageState(
            candidates: candidates,
            cursor: cursor,
            page: page,
            totalPages: totalPages,
            upperCandidates: upperCandidates,
            upperCursor: upperCursor)
        render(cursorRect: cursorRect)
    }

    /// Update the aux footer; re-renders in place if the window is visible.
    /// Pass `deferRender: true` when a `show`/`hide` follows in the same
    /// action batch, so the panel is rendered once per batch instead of
    /// once for the aux change and again for the candidates.
    func setAux(_ text: String?, deferRender: Bool = false) {
        auxText = text
        if !deferRender, isVisible, pageState != nil {
            render(cursorRect: nil)
        }
    }

    func hide() {
        pageState = nil
        panel.orderOut(nil)
        upperPanel.orderOut(nil)
    }

    private static func attributedCandidateText(
        _ candidate: CandidateItem, selected: Bool, palette: CandidatePalette
    ) -> NSAttributedString {
        let text = NSMutableAttributedString(
            string: candidate.text,
            attributes: [
                .font: NSFont.systemFont(ofSize: candidateFontSize),
                .foregroundColor: selected ? palette.selectedText : palette.text,
            ])
        if let description = candidate.description {
            text.append(NSAttributedString(
                string: "  \(description)",
                attributes: [
                    .font: NSFont.systemFont(ofSize: footerFontSize),
                    .foregroundColor: selected
                        ? palette.selectedDescriptionText : palette.descriptionText,
                ]))
        }
        return text
    }

    private static func applySelectionStyle(
        to view: NSView,
        selected: Bool,
        palette: CandidatePalette,
        appearance: NSAppearance
    ) {
        appearance.performAsCurrentDrawingAppearance {
            view.layer?.backgroundColor =
                selected ? palette.selectionBackground.cgColor : NSColor.clear.cgColor
        }
    }

    private func render(cursorRect: NSRect?) {
        guard let state = pageState else {
            hide()
            return
        }
        // An empty list still renders while the aux footer has text: a
        // source-filtered view narrowed to an empty source must show its
        // 「候補なし」 footer, not a silently vanishing panel.
        if state.candidates.isEmpty && (auxText ?? "").isEmpty {
            hide()
            return
        }

        let mainAppearance = resolvedAppearance(for: panel)
        let currentStyle = StyleKey(
            theme: theme,
            appearance: mainAppearance.bestMatch(from: [.darkAqua, .aqua]))
        let palette = theme.palette(for: mainAppearance)
        let styleChanged = renderedStyle != currentStyle
        if styleChanged {
            symbolImages.removeAll()
            renderedStyle = currentStyle
            mainAppearance.performAsCurrentDrawingAppearance {
                panelBackground.fillColor = palette.background
                panelBackground.borderColor = palette.border
                footerSeparator.fillColor = palette.separator
            }
        }
        var layoutChanged = rowViews.count != state.candidates.count
        while rowViews.count > state.candidates.count {
            let row = rowViews.removeLast()
            stackView.removeArrangedSubview(row.view)
            row.view.removeFromSuperview()
        }
        while rowViews.count < state.candidates.count {
            let row = addCandidateRow(number: rowViews.count + 1)
            rowViews.append(row)
        }
        for (index, candidate) in state.candidates.enumerated() {
            var row = rowViews[index]
            let candidateChanged = row.candidate != candidate
            let selected = state.cursor == index
            let selectionChanged = row.selected != selected
            if candidateChanged || styleChanged || selectionChanged {
                row.candidateLabel.attributedStringValue = Self.attributedCandidateText(
                    candidate, selected: selected, palette: palette)
                if candidateChanged {
                    row.candidate = candidate
                    layoutChanged = true
                }
            }
            if selectionChanged || styleChanged {
                row.numberLabel.textColor =
                    selected ? palette.selectedNumberText : palette.numberText
                row.numberLabel.font = NSFont.monospacedDigitSystemFont(
                    ofSize: Self.numberFontSize,
                    weight: selected ? .semibold : .regular)
                Self.applySelectionStyle(
                    to: row.view, selected: selected, palette: palette, appearance: mainAppearance)
                row.selected = selected
            }
            rowViews[index] = row
        }

        let upperCandidates = state.upperCandidates ?? []
        let hasUpper = !upperCandidates.isEmpty
        if hasUpper {
            let upperAppearance = resolvedAppearance(for: upperPanel)
            let currentUpperStyle = StyleKey(
                theme: theme,
                appearance: upperAppearance.bestMatch(from: [.darkAqua, .aqua]))
            let upperPalette = theme.palette(for: upperAppearance)
            let upperStyleChanged = renderedUpperStyle != currentUpperStyle
            if upperStyleChanged {
                renderedUpperStyle = currentUpperStyle
                upperAppearance.performAsCurrentDrawingAppearance {
                    upperPanelBackground.fillColor = upperPalette.background
                    upperPanelBackground.borderColor = upperPalette.border
                }
            }
            if upperRowViews.count != upperCandidates.count {
                layoutChanged = true
            }
            while upperRowViews.count > upperCandidates.count {
                let row = upperRowViews.removeLast()
                upperStackView.removeArrangedSubview(row.view)
                row.view.removeFromSuperview()
            }
            while upperRowViews.count < upperCandidates.count {
                upperRowViews.append(addUpperRow())
            }
            for (index, candidate) in upperCandidates.enumerated() {
                var row = upperRowViews[index]
                let candidateChanged = row.candidate != candidate
                let selected = state.upperCursor == index
                let selectionChanged = row.selected != selected
                if candidateChanged || upperStyleChanged || selectionChanged {
                    row.candidateLabel.attributedStringValue = Self.attributedCandidateText(
                        candidate, selected: selected, palette: upperPalette)
                    if candidateChanged {
                        row.candidate = candidate
                        layoutChanged = true
                    }
                }
                if selectionChanged || upperStyleChanged {
                    Self.applySelectionStyle(
                        to: row.view,
                        selected: selected,
                        palette: upperPalette,
                        appearance: upperAppearance)
                    row.selected = selected
                }
                upperRowViews[index] = row
            }
            if !upperPanel.isVisible {
                layoutChanged = true
            }
        } else if upperPanel.isVisible {
            upperPanel.orderOut(nil)
            layoutChanged = true
        }

        var footer = auxText ?? ""
        if state.totalPages > 1 {
            let page = "(\(state.page + 1)/\(state.totalPages))"
            if !footer.contains(page) {
                footer = footer.isEmpty ? page : "\(footer) \(page)"
            }
        }
        if renderedFooter != footer || styleChanged {
            if !footer.isEmpty {
                footerLabel.attributedStringValue = styledFooter(
                    footer, palette: palette, appearance: mainAppearance)
                if footerLabel.superview == nil {
                    for view in [footerSeparator, footerLabel] {
                        stackView.addArrangedSubview(view)
                        view.widthAnchor.constraint(equalTo: stackView.widthAnchor, constant: -16)
                            .isActive = true
                    }
                }
            } else if footerLabel.superview != nil {
                for view in [footerSeparator, footerLabel] {
                    stackView.removeArrangedSubview(view)
                    view.removeFromSuperview()
                }
            }
            renderedFooter = footer
            layoutChanged = true
        }

        if layoutChanged || cursorRect != nil || !panel.isVisible || (hasUpper && !upperPanel.isVisible) {
            positionPanels(cursorRect: cursorRect, hasUpper: hasUpper)
        }
    }

    private func addCandidateRow(number: Int) -> CandidateRow {
        let numberLabel = NSTextField(labelWithString: "\(number)")
        numberLabel.font = NSFont.systemFont(ofSize: Self.numberFontSize)
        numberLabel.alignment = .center
        numberLabel.translatesAutoresizingMaskIntoConstraints = false

        let numberColumn = NSView()
        numberColumn.translatesAutoresizingMaskIntoConstraints = false
        numberColumn.addSubview(numberLabel)
        NSLayoutConstraint.activate([
            numberColumn.widthAnchor.constraint(equalToConstant: 14),
            numberColumn.heightAnchor.constraint(equalToConstant: 21),
            numberLabel.centerXAnchor.constraint(equalTo: numberColumn.centerXAnchor),
            numberLabel.centerYAnchor.constraint(equalTo: numberColumn.centerYAnchor),
        ])

        let candidateLabel = NSTextField(labelWithString: "")
        candidateLabel.translatesAutoresizingMaskIntoConstraints = false
        candidateLabel.alignment = .left
        candidateLabel.setContentCompressionResistancePriority(.required, for: .horizontal)

        let row = NSStackView(views: [numberColumn, candidateLabel])
        row.translatesAutoresizingMaskIntoConstraints = false
        row.orientation = .horizontal
        row.alignment = .centerY
        row.distribution = .fill
        row.spacing = 6
        row.edgeInsets = NSEdgeInsets(top: 3, left: 6, bottom: 3, right: 10)
        row.wantsLayer = true
        row.layer?.cornerRadius = 8
        row.layer?.masksToBounds = true
        stackView.insertArrangedSubview(row, at: rowViews.count)
        row.widthAnchor.constraint(equalTo: stackView.widthAnchor, constant: -16).isActive = true
        return CandidateRow(view: row, numberLabel: numberLabel, candidateLabel: candidateLabel)
    }

    private func addUpperRow() -> UpperRow {
        let candidateLabel = NSTextField(labelWithString: "")
        candidateLabel.translatesAutoresizingMaskIntoConstraints = false
        candidateLabel.alignment = .left
        candidateLabel.setContentCompressionResistancePriority(.required, for: .horizontal)

        let row = NSStackView(views: [candidateLabel])
        row.translatesAutoresizingMaskIntoConstraints = false
        row.orientation = .horizontal
        row.alignment = .centerY
        row.distribution = .fill
        row.edgeInsets = NSEdgeInsets(top: 3, left: 10, bottom: 3, right: 10)
        row.wantsLayer = true
        row.layer?.cornerRadius = 8
        row.layer?.masksToBounds = true
        upperStackView.addArrangedSubview(row)
        NSLayoutConstraint.activate([
            row.widthAnchor.constraint(equalTo: upperStackView.widthAnchor, constant: -16),
            row.heightAnchor.constraint(greaterThanOrEqualToConstant: 27),
        ])
        return UpperRow(view: row, candidateLabel: candidateLabel)
    }

    private func styledFooter(
        _ text: String, palette: CandidatePalette, appearance: NSAppearance
    ) -> NSAttributedString {
        var text = text
        if text.hasPrefix("⚡") {
            text.removeFirst()
            text = text.trimmingCharacters(in: .whitespaces)
        }
        let source = text as NSString
        var symbols: [(NSRange, String, String)] = []
        // Match engine metadata tokens, preserving emoji in the user's reading and context.
        for (emoji, label, symbol) in Self.sourceSymbols {
            for token in ["[変換:\(emoji)]", "| \(emoji) \(label)"] {
                let tokenRange = source.range(of: token, options: .backwards)
                if tokenRange.location != NSNotFound {
                    symbols.append((source.range(of: emoji, range: tokenRange), symbol, label))
                }
            }
        }
        if text.hasPrefix("[☺]") {
            symbols.append((source.range(of: "☺"), "face.smiling", "絵文字入力"))
        }
        if text.contains("[変換"), text.contains(" | 推論:") {
            let range = source.range(of: " 🎯 ")
            if range.location != NSNotFound {
                symbols.append((source.range(of: "🎯", range: range), "scope", "変換範囲"))
            }
        }
        let font = NSFont.systemFont(ofSize: Self.footerFontSize)
        let result = NSMutableAttributedString(
            string: text, attributes: [.font: font, .foregroundColor: palette.footerText])
        for (range, symbol, description) in symbols.sorted(by: { $0.0.location > $1.0.location }) {
            if symbolImages[symbol] == nil {
                appearance.performAsCurrentDrawingAppearance {
                    let configuration = NSImage.SymbolConfiguration(
                        pointSize: Self.footerFontSize, weight: .regular
                    )
                    .applying(NSImage.SymbolConfiguration(paletteColors: [palette.footerText]))
                    symbolImages[symbol] = NSImage(
                        systemSymbolName: symbol, accessibilityDescription: description)?
                        .withSymbolConfiguration(configuration)
                }
            }
            let attachment = NSTextAttachment()
            attachment.image = symbolImages[symbol]
            attachment.bounds = NSRect(x: 0, y: -2, width: Self.footerFontSize, height: Self.footerFontSize)
            result.replaceCharacters(in: range, with: NSAttributedString(attachment: attachment))
        }
        return result
    }

    private var lastCursorRect: NSRect = .zero

    private func positionPanels(cursorRect: NSRect?, hasUpper: Bool) {
        if let rect = cursorRect {
            lastCursorRect = rect
        }
        let cursorRect = lastCursorRect

        stackView.layoutSubtreeIfNeeded()
        let contentSize = stackView.fittingSize
        let panelWidth = max(ceil(contentSize.width) + 2, Self.minPanelWidth)
        let panelHeight = ceil(contentSize.height) + 2

        var upperWidth: CGFloat = 0
        var upperHeight: CGFloat = 0
        if hasUpper {
            upperStackView.layoutSubtreeIfNeeded()
            let upperContentSize = upperStackView.fittingSize
            upperWidth = max(ceil(upperContentSize.width) + 2, Self.minPanelWidth)
            upperHeight = ceil(upperContentSize.height) + 2
        }

        let panelGap: CGFloat = 4
        let screen: NSScreen? =
            cursorRect == .zero
            ? NSScreen.main
            : (NSScreen.screens.first(where: { $0.frame.contains(cursorRect.origin) })
                ?? NSScreen.main)
        let visible = screen?.visibleFrame

        var mainFrame = NSRect(x: 100, y: 100, width: panelWidth, height: panelHeight)
        var upperFrame = NSRect.zero

        if cursorRect != .zero {
            let showAbove = visible.map {
                cursorRect.origin.y - panelHeight < $0.origin.y
            } ?? false
            let lineTop = cursorRect.height >= 8 ? cursorRect.maxY : cursorRect.origin.y + 22

            var mainY = showAbove ? lineTop : cursorRect.origin.y - panelHeight
            if hasUpper {
                let upperY: CGFloat
                if showAbove {
                    let stackedY = mainY + panelHeight + panelGap
                    upperY = visible.map { min(stackedY, $0.maxY - upperHeight) } ?? stackedY
                } else if let visible, lineTop + panelGap + upperHeight > visible.maxY {
                    upperY = cursorRect.origin.y - upperHeight
                    mainY = max(visible.origin.y, upperY - panelGap - panelHeight)
                } else {
                    upperY = lineTop + panelGap
                }
                let minX = visible?.minX ?? cursorRect.origin.x
                let upperX = visible.map {
                    min(max(cursorRect.origin.x, minX), max(minX, $0.maxX - upperWidth))
                } ?? cursorRect.origin.x
                upperFrame = NSRect(x: upperX, y: upperY, width: upperWidth, height: upperHeight)
            }
            mainFrame.origin = NSPoint(x: cursorRect.origin.x, y: mainY)
        } else if hasUpper {
            upperFrame = NSRect(
                x: mainFrame.origin.x,
                y: mainFrame.maxY + panelGap,
                width: upperWidth,
                height: upperHeight)
        }

        if panel.frame != mainFrame {
            panel.setFrame(mainFrame, display: true)
        }
        if !panel.isVisible {
            panel.orderFront(nil)
        }
        if hasUpper {
            if upperPanel.frame != upperFrame {
                upperPanel.setFrame(upperFrame, display: true)
            }
            if !upperPanel.isVisible {
                upperPanel.orderFront(nil)
            }
        }
    }
}
