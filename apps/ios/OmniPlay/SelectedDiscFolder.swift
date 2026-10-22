import Foundation
import Darwin

/// Only native picker URLs reach this object. Every producer owns its coordinator
/// through the complete read; no asynchronous task ever borrows a Rust pointer.
final class SelectedDiscFolder {
    private struct Failure: Error { let code: Int32; init(_ code: Int32) { self.code=code } }
    private struct Node { let directory: Bool; let size: Int64 }
    private final class Answer<T> {
        let ready=DispatchSemaphore(value:0)
        private let lock=NSLock()
        private var stored: Result<T, Error>?
        func finish(_ value: Result<T, Error>) { lock.lock();stored=value;lock.unlock();ready.signal() }
        func result() throws -> T { lock.lock();defer{lock.unlock()};guard let stored=stored else{throw Failure(-5)};return try stored.get() }
    }
    private final class Stream {
        let token: UInt64
        let path: String
        // Accessed only under the provider's state lock.
        var started=false
        private let condition=NSCondition()
        private var block: Data?
        private var offset=0
        private var terminal: Int32?
        init(token: UInt64, path: String) { self.token=token;self.path=path }
        func stop(_ code: Int32) { condition.lock();terminal=code;block=nil;offset=0;condition.broadcast();condition.unlock() }
        func check() throws { condition.lock();defer{condition.unlock()};if let terminal=terminal,terminal<0{throw Failure(terminal)} }
        func finish(_ code: Int32) { condition.lock();if terminal==nil{terminal=code};condition.broadcast();condition.unlock() }
        /// One block is held until consumed, with no unbounded producer queue.
        func publish(_ value: Data) throws {
            condition.lock();defer{condition.unlock()}
            if let terminal=terminal { throw Failure(terminal<0 ? terminal : -2) }
            guard block==nil,!value.isEmpty,value.count<=65_536 else{throw Failure(-5)}
            block=value;offset=0;condition.broadcast()
            while block != nil && terminal==nil { condition.wait(until:Date(timeIntervalSinceNow:0.25)) }
            if let terminal=terminal,terminal<0{throw Failure(terminal)}
        }
        func take(_ output: UnsafeMutablePointer<UInt8>, capacity: Int, deadline: UInt64, owner: SelectedDiscFolder) -> Int64 {
            condition.lock()
            while block==nil && terminal==nil {
                if SelectedDiscFolder.now>=deadline {condition.unlock();owner.cancel(-4);return -4}
                condition.wait(until:Date(timeIntervalSinceNow:0.25))
            }
            if let terminal=terminal,terminal<0{condition.unlock();return Int64(terminal)}
            guard let data=block else{let code=terminal ?? -5;condition.unlock();return Int64(code)}
            if SelectedDiscFolder.now>=deadline {condition.unlock();owner.cancel(-4);return -4}
            let count=min(capacity,data.count-offset)
            guard count>0 else{condition.unlock();return -5}
            data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in output.update(from:raw.bindMemory(to:UInt8.self).baseAddress!.advanced(by:offset),count:count) }
            offset+=count;if offset==data.count{block=nil;offset=0;condition.broadcast()};condition.unlock();return Int64(count)
        }
    }
    private static let maximum: UInt64=4*1024*1024*1024
    private static let admission=NSLock()
    private static var contexts=0
    private static var now: UInt64 { DispatchTime.now().uptimeNanoseconds }
    private static func reserve() -> Bool { admission.lock();defer{admission.unlock()};guard contexts<4 else{return false};contexts+=1;return true }
    private static func retire() { admission.lock();contexts-=1;admission.unlock() }
    let id: UUID
    private let source: URL
    private let root: URL
    private let profile: [UInt8]
    private let io: DispatchQueue
    private let metadataLock=NSLock()
    private let state=NSLock()
    private var stopped: Int32=0
    private var prepared=false,started=false
    private var deadline: UInt64
    private var expiry: DispatchWorkItem?
    private var coordinator: NSFileCoordinator?
    private var producer: UInt64?
    private var streams: [UInt64: Stream]=[:]
    private var nextToken: UInt64=1
    // These resources and caches are only accessed on io, or in deinit after its
    // last operation releases self. Cancellation never closes a borrowed fd.
    private var rootFD: Int32 = -1
    private var rootIdentity: stat?
    private var directories: [String: [String: Node]]=[:]
    private var rows=0,metadataBytes=0

    init(profile: UUID, source: URL, requestDeadlineMilliseconds: Double) throws {
        guard source.isFileURL,(source.host ?? "").isEmpty||source.host=="localhost",source.query==nil,source.fragment==nil,source.path.hasPrefix("/"),source.path != "/",source.path.utf8.count<=4096,
              requestDeadlineMilliseconds.isFinite,requestDeadlineMilliseconds>Date().timeIntervalSince1970*1000 else{throw Failure(-3)}
        guard Self.reserve() else{throw Failure(-5)}
        guard source.startAccessingSecurityScopedResource() else{Self.retire();throw Failure(-3)}
        let identifier=UUID();id=identifier
        self.source=source;root=source.standardizedFileURL
        var identity=profile.uuid;self.profile=withUnsafeBytes(of:&identity){Array($0)}
        io=DispatchQueue(label:"com.omniplay.ios.disc."+identifier.uuidString,qos:.userInitiated)
        let remaining=min(120_000,max(1,requestDeadlineMilliseconds-Date().timeIntervalSince1970*1000))
        deadline=Self.now+UInt64(remaining*1_000_000)
        state.lock();armLocked(deadline);state.unlock()
    }
    deinit { expiry?.cancel();if rootFD>=0{Darwin.close(rootFD)};source.stopAccessingSecurityScopedResource();Self.retire() }

    func prepare() throws -> [String: Any] {
        metadataLock.lock();defer{metadataLock.unlock()}
        let value: [String:Any]=try perform {
            guard self.rootFD<0 else{throw Failure(-3)}
            try self.coordinate("",options:[.withoutChanges,.immediatelyAvailableMetadataOnly]) { readable in
                let values=try readable.resourceValues(forKeys:[.isDirectoryKey,.isSymbolicLinkKey,.isAliasFileKey])
                guard values.isDirectory==true,values.isSymbolicLink != true,values.isAliasFile != true else{throw Failure(-3)}
                let fd=Darwin.open(readable.path,O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC)
                guard fd>=0 else{throw Failure(-3)}
                var info=stat();guard fstat(fd,&info)==0 else{Darwin.close(fd);throw Failure(-5)}
                self.rootFD=fd;self.rootIdentity=info
            }
            let nodes=try self.children("");var entries:[String]=[]
            for(name,node)in nodes where !node.directory {
                let ext=(name as NSString).pathExtension.lowercased()
                if ["cue","m3u","chd","iso","pbp"].contains(ext){entries.append(name);if entries.count>64{throw Failure(-5)}}
            }
            guard !entries.isEmpty else{throw Failure(-1)}
            var title=self.root.lastPathComponent.unicodeScalars.filter{!CharacterSet.controlCharacters.contains($0)}.prefix(120).map{String($0)}.joined().trimmingCharacters(in:.whitespacesAndNewlines)
            if title.isEmpty{title="Game"}
            return ["entries":entries.sorted(),"suggested_title":title]
        }
        state.lock();defer{state.unlock()};try checkLocked();prepared=true;armLocked(Self.now+300_000_000_000);return value
    }
    private func open(_ boundProfile: UnsafePointer<UInt8>?, relative: UnsafePointer<UInt8>?, length: Int, result: UnsafeMutablePointer<OmniMediaStream>?) -> Int32 {
        guard let boundProfile=boundProfile,Array(UnsafeBufferPointer(start:boundProfile,count:16))==profile,
              let relative=relative,length>0,length<=2048,let result=result,
              let path=String(bytes:UnsafeBufferPointer(start:relative,count:length),encoding:.utf8),Self.components(path) != nil else{return -3}
        metadataLock.lock();defer{metadataLock.unlock()}
        do {
            state.lock()
            do{try checkLocked();guard prepared,streams.count<129 else{throw Failure(-3)};if !started{started=true;armLocked(Self.now+1_800_000_000_000)};state.unlock()}catch{state.unlock();throw error}
            try perform { _=try self.resolve(path) }
            state.lock();defer{state.unlock()};try checkLocked();guard streams.count<129,nextToken>0,nextToken<UInt64.max else{throw Failure(-5)}
            let token=nextToken;nextToken+=1;streams[token]=Stream(token:token,path:path)
            // Immediately available metadata can describe an unhydrated cloud file.
            // Its data length is deliberately unknown until the coordinated read.
            result.pointee=OmniMediaStream(token:token,byte_length:-1);return 0
        }catch{return errorCode(error)}
    }
    private func read(_ token: UInt64, output: UnsafeMutablePointer<UInt8>?, capacity: Int) -> Int64 {
        guard token != 0,let output=output,capacity>0,capacity<=65_536 else{return -3}
        let stream: Stream,until: UInt64
        state.lock()
        do {
            try checkLocked();guard let found=streams[token] else{throw Failure(-3)};stream=found;until=min(deadline,Self.now+30_000_000_000)
            if !found.started {
                guard producer==nil else{throw Failure(-5)};found.started=true;producer=token
                io.async {self.produce(found)}
            }
            state.unlock()
        }catch{state.unlock();return Int64(errorCode(error))}
        return stream.take(output,capacity:capacity,deadline:until,owner:self)
    }
    private func produce(_ stream: Stream) {
        let outcome: Int32
        do {
            try check();try stream.check()
            try coordinate(stream.path,options:[]) { readable in
                try stream.check();let values=try readable.resourceValues(forKeys:[.isSymbolicLinkKey,.isAliasFileKey])
                guard values.isSymbolicLink != true,values.isAliasFile != true else{throw Failure(-3)}
                let fd=try self.openRelative(stream.path,directory:false)
                let input=FileHandle(fileDescriptor:fd,closeOnDealloc:true);defer{try? input.close()}
                var before=stat(),named=stat()
                guard fstat(fd,&before)==0,lstat(readable.path,&named)==0,Self.sameFile(before,named),before.st_size>0,UInt64(before.st_size)<=Self.maximum else{throw Failure(-5)}
                var copied: UInt64=0
                while true {
                    try self.check();try stream.check()
                    let data=try autoreleasepool {try input.read(upToCount:65_536) ?? Data()}
                    try self.check();try stream.check();if data.isEmpty{break}
                    copied+=UInt64(data.count);guard data.count<=65_536,copied<=Self.maximum,copied<=UInt64(before.st_size) else{throw Failure(-5)}
                    try stream.publish(data)
                }
                var after=stat()
                guard copied==UInt64(before.st_size),fstat(fd,&after)==0,Self.sameFile(before,after),before.st_size==after.st_size,
                      before.st_mtimespec.tv_sec==after.st_mtimespec.tv_sec,before.st_mtimespec.tv_nsec==after.st_mtimespec.tv_nsec,
                      before.st_ctimespec.tv_sec==after.st_ctimespec.tv_sec,before.st_ctimespec.tv_nsec==after.st_ctimespec.tv_nsec else{throw Failure(-5)}
            }
            outcome=0
        }catch{outcome=errorCode(error)}
        // EOF becomes visible only after the accessor, descriptor and coordinator
        // have finished. The next metadata/stream operation can then proceed.
        state.lock();if producer==stream.token{producer=nil};state.unlock();stream.finish(outcome)
    }
    private func close(_ token: UInt64) { state.lock();let stream=streams.removeValue(forKey:token);state.unlock();stream?.stop(-2) }
    func cancel() { cancel(-2) }
    private func cancel(_ code: Int32, expectedDeadline: UInt64?=nil) {
        state.lock();guard stopped==0,expectedDeadline==nil||deadline==expectedDeadline else{state.unlock();return}
        stopped=code;expiry?.cancel();let readers=Array(streams.values),active=coordinator;state.unlock()
        readers.forEach{$0.stop(code)}
        // Apple's contract permits cancel on any thread and specifies immediate
        // return. A running accessor still has to finish its own I/O/cleanup.
        active?.cancel()
    }
    private func checkLocked() throws {if stopped != 0{throw Failure(stopped)};if Self.now>=deadline{throw Failure(-4)} }
    private func check() throws {state.lock();defer{state.unlock()};try checkLocked()}
    private func armLocked(_ value: UInt64) {
        expiry?.cancel();deadline=value
        let work=DispatchWorkItem{[weak self]in self?.cancel(-4,expectedDeadline:value)};expiry=work
        DispatchQueue.global(qos:.utility).asyncAfter(deadline:DispatchTime(uptimeNanoseconds:value),execute:work)
    }
    private func perform<T>(_ operation: @escaping () throws -> T) throws -> T {
        try check();state.lock();let until=min(deadline,Self.now+30_000_000_000);state.unlock();let answer=Answer<T>()
        io.async {do{try self.check();answer.finish(.success(try operation()))}catch{answer.finish(.failure(error))}}
        while answer.ready.wait(timeout:.now() + .milliseconds(250)) == .timedOut {try check();if Self.now>=until{cancel(-4);throw Failure(-4)}}
        try check();return try answer.result()
    }
    private func coordinate<T>(_ relative: String, options: NSFileCoordinator.ReadingOptions, operation: (URL) throws -> T) throws -> T {
        var target=root;if !relative.isEmpty{for component in relative.split(separator:"/"){target.appendPathComponent(String(component))}}
        let coordinator=NSFileCoordinator(filePresenter:nil)
        state.lock();do{try checkLocked();guard self.coordinator==nil else{throw Failure(-5)};self.coordinator=coordinator;state.unlock()}catch{state.unlock();throw error}
        defer{state.lock();if self.coordinator===coordinator{self.coordinator=nil};state.unlock()}
        var coordinationError: NSError?,answer: Result<T, Error>?
        coordinator.coordinate(readingItemAt:target,options:options,error:&coordinationError){readable in
            do {
                try self.check()
                guard readable.isFileURL,readable.standardizedFileURL.path==target.standardizedFileURL.path else{throw Failure(-3)}
                // A moved provider URL requires a fresh picker rather than silently
                // widening the selected directory or following a replacement root.
                if self.rootFD>=0{try self.checkRoot()}
                answer = .success(try operation(readable))
            }catch{answer = .failure(error)}
        }
        try check();if coordinationError != nil{throw Failure(-5)};guard let answer=answer else{throw Failure(-5)};return try answer.get()
    }
    private func checkRoot() throws {
        let fd=Darwin.open(root.path,O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);guard fd>=0 else{throw Failure(-3)};defer{Darwin.close(fd)}
        var info=stat();guard let original=rootIdentity,fstat(fd,&info)==0,Self.sameFile(original,info) else{throw Failure(-3)}
    }
    private static func sameFile(_ first: stat, _ second: stat) -> Bool {first.st_dev==second.st_dev&&first.st_ino==second.st_ino&&(first.st_mode & mode_t(S_IFMT))==(second.st_mode & mode_t(S_IFMT))}
    private func openRelative(_ path: String, directory: Bool) throws -> Int32 {
        guard rootFD>=0 else{throw Failure(-3)}
        let parts=path.isEmpty ? [] : (Self.components(path) ?? [])
        guard path.isEmpty || !parts.isEmpty else{throw Failure(-3)}
        var current=Darwin.dup(rootFD);guard current>=0 else{throw Failure(-5)}
        for(index,part)in parts.enumerated(){
            let folder=index+1<parts.count||directory
            let next=part.withCString{Darwin.openat(current,$0,O_RDONLY|O_NOFOLLOW|O_CLOEXEC|O_NONBLOCK|(folder ? O_DIRECTORY:0))}
            Darwin.close(current);guard next>=0 else{throw Failure(-3)};current=next
        }
        var info=stat();guard fstat(current,&info)==0,(info.st_mode & mode_t(S_IFMT))==mode_t(directory ? S_IFDIR:S_IFREG) else{Darwin.close(current);throw Failure(-3)};return current
    }
    private func children(_ path: String) throws -> [String: Node] {
        if let cached=directories[path]{return cached};guard directories.count<256 else{throw Failure(-5)}
        let nodes: [String:Node]=try coordinate(path,options:[.withoutChanges,.immediatelyAvailableMetadataOnly]){readable in
            let fd=try self.openRelative(path,directory:true)
            guard let directory=fdopendir(fd) else{Darwin.close(fd);throw Failure(-5)};defer{closedir(directory)}
            var named=stat(),opened=stat();guard lstat(readable.path,&named)==0,fstat(dirfd(directory),&opened)==0,Self.sameFile(named,opened) else{throw Failure(-3)}
            var result:[String:Node]=[:]
            while true {
                try self.check();errno=0;guard let entry=readdir(directory) else{if errno != 0{throw Failure(-5)};break}
                self.rows+=1;guard self.rows<=4096 else{throw Failure(-5)}
                var raw=entry.pointee.d_name;let capacity=MemoryLayout.size(ofValue:raw)
                let name=withUnsafePointer(to:&raw){pointer in pointer.withMemoryRebound(to:CChar.self,capacity:capacity){String(validatingUTF8:$0)}}
                guard let name=name,name != ".",name != ".." else{continue}
                self.metadataBytes+=name.utf8.count+64;guard self.metadataBytes<=2*1024*1024 else{throw Failure(-5)}
                guard Self.safeName(name) else{continue}
                var info=stat();guard name.withCString({fstatat(dirfd(directory),$0,&info,AT_SYMLINK_NOFOLLOW)})==0 else{throw Failure(-5)}
                let kind=info.st_mode & mode_t(S_IFMT);guard kind==mode_t(S_IFDIR)||kind==mode_t(S_IFREG) else{continue}
                guard result[name]==nil else{throw Failure(-3)};result[name]=Node(directory:kind==mode_t(S_IFDIR),size:Int64(info.st_size))
            }
            return result
        }
        directories[path]=nodes;return nodes
    }
    private func resolve(_ path: String) throws -> Node {
        guard let components=Self.components(path) else{throw Failure(-3)};var parent=""
        for(index,name)in components.enumerated(){
            guard let node=try children(parent)[name] else{throw Failure(-1)}
            if index+1==components.count{guard !node.directory,node.size<=Int64(Self.maximum) else{throw Failure(-3)};return node}
            guard node.directory else{throw Failure(-3)};parent=parent.isEmpty ? name:parent+"/"+name
        }
        throw Failure(-3)
    }
    private static func safeName(_ value: String) -> Bool {
        !value.isEmpty && value != "." && value != ".." && value.utf8.count<=512 && !value.hasSuffix(".") && !value.hasSuffix(" ") &&
        !value.unicodeScalars.contains{CharacterSet.controlCharacters.contains($0)||["/","\\",":"].contains(String($0))}
    }
    private static func components(_ path: String) -> [String]? {
        guard !path.isEmpty,path.utf8.count<=2048 else{return nil};let values=path.split(separator:"/",omittingEmptySubsequences:false).map(String.init)
        return values.count<=8&&values.allSatisfy(safeName) ? values:nil
    }
    private func errorCode(_ error: Error) -> Int32 {state.lock();let code=stopped;state.unlock();if code != 0{return code};return (error as? Failure)?.code ?? -5}

    static func callbacks(_ folder: SelectedDiscFolder) -> OmniMediaCallbacks {
        let context=Unmanaged.passRetained(folder).toOpaque()
        return OmniMediaCallbacks(abi_version:1,context:context,open:{context,profile,path,length,stream in
            guard let context=context else{return -3};return Unmanaged<SelectedDiscFolder>.fromOpaque(context).takeUnretainedValue().open(profile,relative:path,length:length,result:stream)
        },read:{context,token,output,capacity in
            guard let context=context else{return -3};return Unmanaged<SelectedDiscFolder>.fromOpaque(context).takeUnretainedValue().read(token,output:output,capacity:capacity)
        },close:{context,token in
            if let context=context{Unmanaged<SelectedDiscFolder>.fromOpaque(context).takeUnretainedValue().close(token)}
        },cancel:{context in
            if let context=context{Unmanaged<SelectedDiscFolder>.fromOpaque(context).takeUnretainedValue().cancel()}
        },release:{context in
            if let context=context{let folder=Unmanaged<SelectedDiscFolder>.fromOpaque(context).takeRetainedValue();folder.cancel()}
        })
    }
}

/// The UI holds weak references. Native ownership and running producer work retain
/// their scopes independently, including after a reply is lost or a tab closes.
final class DiscFolderRegistry {
    private final class Weak {weak var value:SelectedDiscFolder?;init(_ value:SelectedDiscFolder){self.value=value}}
    private let lock=NSLock()
    private var folders:[UUID:Weak]=[:]
    func insert(_ value:SelectedDiscFolder){lock.lock();folders=folders.filter{$0.value.value != nil};folders[value.id]=Weak(value);lock.unlock()}
    func cancelAll(){lock.lock();let values=folders.values.compactMap{$0.value};folders.removeAll();lock.unlock();values.forEach{$0.cancel()}}
    deinit{cancelAll()}
}
