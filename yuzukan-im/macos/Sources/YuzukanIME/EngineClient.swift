import Foundation

/// Newline-delimited JSON-RPC 2.0 client for yuzukan-imserver.
///
/// Requests are written to the child's stdin; a dedicated reader queue
/// splits stdout on 0x0A and dispatches responses to pending completions.
/// Key processing uses the synchronous API (the IMK `handle` callback must
/// answer "consumed?" synchronously, the same trade-off Mozc makes); slow
/// or fire-and-forget calls use the async API.
class EngineClient {
    private let serverProcess: EngineProcess
    private var nextID = 1
    private let requestQueue = DispatchQueue(label: "dev.otofu1024.yuzukan.jsonrpc.request")

    /// 成功した応答の生の行とデコード結果。
    private typealias Reply = (line: Data, envelope: RpcResponseEnvelope)

    private struct PendingRequest {
        /// `result` をデコードする型。nil ならデコードしない。
        let resultType: RpcResultType?
        /// エラー応答・EOF・書き込み失敗・タイムアウトでは nil で呼ぶ。
        let completion: (Reply?) -> Void
    }

    private let lock = NSLock()
    private var pendingRequests: [Int: PendingRequest] = [:]

    /// Called after `init` succeeds so the UI can apply server-configured display settings.
    var onInit: ((InitResult) -> Void)?

    /// `autoInit` re-sends `init` whenever the server (re)starts. Tests
    /// disable it to avoid loading models.
    init(serverProcess: EngineProcess, autoInit: Bool = true) {
        self.serverProcess = serverProcess
        self.serverProcess.onRestart = { [weak self] in
            self?.startReaderLoop()
            if autoInit {
                self?.initAsync()
            }
        }
    }

    // MARK: - Engine methods

    func initAsync() {
        sendTypedRequest(method: "init", params: [:]) { [weak self] (result: InitResult?) in
            guard let self else { return }
            guard let result else {
                NSLog("YuzukanIME: engine init failed")
                return
            }
            self.serverProcess.resetBackoff()
            NSLog(
                "YuzukanIME: engine initialized (protocol v\(result.protocolVersion), model=\(result.modelName), theme=\(result.theme ?? "default"))"
            )
            self.onInit?(result)
        }
    }

    func processKeySync(_ key: EngineKeyEvent) -> KeyResult? {
        let params: [String: Any] = [
            "keysym": key.keysym,
            "modifiers": key.modifiers.jsonObject,
            "is_release": false,
        ]
        return keyResultSync(method: "process_key", params: params, timeout: 3.0)
    }

    func commitSync() -> KeyResult? {
        keyResultSync(method: "commit", params: [:], timeout: 1.0)
    }

    func saveLearningAsync() {
        register(method: "save_learning", params: [:], resultType: nil) { _ in }
    }

    func setSurroundingTextAsync(text: String, cursorPos: Int) {
        register(
            method: "set_surrounding_text",
            params: ["text": text, "cursor_pos": cursorPos],
            resultType: nil
        ) { _ in }
    }

    private func keyResultSync(method: String, params: [String: Any], timeout: TimeInterval)
        -> KeyResult?
    {
        waitForResponse(method: method, timeout: timeout) { completion in
            sendTypedRequest(method: method, params: params, completion: completion)
        }
    }

    // MARK: - JSON-RPC transport

    func startReaderLoop() {
        guard let stdout = serverProcess.stdoutPipe else { return }

        let queue = DispatchQueue(label: "dev.otofu1024.yuzukan.jsonrpc.reader")
        queue.async { [weak self] in
            let handle = stdout.fileHandleForReading
            var buffer = Data()
            // JSONDecoder はこの直列キュー専用にし、他のスレッドと共有しない。
            let decoder = makeProtocolDecoder()

            while true {
                // The long-lived reader must release Foundation temporaries after each read.
                let hasData = autoreleasepool {
                    let chunk = handle.availableData
                    if chunk.isEmpty {
                        // EOF: server terminated
                        self?.failAllPending()
                        return false
                    }
                    buffer.append(chunk)

                    while let newlineRange = buffer.range(of: Data([0x0A])) {
                        let lineData = buffer.subdata(in: buffer.startIndex..<newlineRange.lowerBound)
                        buffer.removeSubrange(buffer.startIndex...newlineRange.lowerBound)
                        guard !lineData.isEmpty else { continue }
                        self?.handleResponse(lineData, decoder: decoder)
                    }
                    return true
                }
                if !hasData { break }
            }
        }
    }

