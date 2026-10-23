import Foundation

enum OmniFailure: Error, LocalizedError {
    case message(String)
    var errorDescription: String? { if case let .message(value) = self { return value }; return nil }
}

/// Safe to inspect from the serialized native queue after a WebKit document
/// has been replaced on the main queue. Already-running I/O is cooperative.
final class NativeDocumentLifetime {
    private let lock=NSLock()
    private var alive=true
    var active:Bool{lock.lock();defer{lock.unlock()};return alive}
    func invalidate(){lock.lock();alive=false;lock.unlock()}
}

/// Only this native object can call path-bearing authorization APIs. Each profile's
/// SQLite handle is serialized off the main thread; WebKit never sees its root path.
final class NativeCore {
    @MainActor private static var primaryCore: NativeCore?
    @MainActor static func primary(create: Bool = true) throws -> NativeCore? {
        if let value = primaryCore { return value }
        let key = "omniplay.primary.profile", profileID: UUID
        if let stored = UserDefaults.standard.string(forKey: key) {
            guard let parsed = UUID(uuidString: stored) else { throw OmniFailure.message("The local profile identity needs recovery.") }; profileID = parsed
        } else {
            guard create else { return nil }; profileID = UUID(); UserDefaults.standard.set(profileID.uuidString, forKey: key)
        }
        guard let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else { throw OmniFailure.message("Application storage is unavailable.") }
        let value = NativeCore(profileID: profileID, root: support.appendingPathComponent("OmniPlay/profiles/\(profileID.uuidString.lowercased())", isDirectory: true))
        primaryCore = value; return value
    }
    private let queue = DispatchQueue(label: "com.omniplay.mobile.core", qos: .userInitiated)
    private let lifecycleQueue = DispatchQueue(label: "com.omniplay.mobile.lifecycle", qos: .userInitiated)
    private let mediaQueue=DispatchQueue(label:"com.omniplay.mobile.media",qos:.userInitiated)
    private let mediaPermit=DispatchSemaphore(value:1)
    private let requestPermit=DispatchSemaphore(value:16)
    private let authorizationPermit=DispatchSemaphore(value:16)
    private let stateLock = NSLock()
    private var storedHandle: UInt64 = 0
    private var desiredForeground = false
    private var lifecycleScheduled = false
    private var lifecycleNeedsCancel = false
    private var lifecycleRevision: UInt64 = 0
    private var detachQueued=false
    private var handle: UInt64 {
        get { stateLock.lock(); defer { stateLock.unlock() }; return storedHandle }
        set { stateLock.lock(); storedHandle = newValue; stateLock.unlock() }
    }
    let profileID: UUID
    let root: URL

