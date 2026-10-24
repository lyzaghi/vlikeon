import UIKit
import BackgroundTasks

/// One trusted OS opportunity. Its stop flag survives a late profile-open/start
/// reply, and cancel/status never use NativeCore's serialized request queue.
final class NativeBackgroundRun {
    private let lock = NSLock()
    private let deadline = DispatchTime.now().uptimeNanoseconds + 25_000_000_000
    private var stopped = false
    private var handle: UInt64 = 0
    private var ticket: String?
    var active: Bool { lock.lock(); defer { lock.unlock() }; return !stopped }
    var remainingMilliseconds: UInt64 { let now = DispatchTime.now().uptimeNanoseconds; return now < deadline ? (deadline-now)/1_000_000 : 0 }
    func bind(handle: UInt64, ticket: String) {
        lock.lock(); self.handle = handle; self.ticket = ticket; let cancel = stopped; lock.unlock()
        if cancel { _ = try? status(cancel: true) }
    }
    func cancel() {
        lock.lock(); let send = !stopped; stopped = true; lock.unlock()
        if send { _ = try? status(cancel: true) }
    }
    func status(cancel: Bool = false) throws -> [String:Any] {
        lock.lock(); let owned = handle; let id = ticket; lock.unlock()
        guard owned != 0, let id = id else { throw OmniFailure.message("The background task has not started.") }
        let bytes = Array(id.utf8)
        guard bytes.count == 36 else { throw OmniFailure.message("The background task ticket is invalid.") }
        let buffer = bytes.withUnsafeBufferPointer { om_mobile_background_status(owned,$0.baseAddress,$0.count,cancel ? 1 : 0) }
        defer { om_mobile_free(buffer) }
        guard let pointer = buffer.data, buffer.length > 0, buffer.length <= 1_048_576,
              let response = try JSONSerialization.jsonObject(with: Data(bytes:pointer,count:buffer.length)) as? [String:Any],
              response["status"] as? String == "ok", let result = response["result"] as? [String:Any] else { throw OmniFailure.message("The background task is unavailable.") }
        return result
    }
}

@MainActor
final class BackgroundSync {
    static let shared = BackgroundSync()
    private static let identifier = "org.omniplay.browser.save-refresh"
    private var registered = false
    private var scheduled = false
    private var task: BGAppRefreshTask?
    private var run: NativeBackgroundRun?
    private var pollWork: DispatchWorkItem?
    private var expiry: DispatchWorkItem?
    private var observers: [NSObjectProtocol] = []

    func register() {
        SecureCredentials.setProtectedDataAvailable(UIApplication.shared.isProtectedDataAvailable)
        PairingCredentials.setProtectedDataAvailable(UIApplication.shared.isProtectedDataAvailable)
        observers.append(NotificationCenter.default.addObserver(forName:UIApplication.protectedDataWillBecomeUnavailableNotification,object:nil,queue:.main){ [weak self] _ in
            SecureCredentials.setProtectedDataAvailable(false)
            PairingCredentials.setProtectedDataAvailable(false)
            Task { @MainActor in if let task = self?.task { self?.finish(task,success:false) } }
        })
        observers.append(NotificationCenter.default.addObserver(forName:UIApplication.protectedDataDidBecomeAvailableNotification,object:nil,queue:.main){ _ in
            SecureCredentials.setProtectedDataAvailable(true)
            PairingCredentials.setProtectedDataAvailable(true)
        })
        observers.append(NotificationCenter.default.addObserver(forName:UIApplication.willEnterForegroundNotification,object:nil,queue:.main){ [weak self] _ in
            Task { @MainActor in if let task = self?.task { self?.finish(task,success:false) } }
        })
        registered = BGTaskScheduler.shared.register(forTaskWithIdentifier: Self.identifier, using: .main) { [weak self] task in
            Task { @MainActor in
                guard let self = self, let task = task as? BGAppRefreshTask else { task.setTaskCompleted(success:false); return }
                self.start(task)
            }
        }
        refresh()
    }
    func refresh() {
        do {
            guard let core = try NativeCore.primary(create:false) else { BGTaskScheduler.shared.cancel(taskRequestWithIdentifier:Self.identifier); return }
            core.backgroundPolicy { [weak self] result in if case .success(let policy) = result { _ = self?.apply(policy) } }
        } catch { /* A local identity/storage fault never creates another profile. */ }
    }
    func apply(_ policy: [String:Any]) -> [String:Any] {
        var value = policy
        guard policy["eligible"] as? Bool == true else {
            BGTaskScheduler.shared.cancel(taskRequestWithIdentifier:Self.identifier); scheduled = false
            value["scheduled"] = false; value["scheduler_state"] = "disabled"; return value
        }
        var state = "unavailable"
        if registered {
            if !scheduled {
                let request = BGAppRefreshTaskRequest(identifier:Self.identifier)
                request.earliestBeginDate = Date(timeIntervalSinceNow:15*60)
                do { try BGTaskScheduler.shared.submit(request); scheduled = true } catch { scheduled = false }
            }
            state = UIApplication.shared.backgroundRefreshStatus == .available ? (scheduled ? "scheduled" : "unavailable") : "restricted"
        }
        value["scheduled"] = scheduled; value["scheduler_state"] = state; return value
    }
    private func start(_ task: BGAppRefreshTask) {
        guard UIApplication.shared.applicationState != .active, UIApplication.shared.isProtectedDataAvailable else { scheduled = false; task.setTaskCompleted(success:false); refresh(); return }
        guard self.task == nil else { task.setTaskCompleted(success:false); return }
        scheduled = false
        let run = NativeBackgroundRun(); self.task = task; self.run = run
        // Expiration can run away from the main queue. Signal native cancellation
        // immediately; the main queue owns exactly-once OS task completion.
        task.expirationHandler = { [weak self,weak task] in
            run.cancel(); DispatchQueue.main.async { if let task = task { self?.finish(task, success:false) } }
        }
        let expiry = DispatchWorkItem { [weak self,weak task] in if let task = task { self?.finish(task, success:false) } }
        self.expiry = expiry; DispatchQueue.main.asyncAfter(deadline:.now()+25,execute:expiry)
        do {
            guard let core = try NativeCore.primary(create:false) else { finish(task,success:false); return }
            // Submit the next opportunity without starting a second native job.
            refresh()
            core.beginBackground(run) { [weak self,weak task] result in
                guard let self = self, let task = task, self.task === task else { return }
                switch result { case .success: self.poll(task); case .failure: self.finish(task,success:false) }
            }
        } catch { finish(task,success:false) }
    }
    private func poll(_ task: BGAppRefreshTask) {
        guard self.task === task, let run = run else { return }
        do {
            let state = try run.status()["state"] as? String
            if state != "running" { finish(task,success:state == "complete"); return }
        } catch { finish(task,success:false); return }
        let item = DispatchWorkItem { [weak self,weak task] in if let task = task { self?.poll(task) } }
        pollWork = item; DispatchQueue.main.asyncAfter(deadline:.now()+0.5,execute:item)
    }
    private func finish(_ task: BGAppRefreshTask, success:Bool) {
        guard self.task === task else { return }
        run?.cancel(); pollWork?.cancel(); expiry?.cancel(); task.expirationHandler = nil
        run = nil; self.task = nil; pollWork = nil; expiry = nil
        task.setTaskCompleted(success:success)
    }
}
