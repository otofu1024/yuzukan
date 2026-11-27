import XCTest

@testable import YuzukanIME

final class EngineProtocolTests: XCTestCase {
    func decodeKeyResult(_ json: String) throws -> KeyResult {
        try makeProtocolDecoder().decode(KeyResult.self, from: Data(json.utf8))
    }

    func testDecodePreeditAction() throws {
        let json = """
            {"consumed":true,"actions":[{"attributes":[{"end":1,"start":0,"style":"underline"}],"caret":1,"text":"か","type":"update_preedit"}],"conversion_ms":0,"process_key_ms":0}
            """
        let result = try decodeKeyResult(json)
        XCTAssertTrue(result.consumed)
        guard case .updatePreedit(let text, let caret, let attributes) = result.actions[0] else {
            return XCTFail("expected update_preedit")
        }
        XCTAssertEqual(text, "か")
        XCTAssertEqual(caret, 1)
        XCTAssertEqual(attributes.count, 1)
        XCTAssertEqual(attributes[0].style, "underline")
    }

    func testDecodeShowCandidates() throws {
        let json = """
            {"consumed":true,"actions":[{"candidates":[{"description":"[全]カタカナ","text":"カ"},{"text":"か"}],"cursor":0,"page":0,"total_pages":3,"type":"show_candidates"}],"conversion_ms":11,"process_key_ms":42}
            """
        let result = try decodeKeyResult(json)
        guard
            case .showCandidates(
                let candidates, let cursor, let page, let totalPages,
                let upperCandidates, let upperCursor) =
                result.actions[0]
        else {
            return XCTFail("expected show_candidates")
        }
        XCTAssertEqual(candidates.count, 2)
        XCTAssertEqual(candidates[0].text, "カ")
        XCTAssertEqual(candidates[0].description, "[全]カタカナ")
        XCTAssertNil(candidates[1].description)
        XCTAssertEqual(cursor, 0)
        XCTAssertEqual(page, 0)
        XCTAssertEqual(totalPages, 3)
        XCTAssertNil(upperCandidates)
        XCTAssertNil(upperCursor)
        XCTAssertEqual(result.conversionMs, 11)
    }

    func testDecodeShowCandidatesWithUpperPanel() throws {
        let json = """
            {"consumed":true,"actions":[{"candidates":[{"text":"今日"},{"text":"京"}],"page":0,"total_pages":1,"type":"show_candidates","upper_candidates":[{"description":"[全]カタカナ","text":"キョウ"},{"description":"[全]ひらがな","text":"きょう"}],"upper_cursor":1}],"conversion_ms":0,"process_key_ms":1}
            """
        let result = try decodeKeyResult(json)
        guard
            case .showCandidates(
                let candidates, let cursor, let page, let totalPages,
                let upperCandidates, let upperCursor) =
                result.actions[0]
        else {
            return XCTFail("expected show_candidates")
        }
        XCTAssertEqual(candidates.count, 2)
        XCTAssertNil(cursor)
        XCTAssertEqual(page, 0)
        XCTAssertEqual(totalPages, 1)
        XCTAssertEqual(upperCursor, 1)
        XCTAssertEqual(upperCandidates?.count, 2)
        XCTAssertEqual(upperCandidates?[0].text, "キョウ")
        XCTAssertEqual(upperCandidates?[0].description, "[全]カタカナ")
        XCTAssertEqual(upperCandidates?[1].text, "きょう")
        XCTAssertEqual(upperCandidates?[1].description, "[全]ひらがな")
    }

    func testDecodeCommitAndHide() throws {
        let json = """
            {"consumed":true,"actions":[{"text":"書きます","type":"commit"},{"type":"hide_candidates"},{"type":"hide_aux"}],"conversion_ms":0,"process_key_ms":0}
            """
        let result = try decodeKeyResult(json)
        XCTAssertEqual(result.actions.count, 3)
        guard case .commit(let text) = result.actions[0] else {
            return XCTFail("expected commit")
        }
        XCTAssertEqual(text, "書きます")
        guard case .hideCandidates = result.actions[1] else {
            return XCTFail("expected hide_candidates")
        }
        guard case .hideAux = result.actions[2] else {
            return XCTFail("expected hide_aux")
        }
    }

    func testDecodeInitResult() throws {
        let json = """
            {"protocol_version":1,"model_name":"jinen-v1-small-q5"}
            """
        let result = try makeProtocolDecoder().decode(InitResult.self, from: Data(json.utf8))
        XCTAssertEqual(result.protocolVersion, 1)
        XCTAssertEqual(result.modelName, "jinen-v1-small-q5")
        XCTAssertNil(result.theme)
    }

    func testDecodeInitResultWithTheme() throws {
        let json = """
            {"protocol_version":1,"model_name":"jinen-v2-small-q5","theme":"yuzu-leaf"}
            """
        let result = try makeProtocolDecoder().decode(InitResult.self, from: Data(json.utf8))
        XCTAssertEqual(result.protocolVersion, 1)
        XCTAssertEqual(result.modelName, "jinen-v2-small-q5")
        XCTAssertEqual(result.theme, "yuzu-leaf")
    }

    func testCandidateThemeConfigParsing() {
        XCTAssertEqual(CandidateTheme(configName: nil), .system)
        XCTAssertEqual(CandidateTheme(configName: "yuzu"), .yuzu)
        XCTAssertEqual(CandidateTheme(configName: "yuzu-light"), .yuzuLight)
        XCTAssertEqual(CandidateTheme(configName: "yuzu_light"), .yuzuLight)
        XCTAssertEqual(CandidateTheme(configName: "yuzu-dark"), .yuzuDark)
        XCTAssertEqual(CandidateTheme(configName: "yuzu_dark"), .yuzuDark)
        XCTAssertEqual(CandidateTheme(configName: "yuzu-leaf"), .yuzuLeaf)
        XCTAssertEqual(CandidateTheme(configName: "yuzu-green"), .yuzuLeaf)
        XCTAssertEqual(CandidateTheme(configName: "yuzu_green"), .yuzuLeaf)
        XCTAssertEqual(CandidateTheme(configName: "system"), .system)
        XCTAssertEqual(CandidateTheme(configName: "monochrome"), .system)
        XCTAssertEqual(CandidateTheme(configName: "unknown-theme"), .system)
    }

    func testUnknownActionTypeFails() {
        let json = """
            {"consumed":true,"actions":[{"type":"warp_to_mars"}],"conversion_ms":0,"process_key_ms":0}
            """
        XCTAssertThrowsError(try decodeKeyResult(json))
    }
}
