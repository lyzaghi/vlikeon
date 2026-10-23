import Foundation
import Network
import Security

/// Serves only the app's immutable, inventoried public resources. There are no HTTP
/// endpoints for saves, library data, filesystem access or native control commands.
final class AssetServer {
    private struct Asset: Decodable { let path: String; let bytes: UInt64; let mime: String }
    private struct Inventory: Decodable { let schema_version: Int; let assets: [Asset]; let streaming_origins: [String]? }
    private let queue = DispatchQueue(label: "com.omniplay.ios.assets")
    private var listener: NWListener?
    private var connections: [UUID: NWConnection] = [:]
    private var files: [String: Asset] = [:]
    private let directory: URL
    private let token: String
    private var streamingSources = ""
    private(set) var baseURL: URL?
    private var routePrefix: String { "/h/\(token)/" }

    init(directory: URL) throws {
        self.directory = directory.resolvingSymlinksInPath()
        var random = [UInt8](repeating: 0, count: 24)
        guard SecRandomCopyBytes(kSecRandomDefault, random.count, &random) == errSecSuccess else { throw OmniFailure.message("Secure local resource setup failed.") }
        token = random.map { String(format: "%02x", $0) }.joined()
        let raw = try Data(contentsOf: directory.appendingPathComponent("asset-index.json"))
        guard raw.count <= 2_097_152 else { throw OmniFailure.message("The bundled resource inventory is too large.") }
        let inventory = try JSONDecoder().decode(Inventory.self, from: raw)
        guard inventory.schema_version == 1, inventory.assets.count <= 8192 else { throw OmniFailure.message("The bundled resource inventory is invalid.") }
        let origins = inventory.streaming_origins ?? []
        guard origins.count <= 8 else { throw OmniFailure.message("The bundled streaming origin inventory is invalid.") }
        for origin in origins {
            guard origin.utf8.count <= 256, origin.utf8.allSatisfy({ $0 >= 33 && $0 <= 126 }), !origin.contains("'"), !origin.contains("\""), !origin.contains(";"), !origin.contains("\\"), let parts = URLComponents(string: origin), parts.scheme == "https", parts.host != nil, parts.user == nil, parts.password == nil, parts.path.isEmpty, parts.query == nil, parts.fragment == nil, (parts.port == nil || (1...65535).contains(parts.port!)), parts.string == origin else { throw OmniFailure.message("A streaming origin violates the bundled policy.") }
            streamingSources += " " + origin
        }
        for asset in inventory.assets {
            guard Self.safePath(asset.path), asset.bytes <= 1_073_741_824, !asset.mime.contains("\r"), !asset.mime.contains("\n"), files[asset.path] == nil else { throw OmniFailure.message("A bundled resource violates its policy.") }
            files[asset.path] = asset
        }
        guard files["index.html"] != nil else { throw OmniFailure.message("The bundled Game Hub is missing.") }
    }
    private static func safePath(_ path: String) -> Bool {
        guard !path.isEmpty, path.utf8.count <= 512, !path.contains("\\"), !path.contains(":"), !path.hasPrefix("/") else { return false }
        return path.split(separator: "/", omittingEmptySubsequences: false).allSatisfy { !$0.isEmpty && $0 != "." && $0 != ".." && !$0.contains(where: { $0.isNewline || $0.asciiValue == 0 }) }
    }
    func owns(_ url: URL?) -> Bool {
        guard let url = url, let base = baseURL else { return false }
        return url.scheme == "http" && url.host == "127.0.0.1" && url.port == base.port && url.user == nil && url.password == nil && url.path == routePrefix+"index.html" && url.query == nil
    }
    func start(completion: @escaping (Result<URL, Error>) -> Void) {
        queue.async {
            do {
                let parameters = NWParameters.tcp
                parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
                parameters.allowLocalEndpointReuse = false
                let listener = try NWListener(using: parameters)
                self.listener = listener
                var completed = false
                listener.stateUpdateHandler = { state in
                    switch state {
                    case .ready:
                        guard !completed, let port = listener.port, let url = URL(string: "http://127.0.0.1:\(port.rawValue)/h/\(self.token)/") else { return }
                        completed = true; self.baseURL = url
                        DispatchQueue.main.async { completion(.success(url.appendingPathComponent("index.html"))) }
                    case .failed(let error): if !completed { completed = true; DispatchQueue.main.async { completion(.failure(error)) } }
                    default: break
                    }
                }
                listener.newConnectionHandler = { connection in self.accept(connection) }
                listener.start(queue: self.queue)
            } catch { DispatchQueue.main.async { completion(.failure(error)) } }
        }
    }
    private func accept(_ connection: NWConnection) {
        guard connections.count < 32 else { connection.cancel(); return }
        let id = UUID(); connections[id] = connection
        connection.stateUpdateHandler = { [weak self] state in if case .cancelled = state { self?.connections.removeValue(forKey: id) } }
        connection.start(queue: queue)
        queue.asyncAfter(deadline: .now() + 90) { [weak connection] in connection?.cancel() }
        readHeaders(connection, bytes: Data())
    }
    private func readHeaders(_ connection: NWConnection, bytes: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 8192) { data, _, done, error in
            var raw = bytes; if let data = data { raw.append(data) }
            guard raw.count <= 16_384, error == nil else { connection.cancel(); return }
            if let delimiter = raw.range(of: Data([13,10,13,10])) { self.respond(connection, header: raw[..<delimiter.lowerBound]); return }
            if done { connection.cancel(); return }; self.readHeaders(connection, bytes: raw)
        }
    }
    private func respond(_ connection: NWConnection, header: Data.SubSequence) {
        guard let value = String(data: Data(header), encoding: .utf8), let base = baseURL else { connection.cancel(); return }
        let lines = value.components(separatedBy: "\r\n"), start = lines.first?.split(separator: " ") ?? []
        guard start.count == 3, ["GET","HEAD"].contains(String(start[0])), start[2] == "HTTP/1.1" else { reject(connection, code: "405 Method Not Allowed"); return }
        var host: String?
        for line in lines.dropFirst() {
            guard let colon = line.firstIndex(of: ":") else { reject(connection); return }
            let name = line[..<colon].lowercased(), content = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
            if name == "host" { guard host == nil else { reject(connection); return }; host = content.lowercased() }
            if name == "transfer-encoding" || (name == "content-length" && content != "0") { reject(connection); return }
        }
        guard host == "127.0.0.1:\(base.port!)", let path = String(start[1]).removingPercentEncoding, !path.contains("?"), !path.contains("#"), path.hasPrefix(routePrefix) else { reject(connection); return }
        let relative = String(path.dropFirst(routePrefix.count))
        guard Self.safePath(relative), let asset = files[relative] else { reject(connection); return }
        let url = directory.appendingPathComponent(relative).resolvingSymlinksInPath()
        guard url.path.hasPrefix(directory.path + "/") else { reject(connection); return }
        do {
            let file = try FileHandle(forReadingFrom: url), size = try file.seekToEnd()
            guard size == asset.bytes else { try? file.close(); reject(connection); return }; try file.seek(toOffset: 0)
            let headers = "HTTP/1.1 200 OK\r\nContent-Type: \(asset.mime)\r\nContent-Length: \(asset.bytes)\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; media-src 'self' blob:; connect-src 'self'\(streamingSources); worker-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\n\r\n"
            connection.send(content: Data(headers.utf8), completion: .contentProcessed { error in
                guard error == nil, start[0] == "GET" else { try? file.close(); connection.cancel(); return }; self.stream(file, connection: connection)
            })
        } catch { reject(connection) }
    }
    private func stream(_ file: FileHandle, connection: NWConnection) {
        do {
            let chunk = try file.read(upToCount: 128 * 1024) ?? Data()
            guard !chunk.isEmpty else { try? file.close(); connection.cancel(); return }
            connection.send(content: chunk, completion: .contentProcessed { error in if error != nil { try? file.close(); connection.cancel() } else { self.stream(file, connection: connection) } })
        } catch { try? file.close(); connection.cancel() }
    }
    private func reject(_ connection: NWConnection, code: String = "404 Not Found") {
        connection.send(content: Data("HTTP/1.1 \(code)\r\nContent-Length: 0\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n".utf8), completion: .contentProcessed { _ in connection.cancel() })
    }
    func stop() { queue.async { self.listener?.cancel(); self.connections.values.forEach { $0.cancel() }; self.connections.removeAll() } }
}
