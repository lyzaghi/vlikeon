import Foundation

@MainActor
final class BrowserStore {
    struct Entry:Codable {let id:UUID;var title:String;let url:String;let visitedAt:Date}
    private struct State:Codable {var schema_version=1;var bookmarks:[Entry]=[];var history:[Entry]=[];var restoredTabs:[String]=[]}
    private var state=State()
    private let file:URL?
    var history:[Entry]{state.history}
    var bookmarks:[Entry]{state.bookmarks}
    var restoredTabs:[String]{state.restoredTabs}
    init(){
        file=FileManager.default.urls(for:.applicationSupportDirectory,in:.userDomainMask).first?.appendingPathComponent("OmniPlay/Browser/state.json")
        guard let file=file,let values=try? file.resourceValues(forKeys:[.fileSizeKey]),(values.fileSize ?? 0)<=1_048_576,let data=try? Data(contentsOf:file),let decoded=try? JSONDecoder().decode(State.self,from:data),decoded.schema_version==1,decoded.history.count<=256,decoded.bookmarks.count<=128,decoded.restoredTabs.count<=32 else{return}
        state=decoded
    }
    func visit(_ url:URL,title:String){guard valid(url)else{return};state.history.removeAll{$0.url==url.absoluteString};state.history.insert(Entry(id:UUID(),title:String(title.prefix(200)),url:url.absoluteString,visitedAt:Date()),at:0);state.history=Array(state.history.prefix(256));persist()}
    func bookmark(_ url:URL,title:String){guard valid(url)else{return};state.bookmarks.removeAll{$0.url==url.absoluteString};state.bookmarks.insert(Entry(id:UUID(),title:String(title.prefix(200)),url:url.absoluteString,visitedAt:Date()),at:0);state.bookmarks=Array(state.bookmarks.prefix(128));persist()}
    func removeBookmark(_ id:UUID){state.bookmarks.removeAll{$0.id==id};persist()}
    func clearHistory(){state.history.removeAll();persist()}
    func saveTabs(_ urls:[URL]){state.restoredTabs=Array(urls.filter(valid).prefix(32).map(\.absoluteString));persist()}
    private func valid(_ url:URL)->Bool{["https","http"].contains(url.scheme?.lowercased() ?? "")&&url.host != nil&&url.user==nil&&url.password==nil&&url.absoluteString.utf8.count<=2048}
    private func persist(){
        guard let file=file else{return}
        do {try FileManager.default.createDirectory(at:file.deletingLastPathComponent(),withIntermediateDirectories:true,attributes:[.protectionKey:FileProtectionType.completeUntilFirstUserAuthentication]);let bytes=try JSONEncoder().encode(state);guard bytes.count<=1_048_576 else{return};try bytes.write(to:file,options:.atomic);try FileManager.default.setAttributes([.protectionKey:FileProtectionType.completeUntilFirstUserAuthentication],ofItemAtPath:file.path);var folder=file.deletingLastPathComponent();var values=URLResourceValues();values.isExcludedFromBackup=true;try folder.setResourceValues(values)}catch{/* Browsing remains available; no remote fallback receives private state. */}
    }
}
