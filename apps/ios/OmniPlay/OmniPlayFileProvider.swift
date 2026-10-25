import Foundation

/// File Provider and coordinated file management for OmniPlay saves and exports on iOS.
/// Implements strict path validation, sandbox boundary enforcement, and coordinated file access (NSFileCoordinator).
/// Strictly excludes internal databases (*.db, *.sqlite), keychain material, and private profile tokens.
public final class OmniPlayFileProvider: NSObject {

    public static let shared = OmniPlayFileProvider()

    public enum FileAccessError: Error {
        case invalidPath
        case accessDenied
        case fileNotFound
        case coordinatedReadFailed
        case coordinatedWriteFailed
    }

    private let fileManager = FileManager.default

    private var baseDirectory: URL? {
        fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?.appendingPathComponent("OmniPlay", isDirectory: true)
    }

    private override init() {
        super.init()
    }

    /// Validates that a requested file URL lies strictly within the allowed Saves or NativeExports directories.
    public func validatePathWithinSandbox(_ targetUrl: URL) -> Bool {
        guard let base = baseDirectory else { return false }
        let canonicalTarget = targetUrl.standardized.resolvingSymlinksInPath().path
        let canonicalBase = base.standardized.resolvingSymlinksInPath().path

        // Check root boundary
        guard canonicalTarget.hasPrefix(canonicalBase) else { return false }

        // Reject directory traversal attempts
        if canonicalTarget.contains("..") { return false }

        let lastComponent = targetUrl.lastPathComponent
        // Deny access to hidden files, SQLite files, journals, and credentials
        if lastComponent.hasPrefix(".") { return false }
        if lastComponent.hasSuffix(".db") || lastComponent.hasSuffix(".sqlite") ||
           lastComponent.contains("-wal") || lastComponent.contains("-shm") ||
           lastComponent.contains("profile-id") {
            return false
        }

        // Must be strictly inside Saves/ or NativeExports/ subdirectory
        let savesPath = base.appendingPathComponent("Saves", isDirectory: true).standardized.resolvingSymlinksInPath().path
        let exportsPath = base.appendingPathComponent("NativeExports", isDirectory: true).standardized.resolvingSymlinksInPath().path
        let savesPrefix = savesPath.hasSuffix("/") ? savesPath : savesPath + "/"
        let exportsPrefix = exportsPath.hasSuffix("/") ? exportsPath : exportsPath + "/"

        return canonicalTarget.hasPrefix(savesPrefix) || canonicalTarget.hasPrefix(exportsPrefix)
    }

    /// Performs an atomic, coordinated file read using NSFileCoordinator.
    public func coordinatedRead(at url: URL, completion: @escaping (Result<Data, FileAccessError>) -> Void) {
        guard validatePathWithinSandbox(url) else {
            completion(.failure(.accessDenied))
            return
        }

        let coordinator = NSFileCoordinator()
        var coordinatorError: NSError?
        var completed = false
        coordinator.coordinate(readingItemAt: url, options: .withoutChanges, error: &coordinatorError) { readUrl in
            completed = true
            do {
                let data = try Data(contentsOf: readUrl)
                completion(.success(data))
            } catch {
                completion(.failure(.coordinatedReadFailed))
            }
        }

        if !completed && coordinatorError != nil {
            completion(.failure(.coordinatedReadFailed))
        }
    }

    /// Performs an atomic, coordinated file write using NSFileCoordinator.
    public func coordinatedWrite(data: Data, to url: URL, completion: @escaping (Result<Void, FileAccessError>) -> Void) {
        guard validatePathWithinSandbox(url) else {
            completion(.failure(.accessDenied))
            return
        }

        let coordinator = NSFileCoordinator()
        var coordinatorError: NSError?
        var completed = false
        coordinator.coordinate(writingItemAt: url, options: .forReplacing, error: &coordinatorError) { writeUrl in
            completed = true
            do {
                try data.write(to: writeUrl, options: .atomic)
                completion(.success(()))
            } catch {
                completion(.failure(.coordinatedWriteFailed))
            }
        }

        if !completed && coordinatorError != nil {
            completion(.failure(.coordinatedWriteFailed))
        }
    }

    /// Lists exported save files safely without exposing profile internals.
    public func listExportedFiles() -> [URL] {
        guard let base = baseDirectory else { return [] }
        let exportsDir = base.appendingPathComponent("NativeExports")
        guard let enumerator = fileManager.enumerator(at: exportsDir, includingPropertiesForKeys: [.isRegularFileKey], options: [.skipsHiddenFiles, .skipsPackageDescendants]) else {
            return []
        }

        var results: [URL] = []
        for case let fileUrl as URL in enumerator {
            if validatePathWithinSandbox(fileUrl) {
                results.append(fileUrl)
            }
        }
        return results
    }
}
