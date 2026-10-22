import Foundation
import Security

/// Native-only synchronization device credentials. No item is synchronized to
/// iCloud or shared with browsing WebViews, and the recovery root is never stored.
final class SecureCredentials {
    private static let accessLock = NSLock()
    private static var protectedDataAvailable = false
    static func setProtectedDataAvailable(_ value: Bool) { accessLock.lock(); protectedDataAvailable = value; accessLock.unlock() }
    private func checkAccess(_ profile: UnsafePointer<UInt8>?) -> Int32 {
        guard matches(profile) else { return -3 }
        Self.accessLock.lock(); defer { Self.accessLock.unlock() }; return Self.protectedDataAvailable ? 0 : -2
    }
    private static let service = "org.omniplay.sync.device.v1"
    private let profile: UUID
    private let profileBytes: [UInt8]
    private let lock = NSLock()
    private static let maximum = 4096

    init(profile: UUID) {
        self.profile = profile
        var raw = profile.uuid
        self.profileBytes = withUnsafeBytes(of: &raw) { Array($0) }
    }
    private func matches(_ value: UnsafePointer<UInt8>?) -> Bool {
        guard let value = value else { return false }
        return Array(UnsafeBufferPointer(start: value, count: 16)) == profileBytes
    }
    private var query: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: Self.service,
         kSecAttrAccount as String: profile.uuidString.lowercased(),
         kSecAttrSynchronizable as String: false,
         kSecUseAuthenticationUI as String: kSecUseAuthenticationUIFail]
    }
    private func status(_ value: OSStatus) -> Int32 {
        if value == errSecSuccess { return 0 }
        if value == errSecItemNotFound { return -1 }
        if value == errSecInteractionNotAllowed { return -2 }
        return -3
    }
    private func read(_ boundProfile: UnsafePointer<UInt8>?, output: UnsafeMutablePointer<UInt8>?, capacity: Int) -> Int64 {
        guard matches(boundProfile), let output = output, capacity == Self.maximum else { return -3 }
        lock.lock(); defer { lock.unlock() }
        var request = query
        request[kSecReturnData as String] = true
        request[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let reply = SecItemCopyMatching(request as CFDictionary, &result)
        guard reply == errSecSuccess else { return Int64(status(reply)) }
        guard var data = result as? Data, !data.isEmpty, data.count <= capacity else { return -3 }
        // Best-effort mutation of this Data owner; Swift/OS internal copies are
        // outside an application's guaranteed zeroization boundary.
        defer { data.resetBytes(in: 0..<data.count) }
        data.copyBytes(to: output, count: data.count)
        return Int64(data.count)
    }
    private func write(_ boundProfile: UnsafePointer<UInt8>?, secret: UnsafePointer<UInt8>?, length: Int) -> Int32 {
        guard matches(boundProfile), let secret = secret, length > 0, length <= Self.maximum else { return -3 }
        lock.lock(); defer { lock.unlock() }
        var data = Data(bytes: secret, count: length)
        defer { data.resetBytes(in: 0..<data.count) }
        let changes: [String: Any] = [kSecValueData as String: data, kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        let updated = SecItemUpdate(query as CFDictionary, changes as CFDictionary)
        if updated != errSecItemNotFound { return status(updated) }
        var item = query
        item[kSecValueData as String] = data
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        return status(SecItemAdd(item as CFDictionary, nil))
    }
    private func remove(_ boundProfile: UnsafePointer<UInt8>?) -> Int32 {
        guard matches(boundProfile) else { return -3 }
        lock.lock(); defer { lock.unlock() }
        return status(SecItemDelete(query as CFDictionary))
    }
    /// Rust consumes the retained context even when opening the profile fails.
    /// Construct this table only immediately inside the native open call.
    static func callbacks(profile: UUID) -> OmniCredentialCallbacks {
        let context = Unmanaged.passRetained(SecureCredentials(profile: profile)).toOpaque()
        return OmniCredentialCallbacks(abi_version: 2, context: context, read: { context, profile, output, capacity in
            guard let context = context else { return -3 }
            return Unmanaged<SecureCredentials>.fromOpaque(context).takeUnretainedValue().read(profile, output: output, capacity: capacity)
        }, write: { context, profile, secret, length in
            guard let context = context else { return -3 }
            return Unmanaged<SecureCredentials>.fromOpaque(context).takeUnretainedValue().write(profile, secret: secret, length: length)
        }, remove: { context, profile in
            guard let context = context else { return -3 }
            return Unmanaged<SecureCredentials>.fromOpaque(context).takeUnretainedValue().remove(profile)
        }, release: { context in
            if let context = context { Unmanaged<SecureCredentials>.fromOpaque(context).release() }
        }, check: { context, profile in
            guard let context = context else { return -3 }
            return Unmanaged<SecureCredentials>.fromOpaque(context).takeUnretainedValue().checkAccess(profile)
        })
    }
}
