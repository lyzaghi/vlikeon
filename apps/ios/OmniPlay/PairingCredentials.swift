import Foundation
import Security

/// Remote Play candidate only. Native host must wire protected-data events and
/// consume callbacks through the dedicated provider bridge; save-sync ABI is
/// deliberately separate. No WebView/JavaScript secret-returning method exists.
final class PairingCredentials {
    private static let service = "org.vlikeon.remote-play.credentials.v1"
    private static let accessLock = NSLock()
    private static var protectedDataAvailable = false
    private static var accessEpoch: UInt64 = 1
    private static var accessClosed = false
    private final class WeakProvider {
        weak var value:PairingCredentials?
        init(_ value:PairingCredentials){self.value=value}
    }
    private static var providers:[WeakProvider]=[]
    private static let nilID = UUID(uuidString: "00000000-0000-0000-0000-000000000000")!
    private let profile: UUID
    private let device: UUID
    private let nativeHandle:UInt64
    private let lock = NSLock()
    private static let recordBytes = 137

    init(profile: UUID, device: UUID, nativeHandle:UInt64) {
        self.profile=profile;self.device=device;self.nativeHandle=nativeHandle
        Self.accessLock.lock();Self.providers.removeAll{$0.value==nil};Self.providers.append(WeakProvider(self));Self.accessLock.unlock()
    }
    static func setProtectedDataAvailable(_ value: Bool) {
        accessLock.lock()
        // Close access before invalidating Rust. In particular, unlock must not
        // publish availability while an older HTTP/consent epoch is still live.
        protectedDataAvailable=false
        let exhausted=accessEpoch==UInt64.max
        if exhausted{accessClosed=true}
        if !exhausted{accessEpoch += 1};let epoch=accessEpoch
        providers.removeAll{$0.value==nil};let handles=providers.compactMap{$0.value?.nativeHandle}
        accessLock.unlock()
        // Never hold the Swift access lock through Rust registry admission or
        // queue behind NativeCore's potentially blocked synchronous vault/HTTP.
        let acknowledgments=handles.map{om_mobile_pairing_invalidate_epoch($0)}
        accessLock.lock();defer{accessLock.unlock()}
        if accessEpoch==epoch{protectedDataAvailable=value && !accessClosed && acknowledgments.allSatisfy{$0==1}}
    }
    private static func unlocked() -> Bool {
        accessLock.lock(); defer { accessLock.unlock() }; return protectedDataAvailable
    }
    private static func accessToken() -> UInt64? {
        accessLock.lock(); defer { accessLock.unlock() }; return protectedDataAvailable ? accessEpoch : nil
    }
    private static func permits(_ epoch: UInt64) -> Bool {
        accessLock.lock(); defer { accessLock.unlock() }; return protectedDataAvailable && accessEpoch == epoch
    }
    private func scope(_ pointer: UnsafePointer<OmniPairingAddress>?) -> (account: String, prefix: [UInt8])? {
        guard let pointer = pointer else { return nil }
        let value = pointer.pointee
        let p = UUID(uuid: value.profile), d = UUID(uuid: value.device)
        let item = UUID(uuid: value.item), generation = UUID(uuid: value.generation)
        guard p == profile, d == device, [p, d, item, generation].allSatisfy({ $0 != Self.nilID }),
              value.kind == 1 || value.kind == 2, value.kind != 1 || item == device else { return nil }
        let kind = value.kind == 1 ? "identity" : "pair"
        let account = "\(p.uuidString.lowercased())/\(d.uuidString.lowercased())/\(kind)/\(item.uuidString.lowercased())/\(generation.uuidString.lowercased())"
        var prefix = Array("VLKRP001".utf8) + [UInt8(value.kind)]
        for id in [p, d, item, generation] {
            var bytes = id.uuid
            prefix.append(contentsOf: withUnsafeBytes(of: &bytes) { Array($0) })
        }
        return (account, prefix)
    }
    private func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: Self.service, kSecAttrAccount as String: account,
         kSecAttrSynchronizable as String: false,
         kSecUseAuthenticationUI as String: kSecUseAuthenticationUIFail]
    }
    private func status(_ value: OSStatus) -> Int32 {
        if value == errSecSuccess { return 0 }
        if value == errSecItemNotFound { return -1 }
        if value == errSecInteractionNotAllowed { return -2 }
        return -3
    }
    private func check(_ address: UnsafePointer<OmniPairingAddress>?) -> Int32 {
        guard scope(address) != nil else { return -3 }; return Self.unlocked() ? 0 : -2
    }
    private func retirementMarker(_ prefix:[UInt8])->Data {
        // Public generation-bound tombstone, not a secret envelope. Keeping the
        // same account occupied prevents an old add-only writer recreating it.
        Data(Array("VLKRF001".utf8)+Array(prefix.dropFirst(8)))
    }
    private func read(_ address: UnsafePointer<OmniPairingAddress>?, output: UnsafeMutablePointer<UInt8>?, capacity: Int) -> Int64 {
        guard let scope = scope(address), let output = output, capacity == Self.recordBytes else { return -3 }
        lock.lock(); defer { lock.unlock() }; guard let epoch=Self.accessToken() else { return -2 }
        var request = query(scope.account)
        request[kSecReturnData as String] = true
        request[kSecReturnAttributes as String] = true
        request[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let reply = SecItemCopyMatching(request as CFDictionary, &result)
        guard Self.permits(epoch) else { return -2 }
        guard reply == errSecSuccess else { return Int64(status(reply)) }
        guard let attributes = result as? [String: Any],
              attributes[kSecAttrService as String] as? String == Self.service,
              attributes[kSecAttrAccount as String] as? String == scope.account,
              attributes[kSecAttrAccessible as String] as? String == (kSecAttrAccessibleWhenUnlockedThisDeviceOnly as String),
              var data = attributes[kSecValueData as String] as? Data else { return -3 }
        // Best effort for this Data owner; Swift/OS internal copies are outside
        // the application's guaranteed zeroization boundary.
        defer { data.resetBytes(in: 0..<data.count) }
        if data==retirementMarker(scope.prefix){return Self.permits(epoch) ? -1:-2}
        guard data.count == Self.recordBytes, data.starts(with: scope.prefix), Self.permits(epoch) else { return Self.permits(epoch) ? -3 : -2 }
        data.copyBytes(to: output, count: Self.recordBytes)
        guard Self.permits(epoch) else { for index in 0..<Self.recordBytes { output[index]=0 }; return -2 }
        return Int64(Self.recordBytes)
    }
    private func write(_ address: UnsafePointer<OmniPairingAddress>?, secret: UnsafePointer<UInt8>?, length: Int) -> Int32 {
        guard let scope = scope(address), let secret = secret, length == Self.recordBytes else { return -3 }
        lock.lock(); defer { lock.unlock() }; guard let epoch=Self.accessToken() else { return -2 }
        var data = Data(bytes: secret, count: length)
        defer { data.resetBytes(in: 0..<data.count) }
        guard data.starts(with: scope.prefix) else { return -3 }
        var item = query(scope.account)
        item[kSecValueData as String] = data
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        // New generation only. A duplicate is an error, never an update of old
        // or uncertain material. Rust performs matching read-back separately.
        let reply = SecItemAdd(item as CFDictionary, nil)
        return Self.permits(epoch) ? status(reply) : -2
    }
    private func remove(_ address: UnsafePointer<OmniPairingAddress>?) -> Int32 {
        guard let scope = scope(address) else { return -3 }
        lock.lock(); defer { lock.unlock() }; guard let epoch=Self.accessToken() else { return -2 }
        let marker=retirementMarker(scope.prefix)
        var item=query(scope.account);item[kSecValueData as String]=marker
        item[kSecAttrAccessible as String]=kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        // No delete/add gap. Normal writers remain SecItemAdd-only. If an old
        // add wins first, update its value in place; if the marker wins first,
        // the old add is a duplicate. Never report NotFound as a sealed fence.
        var reply=SecItemAdd(item as CFDictionary,nil)
        guard Self.permits(epoch) else{return -2}
        if reply==errSecDuplicateItem{
            let update:[String:Any]=[kSecValueData as String:marker,kSecAttrAccessible as String:kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
            reply=SecItemUpdate(query(scope.account) as CFDictionary,update as CFDictionary)
        }
        guard Self.permits(epoch) else{return -2}
        guard reply==errSecSuccess else{return reply==errSecItemNotFound ? -3:status(reply)}
        var verify=query(scope.account);verify[kSecReturnData as String]=true
        verify[kSecReturnAttributes as String]=true;verify[kSecMatchLimit as String]=kSecMatchLimitOne
        var result:CFTypeRef?;let observed=SecItemCopyMatching(verify as CFDictionary,&result)
        guard Self.permits(epoch) else{return -2}
        guard observed==errSecSuccess else{return observed==errSecItemNotFound ? -3:status(observed)}
        guard let attributes=result as? [String:Any],attributes[kSecAttrService as String] as? String==Self.service,
              attributes[kSecAttrAccount as String] as? String==scope.account,
              attributes[kSecAttrAccessible as String] as? String==(kSecAttrAccessibleWhenUnlockedThisDeviceOnly as String),
              var data=attributes[kSecValueData as String] as? Data else{return -3}
        defer{data.resetBytes(in:0..<data.count)}
        guard data==marker else{return -3};return Self.permits(epoch) ? 0:-2
    }
    /// Native-only factory. Rust consumes retained context even on rejection.
    static func callbacks(profile: UUID, device: UUID, nativeHandle:UInt64) -> OmniPairingCredentialCallbacks {
        let context = Unmanaged.passRetained(PairingCredentials(profile: profile, device: device,nativeHandle:nativeHandle)).toOpaque()
        return OmniPairingCredentialCallbacks(abi_version: 1, context: context, read: { context, address, output, capacity in
            guard let context = context else { return -3 }
            return Unmanaged<PairingCredentials>.fromOpaque(context).takeUnretainedValue().read(address, output: output, capacity: capacity)
        }, write: { context, address, secret, length in
            guard let context = context else { return -3 }
            return Unmanaged<PairingCredentials>.fromOpaque(context).takeUnretainedValue().write(address, secret: secret, length: length)
        }, remove: { context, address in
            guard let context = context else { return -3 }
            return Unmanaged<PairingCredentials>.fromOpaque(context).takeUnretainedValue().remove(address)
        }, check: { context, address in
            guard let context = context else { return -3 }
            return Unmanaged<PairingCredentials>.fromOpaque(context).takeUnretainedValue().check(address)
        }, release: { context in
            if let context = context { Unmanaged<PairingCredentials>.fromOpaque(context).release() }
        })
    }
}