    /// `result` を生の JSON `Data` で受け取る。
    @discardableResult
    func sendRequest(
        method: String, params: [String: Any], completion: @escaping (Data?) -> Void
    ) -> Int {
        register(method: method, params: params, resultType: nil) { reply in
            completion(reply.flatMap { Self.rawResult(of: $0.line) })
        }
    }

    /// `result` を応答行から直接 `T` にデコードして受け取る。
    @discardableResult
    private func sendTypedRequest<T: Decodable & Sendable>(
        method: String, params: [String: Any], completion: @escaping (T?) -> Void
    ) -> Int {
        register(method: method, params: params, resultType: T.self) { reply in
            switch reply?.envelope.result {
            case .success(let value):
                completion(value as? T)
            case .failure(let error):
                NSLog("YuzukanIME: failed to decode \(method) result: \(error)")
                completion(nil)
            case nil:
                completion(nil)
            }
        }
    }

    @discardableResult
    private func register(
        method: String, params: [String: Any], resultType: RpcResultType?,
        completion: @escaping (Reply?) -> Void
    ) -> Int {
        lock.lock()
        let id = nextID
        nextID += 1
        pendingRequests[id] = PendingRequest(resultType: resultType, completion: completion)
        lock.unlock()

        let request: [String: Any] = [
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        ]

        requestQueue.async { [weak self] in
            guard let self,
                let stdin = self.serverProcess.stdinPipe,
                var data = try? JSONSerialization.data(withJSONObject: request)
            else {
                self?.takePending(id: id)?.completion(nil)
                return
            }
            data.append(0x0A)
            do {
                try stdin.fileHandleForWriting.write(contentsOf: data)
            } catch {
                NSLog("YuzukanIME: failed to write request: \(error)")
                self.takePending(id: id)?.completion(nil)
            }
        }
        return id
    }

    func sendRequestSync(method: String, params: [String: Any], timeout: TimeInterval) -> Data? {
        waitForResponse(method: method, timeout: timeout) { completion in
            sendRequest(method: method, params: params, completion: completion)
        }
    }

    /// `send` が返す要求 id の応答を、タイムアウトまで同期的に待つ。
    private func waitForResponse<T>(
        method: String, timeout: TimeInterval, send: (@escaping (T?) -> Void) -> Int
    ) -> T? {
        let semaphore = DispatchSemaphore(value: 0)
        var result: T?
        let id = send { value in
            result = value
            semaphore.signal()
        }
        if semaphore.wait(timeout: .now() + timeout) == .timedOut {
            NSLog("YuzukanIME: \(method) timed out after \(timeout)s")
            takePending(id: id)?.completion(nil)
            return nil
        }
        return result
    }

    private func handleResponse(_ lineData: Data, decoder: JSONDecoder) {
        // 要求は書き込み前に登録されるので、届いた応答の型は必ずここに含まれる。
        decoder.userInfo[RpcResponseEnvelope.resultTypesKey] = pendingResultTypes()
        let envelope: RpcResponseEnvelope
        do {
            envelope = try decoder.decode(RpcResponseEnvelope.self, from: lineData)
        } catch {
            NSLog("YuzukanIME: unparsable response line: \(error)")
            return
        }
        guard let id = envelope.id else {
            // id:null happens only for parse errors on our side; log and drop.
            NSLog("YuzukanIME: response without id: \(String(decoding: lineData, as: UTF8.self))")
            return
        }
        if let error = envelope.error {
            NSLog("YuzukanIME: engine error for request \(id): \(error.code) \(error.message)")
            takePending(id: id)?.completion(nil)
            return
        }
        takePending(id: id)?.completion((line: lineData, envelope: envelope))
    }

    /// 応答行から `result` を JSON `Data` として切り出す(型指定のない要求用)。
    private static func rawResult(of lineData: Data) -> Data? {
        guard
            let json = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any],
            let result = json["result"]
        else { return nil }
        return try? JSONSerialization.data(withJSONObject: result)
    }

    private func pendingResultTypes() -> [Int: RpcResultType] {
        lock.lock()
        defer { lock.unlock() }
        return pendingRequests.compactMapValues(\.resultType)
    }

    private func takePending(id: Int) -> PendingRequest? {
        lock.lock()
        defer { lock.unlock() }
        return pendingRequests.removeValue(forKey: id)
    }

    private func failAllPending() {
        lock.lock()
        let pending = pendingRequests
        pendingRequests.removeAll()
        lock.unlock()
        for (_, request) in pending {
            request.completion(nil)
        }
    }
}
