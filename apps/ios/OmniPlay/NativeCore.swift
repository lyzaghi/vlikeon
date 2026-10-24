import Foundation
import CoreFoundation

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

/// Public metadata owned by a native dialog, never sent to WebKit.
struct NativeIdentityReview {
    let id:String, publicKey:String, device:String, generation:String
    let expires:DispatchTime
}
enum NativeClientPairingAction:UInt32 { case begin=0,join=1,poll=2,persistAfterPeerConsent=3,cancel=4,activate=5,reviewSaved=6 }
enum NativeHostTicketAction:UInt32 {case register=1,poll=2,close=3}
/// Immutable approved-poll metadata for a trusted native confirmation view.
final class NativePeerApproval {
    let pairID:String,inputAllowed:Bool,expires:DispatchTime
    let grantExpiresAt:Date
    let hostPublicKey:String,hostFingerprint:String,comparisonCode:String
    fileprivate let ticket:String,grantHash:String
    fileprivate let origin:ObjectIdentifier
    private let lock=NSLock()
    private var consumed=false
    private var activationIssued=false
    private let current:()->Bool
    fileprivate init(origin:NativeCore,ticket:String,hash:String,pair:String,host:String,fingerprint:String,comparison:String,input:Bool,grantExpiresAt:Date,expires:DispatchTime,current:@escaping ()->Bool){
        self.origin=ObjectIdentifier(origin)
        self.grantExpiresAt=grantExpiresAt
        hostPublicKey=host;hostFingerprint=fingerprint;comparisonCode=comparison
        self.ticket=ticket;grantHash=hash;pairID=pair;inputAllowed=input;self.expires=expires;self.current=current
    }
    fileprivate func consume()->Bool{
        lock.lock();defer{lock.unlock()}
        guard !consumed,DispatchTime.now()<expires,current() else{return false}
        consumed=true;return true
    }
    fileprivate var admitted:Bool{DispatchTime.now()<expires&&current()}
    fileprivate func issueActivation()->Bool{
        lock.lock();defer{lock.unlock()}
        guard consumed,!activationIssued,admitted else{return false};activationIssued=true;return true
    }
}
final class NativeSavedPeerReview {
    let hostPublicKey:String,hostFingerprint:String,pairID:String,generation:String,audience:String
    let inputAllowed:Bool,grantExpiresAt:Date,expires:DispatchTime
    fileprivate let core:ObjectIdentifier,address:[String:Any],revision:UInt64,hash:String
    private let current:()->Bool
    private let lock=NSLock();private var consumed=false
    fileprivate init(core:NativeCore,value:[String:Any],expires:DispatchTime,current:@escaping ()->Bool) throws{
        guard let address=value["address"] as? [String:Any],let generation=address["generation"] as? String,
              let host=value["host_public_key"] as? String,let fingerprint=value["host_fingerprint"] as? String,
              let pair=value["pair_id"] as? String,let audience=value["audience"] as? String,let allowed=value["input_allowed"] as? Bool,
              let expiry=value["expires_at_ms"] as? NSNumber,let revision=value["revision"] as? NSNumber,let hash=value["grant_hash"] as? String else{throw OmniFailure.message("Thiếu dữ liệu ghép đôi đã lưu.")}
        self.core=ObjectIdentifier(core);self.address=address;self.revision=revision.uint64Value;self.hash=hash
        hostPublicKey=host;hostFingerprint=fingerprint;pairID=pair;self.generation=generation;self.audience=audience;inputAllowed=allowed
        grantExpiresAt=Date(timeIntervalSince1970:expiry.doubleValue/1000);self.expires=expires;self.current=current
    }
    fileprivate var admitted:Bool{DispatchTime.now()<expires&&current()}
    fileprivate func consume()->Bool{
        lock.lock();defer{lock.unlock()};guard !consumed,admitted else{return false};consumed=true;return true
    }
}
final class NativePeerActivation {
    let peer:NativePeerApproval,generation:String
    fileprivate let address:[String:Any],revision:UInt64
    private let lock=NSLock();private var consumed=false
    fileprivate init(peer:NativePeerApproval,address:[String:Any],revision:UInt64,generation:String){self.peer=peer;self.address=address;self.revision=revision;self.generation=generation}
    fileprivate func consume()->Bool{
        lock.lock();defer{lock.unlock()};guard !consumed,peer.admitted else{return false};consumed=true;return true
    }
}
private final class NativePairingDeliveryFence {
    final class Token {}
    private let lock=NSLock()
    private var token=Token()
    func capture(cancel:Bool)->Token {
        lock.lock();defer{lock.unlock()}
        if cancel{token=Token()}
        return token
    }
    func admits(_ candidate:Token)->Bool {
        lock.lock();defer{lock.unlock()};return token === candidate
    }
}
private enum NativePairingText {
    static func publicKey(_ text:String)->Bool {
        let bytes=Array(text.utf8)
        // A 32-byte unpadded base64url value has 43 ASCII characters;
        // the last character carries four data bits and two zero pad bits.
        return bytes.count==43 && bytes.allSatisfy{
            (65...90).contains($0)||(97...122).contains($0)||(48...57).contains($0)||$0==45||$0==95
        } && bytes.last.map{Array("AEIMQUYcgkosw048".utf8).contains($0)} == true
    }
    static func comparison(_ text:String)->Bool {
        text.utf8.count==6 && text.utf8.allSatisfy{(48...57).contains($0)}
    }
    static func fingerprint(_ text:String)->Bool {
        text.utf8.count==64 && text.utf8.allSatisfy{(48...57).contains($0)||(97...102).contains($0)}
    }
}
/// Consumes the caller's Data variable; Swift/OS aliases are outside this
/// owner's best-effort wipe guarantee. Never log or forward to WebKit.
private final class NativePairingInput {
    var bytes:Data
    init(taking source:inout Data){bytes=Data();swap(&bytes,&source)}
    func clear(){bytes.resetBytes(in:0..<bytes.count);bytes.removeAll(keepingCapacity:false)}
    deinit{clear()}
}
struct NativeRecoveryRecord {
    let address:[String:Any],phase:String,publicKey:String,nextStep:String?
    let sessionRecoveryRequired:Bool
    var generation:String{address["generation"] as? String ?? ""}
    var isIdentity:Bool{address["kind"] as? String == "device_signing"}
}
struct NativeRecoverySnapshot {
    let revision:UInt64,device:String,records:[NativeRecoveryRecord],expires:DispatchTime
    let relayRetirementAvailable:Bool
    let namespaceProvenanceMissing:Bool
    let pendingClientTicket:String?
    let pendingHostTicket:String?
    let pairingOrigins:[String]
    var needsAttention:Bool{namespaceProvenanceMissing || records.contains{$0.sessionRecoveryRequired || ($0.nextStep != nil && $0.nextStep != "revalidate_before_use")}}
}
/// Trusted native begin context. It contains public metadata, never invite proof.
final class NativeClientSetup {
    let relayOrigins:[String],identityKey:String,expires:DispatchTime
    fileprivate let origin:ObjectIdentifier,identity:[String:Any],revision:UInt64
    private let lock=NSLock();private var consumed=false
    private let current:()->Bool
    fileprivate init(core:NativeCore,identity:NativeRecoveryRecord,revision:UInt64,origins:[String],expires:DispatchTime,current:@escaping ()->Bool){
        origin=ObjectIdentifier(core);self.identity=identity.address;identityKey=identity.publicKey
        self.revision=revision;relayOrigins=origins;self.expires=expires;self.current=current
    }
    fileprivate func consume()->Bool{
        lock.lock();defer{lock.unlock()}
        guard !consumed,DispatchTime.now()<expires,current() else{return false};consumed=true;return true
    }
    fileprivate var admitted:Bool{DispatchTime.now()<expires&&current()}
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
    private let pairingClientPermit=DispatchSemaphore(value:1)
    private let pairingClientDeliveryFence=NativePairingDeliveryFence()
    private let pairingClientQueuePermit=DispatchSemaphore(value:2)
    private let authorizationPermit=DispatchSemaphore(value:16)
    private let stateLock = NSLock()
    private var storedHandle: UInt64 = 0
    private var desiredForeground = false
    private var lifecycleScheduled = false
    private var lifecycleNeedsCancel = false
    private var lifecycleRevision: UInt64 = 0
    private var detachQueued=false
    private var pairingAttached=false
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
                // Trusted native container paths may contain /var aliases;
                // resolve before Rust pins the profile lease namespace.
                var location = self.root.resolvingSymlinksInPath().standardizedFileURL
                var attributes = URLResourceValues(); attributes.isExcludedFromBackup = true
                try location.setResourceValues(attributes)
                let value = try self.unwrap(self.call(["data_root": location.path, "profile_id": self.profileID.uuidString.lowercased()]) { data, length in
                    om_mobile_open_with_credentials_v2(data, length, SecureCredentials.callbacks(profile: self.profileID))
                })
                guard let number = value["handle"] as? NSNumber, number.uint64Value != 0 else { throw OmniFailure.message("The local profile did not open.") }
                self.handle = number.uint64Value
                // Attachment does not read/create credentials or advertise
                // streaming. Local play remains usable if this provider fails.
                self.pairingAttached = (try? self.attachPairingProvider()) ?? false
                self.refreshLifecycle()
    }
    private func attachPairingProvider() throws -> Bool {
        let scope=try unwrap(consume(om_mobile_pairing_scope(handle)))
        guard scope["profile_id"] as? String == profileID.uuidString.lowercased(),
              let deviceText=scope["device_id"] as? String,let device=UUID(uuidString:deviceText),
              device.uuidString.lowercased()==deviceText,
              device != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else { return false }
        // Construct the retained table only inside the consuming call. Rust
        // owns/retires its context on every result, including rejection.
        let result=try unwrap(consume(om_mobile_attach_pairing_credentials_v1(handle,
            PairingCredentials.callbacks(profile:profileID,device:device,nativeHandle:handle))))
        return result["attached"] as? Bool == true && (result["credential_abi"] as? NSNumber)?.uint32Value == 1
    }
    /// Native UI only. Existing/uncertain identities require explicit recovery;
    /// opening this flow never silently replaces them or activates a peer.
    func reviewPairingIdentity(owner:NativeDocumentLifetime,completion:@escaping @MainActor (Result<NativeIdentityReview,Error>)->Void){
        queue.async {
            do {
                guard owner.active,self.handle != 0,self.pairingAttached else{throw OmniFailure.message("Kho khóa Remote Play chưa sẵn sàng.")}
                let status=try self.unwrap(self.consume(om_mobile_pairing_status(self.handle)))
                guard status["policy_available"] as? Bool == true else{throw OmniFailure.message("Remote Play chưa được bật trong phiên bản này.")}
                guard let records=status["records"] as? [[String:Any]],let revision=status["revision"] as? NSNumber,
                      let device=status["device_id"] as? String,status["profile_id"] as? String == self.profileID.uuidString.lowercased() else{throw OmniFailure.message("Trạng thái danh tính không hợp lệ.")}
                let identities=records.filter{($0["address"] as? [String:Any])?["kind"] as? String == "device_signing" && $0["phase"] as? String != "retired"}
                var expected=revision.uint64Value
                guard owner.active else{throw OmniFailure.message("Phiên xác nhận đã kết thúc.")}
                if identities.isEmpty {
                    let prepared=try self.unwrap(self.consume(om_mobile_pairing_prepare_identity_v1(self.handle,expected,25_000)))
                    guard prepared["phase"] as? String == "material_ready",let next=prepared["revision"] as? NSNumber else{throw OmniFailure.message("Hãy đọc lại trạng thái danh tính trước khi tiếp tục.")}
                    expected=next.uint64Value
                } else {
                    guard identities.count==1,identities[0]["phase"] as? String == "material_ready" else{throw OmniFailure.message("Danh tính đã tồn tại hoặc cần phục hồi. Không tạo khóa thay thế tự động.")}
                }
                guard owner.active else{throw OmniFailure.message("Phiên xác nhận đã kết thúc. Hãy đọc lại trạng thái danh tính.")}
                let value=try self.unwrap(self.consume(om_mobile_pairing_review_identity_v1(self.handle,expected)))
                guard owner.active,let id=value["review_id"] as? String,UUID(uuidString:id) != nil,
                      (value["credential_protocol"] as? NSNumber)?.uint32Value==1,(value["revision"] as? NSNumber)?.uint64Value==expected,
                      value["action"] as? String == "activate_local_identity",value["approves_remote_peer"] as? Bool == false,
                      let address=value["address"] as? [String:Any],address["profile"] as? String == self.profileID.uuidString.lowercased(),
                      address["device"] as? String == device,address["id"] as? String == device,address["kind"] as? String == "device_signing",
                      let generation=address["generation"] as? String,UUID(uuidString:generation) != nil,
                      let key=value["public_key"] as? String,key.count==43,key.utf8.allSatisfy({(65...90).contains($0)||(97...122).contains($0)||(48...57).contains($0)||$0==45||$0==95}),
                      let ttl=value["expires_in_ms"] as? NSNumber,ttl.doubleValue.isFinite,ttl.doubleValue>0,ttl.doubleValue<=60_000 else{
                    let cancelled=om_mobile_pairing_cancel_identity_review(self.handle);om_mobile_free(cancelled)
                    throw OmniFailure.message("Phiên xác nhận danh tính không hợp lệ hoặc đã kết thúc.")
                }
                let review=NativeIdentityReview(id:id,publicKey:key,device:device,generation:generation,
                    expires:.now()+ttl.doubleValue/1000)
                DispatchQueue.main.async{completion(.success(review))}
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    /// Call only from the native dialog's explicit approval action.
    func confirmPairingIdentity(_ review:NativeIdentityReview,owner:NativeDocumentLifetime,completion:@escaping @MainActor (Result<Void,Error>)->Void){
        queue.async {
            do {
                guard owner.active,DispatchTime.now()<review.expires,self.handle != 0,self.pairingAttached else{throw OmniFailure.message("Phiên xác nhận đã hết hạn. Hãy mở lại trạng thái danh tính.")}
                let value=try self.unwrap(self.call(["review_id":review.id]){om_mobile_pairing_confirm_identity_v1(self.handle,$0,$1,25_000)})
                guard owner.active,value["phase"] as? String == "active",value["approves_remote_peer"] as? Bool == false,
                      let address=value["address"] as? [String:Any],address["profile"] as? String == self.profileID.uuidString.lowercased(),
                      address["device"] as? String == review.device,address["id"] as? String == review.device,
                      address["generation"] as? String == review.generation,address["kind"] as? String == "device_signing" else{throw OmniFailure.message("Hãy đọc lại trạng thái danh tính; không tự động gửi lại xác nhận.")}
                DispatchQueue.main.async{completion(.success(()))}
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    func cancelPairingIdentityReview(){
        let owned=handle;if owned != 0{_ = om_mobile_pairing_invalidate_epoch(owned)}
        queue.async{if self.handle != 0{let value=om_mobile_pairing_cancel_identity_review(self.handle);om_mobile_free(value)}}
    }
    /// Trusted native UI only. The UI independently confirms host key and
    /// comparison/input consent before persistAfterPeerConsent. No renderer
    /// request may call this or receive its raw replies/tickets.
    @MainActor func pairingHostTicket(_ action:NativeHostTicketAction,ticket:String,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<[String:Any],Error>)->Void){
        guard let id=UUID(uuidString:ticket),id.uuidString.lowercased()==ticket,id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else{completion(.failure(OmniFailure.message("Ticket host không hợp lệ.")));return}
        let closing=action == .close,token=pairingClientDeliveryFence.capture(cancel:action == .close)
        if closing{
            guard handle != 0,om_mobile_pairing_invalidate_epoch(handle)==1 else{completion(.failure(OmniFailure.message("Chưa xác nhận được vô hiệu hóa lời mời. Hãy xem trạng thái.")));return}
        }
        let reserved = !closing
        if reserved && pairingClientPermit.wait(timeout:.now()) != .success{completion(.failure(OmniFailure.message("Thao tác ghép đôi trước đang kết thúc.")));return}
        guard pairingClientQueuePermit.wait(timeout:.now()) == .success else{if reserved{pairingClientPermit.signal()};completion(.failure(OmniFailure.message("Hàng đợi host đang bận.")));return}
        let deadline=DispatchTime.now()+25
        queue.async{
            defer{self.pairingClientQueuePermit.signal();if reserved{self.pairingClientPermit.signal()}}
            do{
                let now=DispatchTime.now()
                guard now<deadline,self.handle != 0,self.pairingAttached,owner.active,self.pairingClientDeliveryFence.admits(token) else{throw OmniFailure.message("Phiên host đã kết thúc.")}
                let remaining=(deadline.uptimeNanoseconds-now.uptimeNanoseconds)/1_000_000
                guard remaining>=1000 else{throw OmniFailure.message("Yêu cầu host hết hạn trong hàng đợi.")}
                var input=try JSONSerialization.data(withJSONObject:["ticket":ticket]);defer{input.resetBytes(in:0..<input.count)}
                let value=try self.unwrap(input.withUnsafeBytes{raw in try self.consume(om_mobile_pairing_host_action_v1(self.handle,raw.bindMemory(to:UInt8.self).baseAddress,raw.count,action.rawValue,UInt32(min(25_000,remaining))))})
                func flag(_ value:Any?,_ expected:Bool)->Bool{guard let n=value as? NSNumber,CFGetTypeID(n)==CFBooleanGetTypeID() else{return false};return n.boolValue==expected}
                guard (value["pairing_protocol"] as? NSNumber)?.uint32Value==1,value["ticket"] as? String==ticket else{throw OmniFailure.message("Phản hồi host không khớp ticket.")}
                if action == .register{
                    guard flag(value["registered"],true),flag(value["share_available"],true),flag(value["approves_peer"],false) else{throw OmniFailure.message("Phản hồi đăng ký không hợp lệ.")}
                }else if action == .poll{
                    guard flag(value["local_consent_required"],true),let peers=value["peers"] as? [[String:Any]],peers.count<=8,
                          let revision=value["revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),revision.doubleValue>=1,revision.doubleValue<=10_000_000,revision.doubleValue==Double(revision.uint64Value) else{throw OmniFailure.message("Danh sách peer không hợp lệ.")}
                    var seen=Set<String>()
                    for peer in peers{
                        guard let key=peer["client_public_key"] as? String,NativePairingText.publicKey(key),seen.insert(key).inserted,
                              let hash=peer["hello_hash"] as? String,NativePairingText.fingerprint(hash),let code=peer["comparison_code"] as? String,NativePairingText.comparison(code),
                              let label=peer["label"] as? String,!label.isEmpty,label.utf8.count<=120,!label.unicodeScalars.contains(where:{CharacterSet.controlCharacters.contains($0)}) else{throw OmniFailure.message("Metadata peer không hợp lệ.")}
                    }
                }else{
                    guard flag(value["closed"],true),flag(value["pairs_revoked"],false),flag(value["relay_close_confirmed"],true)||(flag(value["relay_close_confirmed"],false)&&flag(value["remote_close_required"],false)&&value["reason"] as? String=="never_registered") else{throw OmniFailure.message("Chưa xác nhận đúng kết quả đóng lời mời.")}
                }
                DispatchQueue.main.async{
                    guard owner.active,DispatchTime.now()<deadline,self.pairingClientDeliveryFence.admits(token) else{completion(.failure(OmniFailure.message("Kết quả host đã cũ. Hãy đọc lại trạng thái.")));return}
                    completion(.success(value))
                }
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    func pairingClient(_ action:NativeClientPairingAction,input:inout Data,owner:NativeDocumentLifetime,admission:@escaping ()->Bool={true},
        completion:@escaping @MainActor (Result<[String:Any],Error>)->Void){
        let owned=NativePairingInput(taking:&input)
        let fail:(String)->Void={message in
            owned.clear();DispatchQueue.main.async{completion(.failure(OmniFailure.message(message)))}
        }
        let maximum=action == .begin ? 16_384:((action == .activate || action == .reviewSaved) ? 1024:(action == .persistAfterPeerConsent ? 256:128))
        guard !owned.bytes.isEmpty,owned.bytes.count<=maximum else{fail("Yêu cầu ghép đôi vượt giới hạn.");return}
        let deliveryToken=pairingClientDeliveryFence.capture(cancel:action == .cancel)
        if action == .cancel{
            let current=handle
            guard current != 0,om_mobile_pairing_invalidate_epoch(current)==1 else{fail("Chưa xác nhận được thao tác hủy. Hãy mở lại trạng thái.");return}
        }
        let reservesOperation=action != .cancel
        if reservesOperation && pairingClientPermit.wait(timeout:.now()) != .success{fail("Thao tác ghép đôi trước vẫn đang kết thúc.");return}
        guard pairingClientQueuePermit.wait(timeout:.now()) == .success else{
            if reservesOperation{pairingClientPermit.signal()};fail("Hàng đợi ghép đôi đang bận.");return
        }
        let deadline=DispatchTime.now()+25
        queue.async{
            defer{owned.clear();self.pairingClientQueuePermit.signal();if reservesOperation{self.pairingClientPermit.signal()}}
            do{
                var expectedTicket:String?
                var reviewedHash:String?
                var activation:[String:Any]?
                if action == .activate || action == .reviewSaved{
                    guard let request=try JSONSerialization.jsonObject(with:owned.bytes) as? [String:Any],request.count==(action == .activate ? 3:2),
                          let address=request["address"] as? [String:Any],address.count==5,address["profile"] as? String==self.profileID.uuidString.lowercased(),address["kind"] as? String=="pair_encryption",
                          let revision=request["expected_revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),revision.doubleValue>=1,revision.doubleValue<10_000_000,revision.doubleValue==Double(revision.uint64Value) else{throw OmniFailure.message("Yêu cầu kích hoạt không hợp lệ.")}
                    if action == .activate{
                        guard let hash=request["reviewed_grant_hash"] as? String,NativePairingText.fingerprint(hash) else{throw OmniFailure.message("Thiếu grant đã duyệt.")}
                    }
                    for field in ["device","id","generation"]{
                        guard let text=address[field] as? String,let id=UUID(uuidString:text),id.uuidString.lowercased()==text,
                              id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else{throw OmniFailure.message("Địa chỉ kích hoạt không hợp lệ.")}
                    }
                    activation=request
                }else if action != .begin{
                    guard let request=try JSONSerialization.jsonObject(with:owned.bytes) as? [String:Any],request.count==(action == .persistAfterPeerConsent ? 2:1),
                          let ticket=request["ticket"] as? String,let id=UUID(uuidString:ticket),id.uuidString.lowercased()==ticket,
                          id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else{throw OmniFailure.message("Ticket ghép đôi không hợp lệ.")}
                    expectedTicket=ticket
                    if action == .persistAfterPeerConsent{
                        guard let hash=request["reviewed_grant_hash"] as? String,NativePairingText.fingerprint(hash) else{throw OmniFailure.message("Thiếu grant đã được người dùng duyệt.")}
                        reviewedHash=hash
                    }
                }
                let now=DispatchTime.now()
                guard self.handle != 0,now<deadline,(action == .cancel || (admission()&&self.pairingClientDeliveryFence.admits(deliveryToken)&&owner.active&&self.pairingAttached)) else{throw OmniFailure.message("Phiên ghép đôi đã kết thúc. Hãy mở lại trạng thái.")}
                let remaining=(deadline.uptimeNanoseconds-now.uptimeNanoseconds)/1_000_000
                guard remaining>=1000 else{throw OmniFailure.message("Yêu cầu ghép đôi đã hết hạn trong hàng đợi.")}
                let budget=UInt32(min(25_000,remaining))
                let value=try self.unwrap(owned.bytes.withUnsafeBytes{raw in
                    let pointer=raw.bindMemory(to:UInt8.self).baseAddress
                    if action == .activate{return try self.consume(om_mobile_pairing_activate_client_v1(self.handle,pointer,raw.count,budget))}
                    if action == .reviewSaved{return try self.consume(om_mobile_pairing_review_client_v1(self.handle,pointer,raw.count,budget))}
                    return try self.consume(action == .begin
                        ? om_mobile_pairing_client_begin_v1(self.handle,pointer,raw.count,budget)
                        : om_mobile_pairing_client_action_v1(self.handle,pointer,raw.count,action.rawValue,budget))
                })
                if action == .reviewSaved{
                    guard let request=activation,let expected=request["address"] as? [String:Any],let address=value["address"] as? [String:Any],address.count==5,
                          NSDictionary(dictionary:expected).isEqual(to:address),value["pair_id"] as? String==expected["id"] as? String,
                          (value["credential_protocol"] as? NSNumber)?.uint32Value==1,value["phase"] as? String=="material_ready",
                          let previous=request["expected_revision"] as? NSNumber,let revision=value["revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),revision.doubleValue==previous.doubleValue,
                          let hash=value["grant_hash"] as? String,NativePairingText.fingerprint(hash),
                          let host=value["host_public_key"] as? String,NativePairingText.publicKey(host),
                          let fingerprint=value["host_fingerprint"] as? String,NativePairingText.fingerprint(fingerprint),
                          let consent=value["local_consent_required"] as? NSNumber,CFGetTypeID(consent)==CFBooleanGetTypeID(),consent.boolValue,
                          let comparison=value["comparison_available"] as? NSNumber,CFGetTypeID(comparison)==CFBooleanGetTypeID(),!comparison.boolValue,
                          let permission=value["input_allowed"] as? NSNumber,CFGetTypeID(permission)==CFBooleanGetTypeID(),
                          let expiry=value["expires_at_ms"] as? NSNumber,CFGetTypeID(expiry) != CFBooleanGetTypeID(),expiry.doubleValue>Date().timeIntervalSince1970*1000,
                          expiry.doubleValue<=9_007_199_254_740_991,expiry.doubleValue==Double(expiry.uint64Value) else{throw OmniFailure.message("Kết quả đọc lại không khớp bản ghép đôi đã lưu.")}
                }else if action == .activate{
                    guard let request=activation,let expected=request["address"] as? [String:Any],let address=value["address"] as? [String:Any],address.count==5,
                          NSDictionary(dictionary:expected).isEqual(to:address),value["grant_hash"] as? String==request["reviewed_grant_hash"] as? String,
                          (value["credential_protocol"] as? NSNumber)?.uint32Value==1,value["phase"] as? String=="active",
                          let reload=value["reload_required"] as? NSNumber,CFGetTypeID(reload)==CFBooleanGetTypeID(),reload.boolValue,
                          let previous=request["expected_revision"] as? NSNumber,let revision=value["revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),
                          revision.doubleValue>previous.doubleValue,revision.doubleValue<=10_000_000,revision.doubleValue==Double(revision.uint64Value) else{throw OmniFailure.message("Kết quả kích hoạt không khớp candidate đã duyệt. Hãy đọc lại trạng thái.")}
                }else if action == .persistAfterPeerConsent{
                    guard let ticket=value["pairing_ticket"] as? String,ticket==expectedTicket else{throw OmniFailure.message("Candidate không khớp ticket ghép đôi.")}
                    guard let hash=value["grant_hash"] as? String,hash==reviewedHash else{throw OmniFailure.message("Grant đã lưu không khớp phiên duyệt.")}
                    guard (value["credential_protocol"] as? NSNumber)?.uint32Value==1,value["phase"] as? String=="material_ready",
                          let address=value["address"] as? [String:Any],address["profile"] as? String==self.profileID.uuidString.lowercased(),
                          address["kind"] as? String=="pair_encryption" else{throw OmniFailure.message("Kết quả candidate không hợp lệ. Hãy đọc lại trạng thái.")}
                    guard let revision=value["revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),
                          revision.doubleValue>=1,revision.doubleValue<=10_000_000,revision.doubleValue==Double(revision.uint64Value),
                          let key=value["public_key"] as? String,NativePairingText.publicKey(key) else{throw OmniFailure.message("Metadata candidate không hợp lệ.")}
                    for field in ["device","id","generation"]{
                        guard let text=address[field] as? String,let id=UUID(uuidString:text),id.uuidString.lowercased()==text,
                              id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else{throw OmniFailure.message("Địa chỉ candidate không hợp lệ.")}
                    }
                }else{
                    guard (value["pairing_protocol"] as? NSNumber)?.uint32Value==1,let ticket=value["ticket"] as? String,
                          let id=UUID(uuidString:ticket),id.uuidString.lowercased()==ticket,
                          id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)),expectedTicket==nil || expectedTicket==ticket else{throw OmniFailure.message("Kết quả không khớp ticket ghép đôi.")}
                }
                guard DispatchTime.now()<deadline,(action == .cancel || (admission()&&self.pairingClientDeliveryFence.admits(deliveryToken)&&owner.active)) else{throw OmniFailure.message("Kết quả ghép đôi đã cũ. Hãy mở lại trạng thái trước khi tiếp tục.")}
                func exactBool(_ object:Any?,_ expected:Bool)->Bool{
                    guard let number=object as? NSNumber,CFGetTypeID(number)==CFBooleanGetTypeID() else{return false}
                    return number.boolValue==expected
                }
                if action == .join{
                    guard value["state"] as? String == "pending",exactBool(value["approves_peer"],false) else{throw OmniFailure.message("Kết quả gửi yêu cầu không hợp lệ.")}
                }
                if action == .begin{
                    guard let comparison=value["comparison_code"] as? String,NativePairingText.comparison(comparison),
                          let host=value["host_public_key"] as? String,NativePairingText.publicKey(host),
                          let fingerprint=value["host_fingerprint"] as? String,NativePairingText.fingerprint(fingerprint),
                          let expiry=value["expires_at_ms"] as? NSNumber,CFGetTypeID(expiry) != CFBooleanGetTypeID(),
                          expiry.doubleValue>Date().timeIntervalSince1970*1000,expiry.doubleValue<=9_007_199_254_740_991,
                          expiry.doubleValue==Double(expiry.uint64Value),
                          let revision=value["revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),
                          revision.doubleValue>=1,revision.doubleValue<=10_000_000,revision.doubleValue==Double(revision.uint64Value) else{throw OmniFailure.message("Thông tin bắt đầu ghép đôi không hợp lệ.")}
                }
                if action == .poll{
                    guard let host=value["host_public_key"] as? String,NativePairingText.publicKey(host),
                          let fingerprint=value["host_fingerprint"] as? String,NativePairingText.fingerprint(fingerprint),
                          let comparison=value["comparison_code"] as? String,NativePairingText.comparison(comparison) else{throw OmniFailure.message("Thiếu thông tin host và mã đối chiếu.")}
                    guard exactBool(value["local_consent_required"],true),let status=value["status"] as? [String:Any],let state=status["state"] as? String else{throw OmniFailure.message("Trạng thái ghép đôi không hợp lệ.")}
                    if state == "approved"{
                        guard let hash=value["grant_hash"] as? String,NativePairingText.fingerprint(hash) else{throw OmniFailure.message("Thiếu mã ràng buộc grant được host duyệt.")}
                        guard let text=status["pair_id"] as? String,let id=UUID(uuidString:text),id.uuidString.lowercased()==text,
                              id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)),
                              exactBool(status["input_allowed"],true)||exactBool(status["input_allowed"],false),
                              let expiry=status["expires_at_ms"] as? NSNumber,CFGetTypeID(expiry) != CFBooleanGetTypeID(),
                              expiry.doubleValue>Date().timeIntervalSince1970*1000,expiry.doubleValue<=9_007_199_254_740_991,
                              expiry.doubleValue==Double(expiry.uint64Value) else{throw OmniFailure.message("Thông tin host duyệt không hợp lệ hoặc đã hết hạn.")}
                    }else if state != "pending" && state != "declined"{throw OmniFailure.message("Trạng thái ghép đôi không được hỗ trợ.")}
                }
                if action == .cancel{
                    guard exactBool(value["cancelled"],true),exactBool(value["remote_revoked"],false) else{throw OmniFailure.message("Kết quả hủy local không hợp lệ.")}
                }
                DispatchQueue.main.async{
                    guard DispatchTime.now()<deadline,(action == .cancel || (admission()&&self.pairingClientDeliveryFence.admits(deliveryToken)&&owner.active)) else{completion(.failure(OmniFailure.message("Kết quả ghép đôi đã cũ. Hãy mở lại trạng thái.")));return}
                    completion(.success(value))
                }
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    @MainActor func clientPairingSetup(owner:NativeDocumentLifetime,completion:@escaping @MainActor (Result<NativeClientSetup,Error>)->Void){
        let token=pairingClientDeliveryFence.capture(cancel:false),deadline=DispatchTime.now()+30
        pairingRecoveryStatus(owner:owner){result in
            do{
                let snapshot=try result.get(),identities=snapshot.records.filter{$0.isIdentity&&$0.phase != "retired"}
                guard owner.active,self.pairingClientDeliveryFence.admits(token),DispatchTime.now()<deadline,
                      !snapshot.namespaceProvenanceMissing,snapshot.pendingClientTicket==nil,snapshot.pendingHostTicket==nil,!snapshot.pairingOrigins.isEmpty,
                      identities.count==1,let identity=identities.first,identity.phase=="active",!identity.sessionRecoveryRequired else{throw OmniFailure.message("Chưa thể ghép đôi. Kiểm tra danh tính Active, phiên chờ và dịch vụ được cấu hình.")}
                let setup=NativeClientSetup(core:self,identity:identity,revision:snapshot.revision,origins:snapshot.pairingOrigins,
                    expires:min(deadline,snapshot.expires),current:{[weak self] in owner.active&&self?.pairingClientDeliveryFence.admits(token)==true})
                completion(.success(setup))
            }catch{completion(.failure(error))}
        }
    }
    @MainActor func beginPairingHost(_ setup:NativeClientSetup,audience:String,label:String,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<[String:Any],Error>)->Void){
        guard owner.active,setup.origin==ObjectIdentifier(self),setup.relayOrigins.contains(audience),!label.isEmpty,label.utf8.count<=120,
              !label.unicodeScalars.contains(where:{CharacterSet.controlCharacters.contains($0)}),setup.consume() else{completion(.failure(OmniFailure.message("Phiên chuẩn bị host đã kết thúc. Hãy mở lại.")));return}
        guard pairingClientPermit.wait(timeout:.now()) == .success else{completion(.failure(OmniFailure.message("Thao tác ghép đôi trước đang kết thúc.")));return}
        guard pairingClientQueuePermit.wait(timeout:.now()) == .success else{pairingClientPermit.signal();completion(.failure(OmniFailure.message("Hàng đợi host đang bận.")));return}
        let deadline=min(setup.expires,DispatchTime.now()+25)
        queue.async{
            defer{self.pairingClientPermit.signal();self.pairingClientQueuePermit.signal()}
            do{
                let now=DispatchTime.now()
                guard now<deadline,self.handle != 0,self.pairingAttached,owner.active,setup.admitted else{throw OmniFailure.message("Phiên chuẩn bị host đã hết hạn.")}
                let remaining=(deadline.uptimeNanoseconds-now.uptimeNanoseconds)/1_000_000
                guard remaining>=1000 else{throw OmniFailure.message("Yêu cầu host hết hạn trong hàng đợi.")}
                var input=try JSONSerialization.data(withJSONObject:["identity":setup.identity,"expected_revision":setup.revision,"audience":audience,"label":label]);defer{input.resetBytes(in:0..<input.count)}
                guard input.count<=1024 else{throw OmniFailure.message("Yêu cầu host vượt giới hạn.")}
                let value=try self.unwrap(input.withUnsafeBytes{raw in try self.consume(om_mobile_pairing_host_begin_v1(self.handle,raw.bindMemory(to:UInt8.self).baseAddress,raw.count,UInt32(min(25_000,remaining))))})
                guard (value["pairing_protocol"] as? NSNumber)?.uint32Value==1,value["host_public_key"] as? String==setup.identityKey,
                      let revision=value["revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),revision.doubleValue==Double(setup.revision),
                      let registered=value["registered"] as? NSNumber,CFGetTypeID(registered)==CFBooleanGetTypeID(),!registered.boolValue,
                      let share=value["share_available"] as? NSNumber,CFGetTypeID(share)==CFBooleanGetTypeID(),!share.boolValue,
                      let expiry=value["expires_at_ms"] as? NSNumber,CFGetTypeID(expiry) != CFBooleanGetTypeID(),expiry.doubleValue>Date().timeIntervalSince1970*1000,
                      expiry.doubleValue<=9_007_199_254_740_991,expiry.doubleValue==Double(expiry.uint64Value) else{throw OmniFailure.message("Phản hồi chuẩn bị host không hợp lệ.")}
                for field in ["ticket","invite_id"]{
                    guard let text=value[field] as? String,let id=UUID(uuidString:text),id.uuidString.lowercased()==text,id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else{throw OmniFailure.message("Ticket lời mời host không hợp lệ.")}
                }
                DispatchQueue.main.async{
                    guard owner.active,setup.admitted,DispatchTime.now()<deadline else{completion(.failure(OmniFailure.message("Kết quả host đã cũ. Hãy đọc lại trạng thái.")));return}
                    completion(.success(value))
                }
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    /// UI independently pins host and clears its invitation field after call.
    /// Immutable Swift/UIKit/JSON string aliases are outside wipe guarantees.
    @MainActor func beginPairingClient(_ setup:NativeClientSetup,code:String,host:String,audience:String,label:String,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<[String:Any],Error>)->Void){
        guard owner.active,setup.origin==ObjectIdentifier(self),setup.relayOrigins.contains(audience),!code.isEmpty,code.utf8.count<=8192,setup.consume() else{completion(.failure(OmniFailure.message("Phiên chuẩn bị đã kết thúc. Hãy mở lại ghép đôi.")));return}
        do{
            var input=try JSONSerialization.data(withJSONObject:["identity":setup.identity,"expected_revision":setup.revision,"code":code,"host_public_key":host,"audience":audience,"label":label])
            pairingClient(.begin,input:&input,owner:owner,admission:{setup.admitted}){result in
                guard setup.admitted else{completion(.failure(OmniFailure.message("Kết quả chuẩn bị đã cũ. Hãy kiểm tra trạng thái native.")));return}
                completion(result)
            }
        }catch{completion(.failure(error))}
    }
    /// Poll once; only a validated approved response yields a consent snapshot.
    @MainActor func reviewPairingPeer(ticket:String,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<NativePeerApproval,Error>)->Void){
        let token=pairingClientDeliveryFence.capture(cancel:false)
        do{
            var input=try JSONSerialization.data(withJSONObject:["ticket":ticket])
            pairingClient(.poll,input:&input,owner:owner){result in
                do{
                    let value=try result.get()
                    guard owner.active,self.pairingClientDeliveryFence.admits(token),let status=value["status"] as? [String:Any],
                          status["state"] as? String == "approved",let pair=status["pair_id"] as? String,
                          let allowed=status["input_allowed"] as? Bool,let hash=value["grant_hash"] as? String,
                          let host=value["host_public_key"] as? String,let fingerprint=value["host_fingerprint"] as? String,let comparison=value["comparison_code"] as? String,
                          let expiry=status["expires_at_ms"] as? NSNumber else{throw OmniFailure.message("Chưa có grant hiện tại để xác nhận.")}
                    let remaining=expiry.doubleValue-Date().timeIntervalSince1970*1000
                    guard remaining>0 else{throw OmniFailure.message("Grant đã hết hạn. Hãy đọc lại trạng thái.")}
                    let review=NativePeerApproval(origin:self,ticket:ticket,hash:hash,pair:pair,host:host,fingerprint:fingerprint,comparison:comparison,input:allowed,
                        grantExpiresAt:Date(timeIntervalSince1970:expiry.doubleValue/1000),
                        expires:DispatchTime.now() + .milliseconds(Int(min(60_000,remaining))),current:{[weak self] in
                            owner.active && self?.pairingClientDeliveryFence.admits(token)==true
                        })
                    completion(.success(review))
                }catch{completion(.failure(error))}
            }
        }catch{completion(.failure(error))}
    }
    /// Actual native peer/comparison/input consent must precede this call.
    @MainActor func confirmPairingPeer(_ review:NativePeerApproval,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<[String:Any],Error>)->Void){
        guard owner.active,review.origin==ObjectIdentifier(self),review.consume() else{completion(.failure(OmniFailure.message("Phiên xác nhận đã kết thúc. Hãy duyệt lại grant hiện tại.")));return}
        do{
            var input=try JSONSerialization.data(withJSONObject:["ticket":review.ticket,"reviewed_grant_hash":review.grantHash])
            pairingClient(.persistAfterPeerConsent,input:&input,owner:owner,admission:{review.admitted}){result in
                guard review.admitted else{completion(.failure(OmniFailure.message("Kết quả xác nhận đã cũ. Hãy kiểm tra trạng thái native.")));return}
                completion(result)
            }
        }catch{completion(.failure(error))}
    }
    @MainActor func reviewSavedPeer(_ record:NativeRecoveryRecord,snapshot:NativeRecoverySnapshot,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<NativeSavedPeerReview,Error>)->Void){
        let token=pairingClientDeliveryFence.capture(cancel:false)
        guard owner.active,DispatchTime.now()<snapshot.expires,!snapshot.namespaceProvenanceMissing,!record.isIdentity,record.phase=="material_ready",!record.sessionRecoveryRequired,
              snapshot.records.contains(where:{NSDictionary(dictionary:$0.address).isEqual(to:record.address)&&$0.phase==record.phase&&$0.publicKey==record.publicKey}) else{completion(.failure(OmniFailure.message("Hãy đọc lại trạng thái trước khi duyệt ghép đôi đã lưu.")));return}
        do{
            var input=try JSONSerialization.data(withJSONObject:["address":record.address,"expected_revision":snapshot.revision])
            pairingClient(.reviewSaved,input:&input,owner:owner,admission:{DispatchTime.now()<snapshot.expires}){result in
                do{
                    let value=try result.get()
                    guard owner.active,self.pairingClientDeliveryFence.admits(token),DispatchTime.now()<snapshot.expires,
                          let expiry=value["expires_at_ms"] as? NSNumber else{throw OmniFailure.message("Phiên duyệt đã kết thúc.")}
                    let remaining=min(60_000,expiry.doubleValue-Date().timeIntervalSince1970*1000)
                    guard remaining>0 else{throw OmniFailure.message("Quyền ghép đôi đã hết hạn.")}
                    let review=try NativeSavedPeerReview(core:self,value:value,expires:.now() + .milliseconds(Int(remaining)),current:{[weak self] in owner.active&&self?.pairingClientDeliveryFence.admits(token)==true})
                    completion(.success(review))
                }catch{completion(.failure(error))}
            }
        }catch{completion(.failure(error))}
    }
    /// Fresh independent host/permission consent required; no old code exists.
    @MainActor func activateSavedPeer(_ review:NativeSavedPeerReview,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<[String:Any],Error>)->Void){
        guard owner.active,review.core==ObjectIdentifier(self),review.consume() else{completion(.failure(OmniFailure.message("Phiên duyệt đã kết thúc. Hãy duyệt lại ghép đôi.")));return}
        do{
            var input=try JSONSerialization.data(withJSONObject:["address":review.address,"expected_revision":review.revision,"reviewed_grant_hash":review.hash])
            pairingClient(.activate,input:&input,owner:owner,admission:{review.admitted},completion:completion)
        }catch{completion(.failure(error))}
    }
    @MainActor func activationForPeer(_ review:NativePeerApproval,value:[String:Any]) throws->NativePeerActivation{
        guard review.origin==ObjectIdentifier(self),review.admitted,
              value["grant_hash"] as? String==review.grantHash,
              (value["credential_protocol"] as? NSNumber)?.uint32Value==1,value["phase"] as? String=="material_ready",value["pairing_ticket"] as? String==review.ticket,
              let address=value["address"] as? [String:Any],address.count==5,address["profile"] as? String==profileID.uuidString.lowercased(),address["kind"] as? String=="pair_encryption",address["id"] as? String==review.pairID,
              let generation=address["generation"] as? String,let revision=value["revision"] as? NSNumber,CFGetTypeID(revision) != CFBooleanGetTypeID(),revision.doubleValue>=1,revision.doubleValue<10_000_000,revision.doubleValue==Double(revision.uint64Value) else{throw OmniFailure.message("Candidate đã lưu không khớp phiên duyệt. Hãy kiểm tra trạng thái.")}
        for field in ["device","generation"]{
            guard let text=address[field] as? String,let id=UUID(uuidString:text),id.uuidString.lowercased()==text,
                  id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else{throw OmniFailure.message("Địa chỉ candidate không hợp lệ.")}
        }
        guard review.issueActivation() else{throw OmniFailure.message("Phiên kích hoạt đã được sử dụng hoặc kết thúc.")}
        return NativePeerActivation(peer:review,address:address,revision:revision.uint64Value,generation:generation)
    }
    @MainActor func activatePairingPeer(_ activation:NativePeerActivation,owner:NativeDocumentLifetime,
        completion:@escaping @MainActor (Result<[String:Any],Error>)->Void){
        guard owner.active,activation.peer.origin==ObjectIdentifier(self),activation.consume() else{completion(.failure(OmniFailure.message("Phiên kích hoạt đã kết thúc. Hãy kiểm tra trạng thái.")));return}
        do{
            var input=try JSONSerialization.data(withJSONObject:["address":activation.address,"expected_revision":activation.revision,"reviewed_grant_hash":activation.peer.grantHash])
            pairingClient(.activate,input:&input,owner:owner,admission:{activation.peer.admitted},completion:completion)
        }catch{completion(.failure(error))}
    }
    /// Read-only native recovery inspection remains available with policy off.
    func pairingRecoveryStatus(owner:NativeDocumentLifetime,completion:@escaping @MainActor (Result<NativeRecoverySnapshot,Error>)->Void){
        queue.async{
            do{
                guard owner.active,self.handle != 0,self.pairingAttached else{throw OmniFailure.message("Trạng thái Remote Play chưa sẵn sàng.")}
                let expires=DispatchTime.now()+30
                let value=try self.unwrap(self.consume(om_mobile_pairing_status(self.handle)))
                guard owner.active,DispatchTime.now()<expires,(value["credential_protocol"] as? NSNumber)?.uint32Value==1,
                      (value["recovery_protocol"] as? NSNumber)?.uint32Value==1,value["profile_id"] as? String == self.profileID.uuidString.lowercased(),
                      let device=value["device_id"] as? String,UUID(uuidString:device)?.uuidString.lowercased()==device,
                      let revision=value["revision"] as? NSNumber,revision.doubleValue>=0,revision.doubleValue<=10_000_000,revision.doubleValue==Double(revision.uint64Value),
                      let records=value["records"] as? [[String:Any]],records.count<=128,
                      let actions=value["recovery"] as? [[String:Any]],actions.count<=128,
                      let relayAvailable=value["relay_retirement_available"] as? Bool,
                      let namespaceMissing=value["namespace_provenance_missing"] as? Bool else{throw OmniFailure.message("Trạng thái phục hồi không hợp lệ.")}
                let steps:Set<String>=["fence_generation","revalidate_before_use","retire_writer","retire_local_sessions","wait_for_pair_cleanup","revoke_relay","remove_material","finalize_retirement"]
                var next:[String:([String:Any],String)]=[:]
                for action in actions{
                    guard let address=action["address"] as? [String:Any],let generation=address["generation"] as? String,
                          let step=action["next_step"] as? String,steps.contains(step),next[generation]==nil else{throw OmniFailure.message("Kế hoạch phục hồi không hợp lệ.")}
                    next[generation]=(address,step)
                }
                var parsed:[NativeRecoveryRecord]=[];var seen=Set<String>()
                for record in records{
                    guard let address=record["address"] as? [String:Any],address["profile"] as? String == self.profileID.uuidString.lowercased(),address["device"] as? String == device,
                          let generation=address["generation"] as? String,UUID(uuidString:generation)?.uuidString.lowercased()==generation,seen.insert(generation).inserted,
                          let item=address["id"] as? String,UUID(uuidString:item)?.uuidString.lowercased()==item,
                          let kind=address["kind"] as? String,["device_signing","pair_encryption"].contains(kind),(kind != "device_signing" || item==device),
                          let phase=record["phase"] as? String,["reserved","writing","material_ready","active","revoking","retired"].contains(phase),
                          let key=record["public_key"] as? String,key.count==43,
                          let sessionRecoveryRequired=record["session_recovery_required"] as? Bool else{throw OmniFailure.message("Bản ghi phục hồi không hợp lệ.")}
                    let action=next.removeValue(forKey:generation)
                    if let action=action{
                        guard phase != "retired",NSDictionary(dictionary:action.0).isEqual(to:address) else{throw OmniFailure.message("Bản ghi và kế hoạch phục hồi không khớp.")}
                    }else if phase != "retired"{throw OmniFailure.message("Thiếu kế hoạch phục hồi cho bản ghi hiện tại.")}
                    parsed.append(NativeRecoveryRecord(address:address,phase:phase,publicKey:key,nextStep:action?.1,sessionRecoveryRequired:sessionRecoveryRequired))
                }
                guard next.isEmpty,owner.active,DispatchTime.now()<expires else{throw OmniFailure.message("Trạng thái phục hồi đã hết hạn.")}
                var pendingTicket:String?
                if let pending=value["pending_client"] as? [String:Any]{
                    guard let ticket=pending["ticket"] as? String,let id=UUID(uuidString:ticket),id.uuidString.lowercased()==ticket,
                          id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)),let expired=pending["expired"] as? Bool,
                          let expiry=pending["expires_at_ms"] as? NSNumber else{throw OmniFailure.message("Phiên ghép đôi chờ không hợp lệ.")}
                    if !expired && expiry.doubleValue>Date().timeIntervalSince1970*1000{pendingTicket=ticket}
                }
                guard let origins=value["pairing_origins"] as? [String],origins.count<=8,Set(origins).count==origins.count else{throw OmniFailure.message("Cấu hình relay không hợp lệ.")}
                for origin in origins{
                    guard let url=URLComponents(string:origin),url.scheme=="https",let host=url.host,!host.isEmpty,url.user==nil,url.password==nil,url.query==nil,url.fragment==nil,url.path.isEmpty else{throw OmniFailure.message("Relay nằm ngoài cấu hình HTTPS hợp lệ.")}
                }
                var hostTicket:String?
                if let pending=value["pending_host"] as? [String:Any]{
                    guard let ticket=pending["ticket"] as? String,let id=UUID(uuidString:ticket),id.uuidString.lowercased()==ticket,
                          id != UUID(uuid:(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)) else{throw OmniFailure.message("Ticket lời mời host không hợp lệ.")}
                    for field in ["expired","locally_closed","remote_close_required"]{
                        guard let flag=pending[field] as? NSNumber,CFGetTypeID(flag)==CFBooleanGetTypeID() else{throw OmniFailure.message("Trạng thái lời mời host không hợp lệ.")}
                    }
                    hostTicket=ticket
                }
                let snapshot=NativeRecoverySnapshot(revision:revision.uint64Value,device:device,records:parsed,expires:expires,relayRetirementAvailable:relayAvailable,namespaceProvenanceMissing:namespaceMissing,pendingClientTicket:pendingTicket,pendingHostTicket:hostTicket,pairingOrigins:origins)
                DispatchQueue.main.async{completion(.success(snapshot))}
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    /// Only after native approval of the exact record from this snapshot.
    func advancePairingRecovery(_ record:NativeRecoveryRecord,snapshot:NativeRecoverySnapshot,owner:NativeDocumentLifetime,completion:@escaping @MainActor (Result<String,Error>)->Void){
        queue.async{
            do{
                let started=DispatchTime.now()
                guard owner.active,started<snapshot.expires,self.handle != 0,self.pairingAttached,!snapshot.namespaceProvenanceMissing,
                      (record.phase=="active" || ["fence_generation","finalize_retirement","remove_material","revoke_relay"].contains(record.nextStep ?? "")),
                      (record.nextStep != "revoke_relay" || snapshot.relayRetirementAvailable),
                      snapshot.records.contains(where:{$0.generation==record.generation&&$0.phase==record.phase&&$0.nextStep==record.nextStep&&NSDictionary(dictionary:$0.address).isEqual(to:record.address)}) else{throw OmniFailure.message("Hãy mở lại trạng thái trước khi phục hồi.")}
                let remaining=(snapshot.expires.uptimeNanoseconds-started.uptimeNanoseconds)/1_000_000
                guard remaining>=1_000 else{throw OmniFailure.message("Trạng thái đã hết hạn. Hãy mở lại trước khi phục hồi.")}
                let budget=UInt32(min(25_000,remaining))
                let value=try self.unwrap(self.call(["address":record.address,"expected_revision":snapshot.revision]){data,length in
                    if record.phase=="active"{return om_mobile_pairing_revoke_active_v1(self.handle,data,length,budget)}
                    if record.nextStep=="remove_material"{return om_mobile_pairing_remove_material_v1(self.handle,data,length,budget)}
                    if record.nextStep=="revoke_relay"{return om_mobile_pairing_revoke_relay_v1(self.handle,data,length,budget)}
                    return om_mobile_pairing_advance_metadata_v1(self.handle,data,length,budget)
                })
                guard owner.active,(value["recovery_protocol"] as? NSNumber)?.uint32Value==1,value["reload_required"] as? Bool == true,
                      let address=value["address"] as? [String:Any],NSDictionary(dictionary:address).isEqual(to:record.address),
                      let result=value["result"] as? [String:Any],let outcome=result["outcome"] as? String,
                      ["committed","pending","revalidation_required","already_retired"].contains(outcome) else{throw OmniFailure.message("Kết quả chưa được xác nhận. Hãy đọc lại trạng thái.")}
                DispatchQueue.main.async{completion(.success(outcome))}
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
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