    init(profileID: UUID, root: URL) { self.profileID = profileID; self.root = root }
    private func consume(_ buffer: OmniBuffer) throws -> [String: Any] {
        defer { om_mobile_free(buffer) }
        guard buffer.length > 0, buffer.length <= 1_048_576, let pointer = buffer.data else { throw OmniFailure.message("The local service returned an invalid response.") }
        let data = Data(bytes: pointer, count: buffer.length)
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw OmniFailure.message("The local service response is invalid.") }
        return object
    }
    private func call(_ value: [String: Any], function: (UnsafePointer<UInt8>?, Int) -> OmniBuffer) throws -> [String: Any] {
        let bytes = try JSONSerialization.data(withJSONObject: value)
        guard !bytes.isEmpty, bytes.count <= 1_048_576 else { throw OmniFailure.message("The local request exceeds its size limit.") }
        return try bytes.withUnsafeBytes { raw in try consume(function(raw.bindMemory(to: UInt8.self).baseAddress, bytes.count)) }
    }
    private func unwrap(_ value: [String: Any]) throws -> [String: Any] {
        guard value["status"] as? String == "ok", let result = value["result"] as? [String: Any] else {
            throw OmniFailure.message((value["error"] as? [String: Any])?["message"] as? String ?? "The local operation failed.")
        }; return result
    }
    private func openNow() throws {
                if handle != 0 { return }
                try FileManager.default.createDirectory(at: self.root, withIntermediateDirectories: true, attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication])
                var location = self.root
                var attributes = URLResourceValues(); attributes.isExcludedFromBackup = true
                try location.setResourceValues(attributes)
                let value = try self.unwrap(self.call(["data_root": self.root.path, "profile_id": self.profileID.uuidString.lowercased()]) { data, length in
                    om_mobile_open_with_credentials_v2(data, length, SecureCredentials.callbacks(profile: self.profileID))
                })
                guard let number = value["handle"] as? NSNumber, number.uint64Value != 0 else { throw OmniFailure.message("The local profile did not open.") }
                self.handle = number.uint64Value
                self.refreshLifecycle()
    }
    func open(completion: @escaping (Result<Void, Error>) -> Void) {
        queue.async {
            do {
                try self.openNow()
                DispatchQueue.main.async { completion(.success(())) }
            } catch { DispatchQueue.main.async { completion(.failure(error)) } }
        }
    }
    func request(_ message: [String: Any], owner:NativeDocumentLifetime? = nil, completion: @escaping (Result<[String: Any], Error>) -> Void) {
        if (message["command"] as? [String:Any])?["method"] as? String == "asset_range_read"{rangeRequest(message,owner:owner,completion:completion);return}
        guard requestPermit.wait(timeout:.now()) == .success else{completion(.failure(OmniFailure.message("Native operations are still finishing. Please retry.")));return}
        queue.async {
            defer{self.requestPermit.signal()}
            do {
                try self.admit(message,owner:owner)
                guard self.handle != 0 else { throw OmniFailure.message("The local profile is closed.") }
                var request = message; request["profile_id"] = self.profileID.uuidString.lowercased()
                var value = try self.call(request) { om_mobile_request(self.handle, $0, $1) }
                let command=request["command"] as? [String:Any];let isDeviceSnapshot=command?["method"] as? String == "device_snapshot"
                if command?["method"] as? String == "hello",value["status"] as? String == "ok",var result=value["result"] as? [String:Any] {
                    var capabilities=result["capabilities"] as? [String] ?? [];if !capabilities.contains("disc_import"){capabilities.append("disc_import")}
                    result["capabilities"]=capabilities;result["disc_provider_protocol"]=1;value["result"]=result
                }
                let method = command?["method"] as? String ?? ""
                var policy: [String:Any]?
                if value["status"] as? String == "ok", method == "hello" || method == "settings_set" || method.hasPrefix("sync_") {
                    policy = try? self.unwrap(self.consume(om_mobile_background_policy(self.handle)))
                }
                DispatchQueue.main.async {
                    if let policy = policy {
                        let scheduled = BackgroundSync.shared.apply(policy)
                        if method == "sync_status", var result = value["result"] as? [String:Any] { result["background"] = scheduled; value["result"] = result }
                        if method == "sync_background_configure" { value["result"] = scheduled }
                    }
                    completion(.success(isDeviceSnapshot ? DeviceSnapshot.augment(value) : value))
                }
            } catch { DispatchQueue.main.async { completion(.failure(error)) } }
        }
    }
    private func admit(_ message:[String:Any],owner:NativeDocumentLifetime?)throws{
        guard owner?.active != false,let deadline=message["deadline_ms"] as? NSNumber,deadline.doubleValue.isFinite,deadline.doubleValue>Date().timeIntervalSince1970*1000 else{throw OmniFailure.message("The request expired or its document closed.")}
    }
    private func rangeRequest(_ message:[String:Any],owner:NativeDocumentLifetime?,completion:@escaping(Result<[String:Any],Error>)->Void){
        guard mediaPermit.wait(timeout:.now()) == .success else{completion(.failure(OmniFailure.message("A native media read is still finishing.")));return}
        mediaQueue.async{
            let result:Result<[String:Any],Error>
            do{try self.admit(message,owner:owner);guard self.handle != 0 else{throw OmniFailure.message("The local profile is closed.")};var request=message;request["profile_id"]=self.profileID.uuidString.lowercased();result = .success(try self.call(request){om_mobile_request(self.handle,$0,$1)})}catch{result = .failure(error)}
            self.mediaPermit.signal();DispatchQueue.main.async{completion(result)}
        }
    }
    func backgroundPolicy(completion: @escaping @MainActor (Result<[String:Any],Error>)->Void) {
        queue.async {
            do { try self.openNow(); let policy = try self.unwrap(self.consume(om_mobile_background_policy(self.handle))); DispatchQueue.main.async { completion(.success(policy)) } }
            catch { DispatchQueue.main.async { completion(.failure(error)) } }
        }
    }
    func beginBackground(_ run: NativeBackgroundRun, completion: @escaping @MainActor (Result<[String:Any],Error>)->Void) {
        queue.async {
            do {
                guard run.active else { throw OmniFailure.message("The background opportunity ended.") }
                try self.openNow()
                let remaining = run.remainingMilliseconds
                guard run.active, remaining >= 1_000 else { throw OmniFailure.message("The background opportunity ended.") }
                let snapshot = try self.unwrap(self.consume(om_mobile_background_start(self.handle, UInt32(min(25_000,remaining)))))
                guard let ticket = snapshot["id"] as? String else { throw OmniFailure.message("The background task returned no ticket.") }
                run.bind(handle: self.handle, ticket: ticket)
                DispatchQueue.main.async { completion(.success(snapshot)) }
            } catch { DispatchQueue.main.async { completion(.failure(error)) } }
        }
    }
    func authorize(_ nativeSelection: [String: Any], completion: @escaping (Result<[String: Any], Error>) -> Void) {
        guard authorizationPermit.wait(timeout:.now()) == .success else{completion(.failure(OmniFailure.message("Native document operations are still finishing.")));return}
        queue.async {
            defer{self.authorizationPermit.signal()}
            do {
                guard self.handle != 0 else { throw OmniFailure.message("The local profile is closed.") }
                let value = try self.unwrap(self.call(nativeSelection) { om_mobile_authorize(self.handle, $0, $1) })
                DispatchQueue.main.async { completion(.success(value)) }
            } catch { DispatchQueue.main.async { completion(.failure(error)) } }
        }
    }
    func expireCapabilities(completion: @escaping (Result<Void, Error>) -> Void) {
        guard authorizationPermit.wait(timeout:.now()) == .success else{completion(.failure(OmniFailure.message("Native document operations are still finishing.")));return}
        queue.async {
            defer{self.authorizationPermit.signal()}
            do {guard self.handle != 0 else{throw OmniFailure.message("The local profile is closed.")};_=try self.unwrap(self.consume(om_mobile_expire_capabilities(self.handle)));DispatchQueue.main.async{completion(.success(()))}}
            catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    func authorizeMedia(_ metadata: [String:Any], folder: SelectedDiscFolder, completion: @escaping (Result<[String:Any], Error>) -> Void) {
        guard authorizationPermit.wait(timeout:.now()) == .success else{folder.cancel();completion(.failure(OmniFailure.message("Native document operations are still finishing.")));return}
        queue.async {
            defer{self.authorizationPermit.signal()}
            do {
                guard self.handle != 0 else{throw OmniFailure.message("The local profile is closed.")}
                let value=try self.unwrap(self.call(metadata){data,length in om_mobile_authorize_media(self.handle,data,length,SelectedDiscFolder.callbacks(folder))})
                DispatchQueue.main.async{completion(.success(value))}
            }catch{folder.cancel();DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    func detach() {
        setForeground(false)
        stateLock.lock();guard !detachQueued else{stateLock.unlock();return};detachQueued=true;stateLock.unlock()
        queue.async{defer{self.stateLock.lock();self.detachQueued=false;self.stateLock.unlock()};if self.handle != 0{let value=om_mobile_detach(self.handle);om_mobile_free(value)}}
    }
    func suspend(completion: @escaping () -> Void = {}) {
        queue.async { if self.handle != 0 { let response = om_mobile_suspend(self.handle); om_mobile_free(response) }; DispatchQueue.main.async(execute: completion) }
    }
    func setForeground(_ value: Bool) {
        stateLock.lock(); desiredForeground = value; if !value { lifecycleNeedsCancel = true }; lifecycleRevision &+= 1; scheduleLifecycleLocked(); stateLock.unlock()
    }
    private func scheduleLifecycleLocked() {
        if !lifecycleScheduled { lifecycleScheduled = true; lifecycleQueue.async { self.drainLifecycle() } }
    }
    private func refreshLifecycle() {
        stateLock.lock(); lifecycleRevision &+= 1; scheduleLifecycleLocked(); stateLock.unlock()
    }
    private func drainLifecycle() {
        while true {
            stateLock.lock(); let active = desiredForeground; let cancel = lifecycleNeedsCancel; lifecycleNeedsCancel = false; let revision = lifecycleRevision; let owned = storedHandle; stateLock.unlock()
            if owned != 0 { if cancel { om_mobile_set_foreground(owned, 0) }; if active || !cancel { om_mobile_set_foreground(owned, active ? 1 : 0) } }
            stateLock.lock(); if revision == lifecycleRevision { lifecycleScheduled = false; stateLock.unlock(); return }; stateLock.unlock()
        }
    }
    deinit { let owned = handle; if owned != 0 { om_mobile_close(owned) } }
}
