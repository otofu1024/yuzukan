import Foundation

// Swift-side mirror of yuzukan-im/core/src/server/protocol.rs.
// All positions (caret, attribute start/end) are in Unicode scalar values;
// convert to UTF-16 offsets before passing to IMK APIs.

struct KeyResult: Decodable {
    let consumed: Bool
    let actions: [EngineAction]
    let conversionMs: UInt64?
    let processKeyMs: UInt64?
}

struct PreeditAttr: Decodable {
    let start: Int
    let end: Int
    let style: String
}

struct CandidateItem: Decodable, Equatable {
    let text: String
    let description: String?
}

struct InitResult: Decodable, Sendable {
    let protocolVersion: Int
    let modelName: String
    let theme: String?
}

enum EngineAction: Decodable {
    case updatePreedit(text: String, caret: Int, attributes: [PreeditAttr])
    case showCandidates(
        candidates: [CandidateItem],
        cursor: Int?,
        page: Int,
        totalPages: Int,
        upperCandidates: [CandidateItem]?,
        upperCursor: Int?
    )
    case hideCandidates
    case commit(text: String)
    case updateAux(text: String)
    case hideAux

    private enum CodingKeys: String, CodingKey {
        case type, text, caret, attributes, candidates, cursor, page, totalPages
        case upperCandidates, upperCursor
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "update_preedit":
            self = .updatePreedit(
                text: try container.decode(String.self, forKey: .text),
                caret: try container.decode(Int.self, forKey: .caret),
                attributes: try container.decodeIfPresent([PreeditAttr].self, forKey: .attributes)
                    ?? []
            )
        case "show_candidates":
            self = .showCandidates(
                candidates: try container.decode([CandidateItem].self, forKey: .candidates),
                cursor: try container.decodeIfPresent(Int.self, forKey: .cursor),
                page: try container.decode(Int.self, forKey: .page),
                totalPages: try container.decode(Int.self, forKey: .totalPages),
                upperCandidates: try container.decodeIfPresent(
                    [CandidateItem].self, forKey: .upperCandidates),
                upperCursor: try container.decodeIfPresent(Int.self, forKey: .upperCursor)
            )
        case "hide_candidates":
            self = .hideCandidates
        case "commit":
            self = .commit(text: try container.decode(String.self, forKey: .text))
        case "update_aux":
            self = .updateAux(text: try container.decode(String.self, forKey: .text))
        case "hide_aux":
            self = .hideAux
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container, debugDescription: "unknown action type: \(type)")
        }
    }
}

/// JSON-RPC のエラー応答。
struct RpcErrorPayload: Decodable {
    let code: Int
    let message: String
}

/// `result` のデコード先として指定できる型。
typealias RpcResultType = any (Decodable & Sendable).Type

/// JSON-RPC の応答行。`result` の型は応答の id で決まるため、型引数ではなく
/// userInfo で受け取り、id・error・result を 1 回のデコードで取り出す。
struct RpcResponseEnvelope: Decodable {
    /// userInfo に `[Int: RpcResultType]` (要求 id → result の型) を渡すキー。
    static let resultTypesKey = CodingUserInfoKey(rawValue: "yuzukan.rpcResultTypes")!

    let id: Int?
    let error: RpcErrorPayload?
    /// 型指定のある要求の result。型指定がない要求やエラー応答では nil。
    /// result のデコードに失敗しても id は失わないよう `Result` で保持する。
    let result: Result<any Decodable, Error>?

    private enum CodingKeys: String, CodingKey {
        case id, error, result
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let id = try container.decodeIfPresent(Int.self, forKey: .id)
        let error = try container.decodeIfPresent(RpcErrorPayload.self, forKey: .error)
        let types = decoder.userInfo[Self.resultTypesKey] as? [Int: RpcResultType]
        if error == nil, let id, let type = types?[id] {
            result = Result { try container.decode(type, forKey: .result) }
        } else {
            result = nil
        }
        self.id = id
        self.error = error
    }
}

/// JSONDecoder configured for the engine protocol (snake_case keys).
func makeProtocolDecoder() -> JSONDecoder {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    return decoder
}
