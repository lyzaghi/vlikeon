import Foundation
import UIKit
import UniformTypeIdentifiers
import WebKit

@MainActor
final class HubBridge: NSObject, WKScriptMessageHandlerWithReply, UIDocumentPickerDelegate {
    typealias Reply = (Any?, String?) -> Void
    private enum Selection { case importFile([String:Any], Reply, [String:String]?), exportFile([String:Any], Reply, URL), discFolder([String:Any], Reply) }
    private struct Pending {let expiry:DispatchWorkItem;let cancel:()->Void}
    private struct Export { let url: URL; let created: Date }
    private let core: NativeCore
    private let server: AssetServer
    private weak var presenter: UIViewController?
    weak var webView: WKWebView?
    var setGameFullscreen:((Bool)->Void)?
    private var selection: Selection?
    private var exports: [String: Export] = [:]
    private var pending:[String:Pending]=[:]
    private var generation:UInt64=0
    private var documentLifetime=NativeDocumentLifetime()
    private var preparingFolder=false
    private weak var picker:UIDocumentPickerViewController?
    private let folders=DiscFolderRegistry()
    private let io = DispatchQueue(label: "com.omniplay.ios.documents", qos: .userInitiated)
    private let documentPermit=DispatchSemaphore(value:4)
    private func documentError(_ error:Error)->String{if error is OmniFailure{return error.localizedDescription};return "The selected document could not be read or stored. Check access and available space, then retry."}

    init(core: NativeCore, server: AssetServer, presenter: UIViewController) { self.core=core;self.server=server;self.presenter=presenter }
    private func response(_ request: [String:Any], result: Any) -> [String:Any] { ["protocol_major":1,"protocol_minor":0,"id":request["id"] ?? "","status":"ok","result":result] }
    private func reject(_ request: [String:Any], _ reply: Reply, code: String, message: String) { reply(["protocol_major":1,"protocol_minor":0,"id":request["id"] ?? "","status":"error","error":["code":code,"message":message,"recoverable":true]],nil) }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage, replyHandler: @escaping Reply) {
        guard message.frameInfo.isMainFrame, message.webView === webView, server.owns(message.frameInfo.request.url), server.owns(webView?.url),
              let origin = server.baseURL, message.frameInfo.securityOrigin.protocol == "http", message.frameInfo.securityOrigin.host == "127.0.0.1", message.frameInfo.securityOrigin.port == origin.port,
              var request = message.body as? [String:Any], let id=request["id"] as? String, UUID(uuidString:id) != nil,
              (request["protocol_major"] as? Int)==1,(request["protocol_minor"] as? Int)==0, let deadline=request["deadline_ms"] as? NSNumber,
              let command=request["command"] as? [String:Any], let method=command["method"] as? String else { replyHandler(nil,"Unauthorized or malformed Game Hub request.");return }
        let now=Date().timeIntervalSince1970*1000
        guard deadline.doubleValue.isFinite,deadline.doubleValue>now, deadline.doubleValue-now<=120_000,pending.count<16,pending[id]==nil,
              let encoded=try? JSONSerialization.data(withJSONObject:request), encoded.count<=1_048_576 else { reject(request,replyHandler,code:"resource_limit",message:"The native request exceeds its deadline or resource bounds.");return }
        request["profile_id"]=core.profileID.uuidString.lowercased()
        let epoch=generation
        var completed=false
        let reply:Reply={ [weak self] value,error in guard !completed else{return};completed=true;if let pending=self?.pending.removeValue(forKey:id){pending.expiry.cancel()};replyHandler(value,error) }
        let expiry=DispatchWorkItem{[weak self]in guard let self=self else{return};self.expireSelection(id);self.reject(request,reply,code:"deadline_exceeded",message:"The native operation deadline expired.")}
        pending[id]=Pending(expiry:expiry,cancel:{[weak self]in self?.reject(request,reply,code:"cancelled",message:"The Game Hub document changed or closed.")})
        DispatchQueue.main.asyncAfter(deadline:.now() + .milliseconds(Int(ceil(deadline.doubleValue-now))),execute:expiry)
        pruneExports()
        let params=command["params"] as? [String:Any] ?? [:]
        switch method {
        case "presentation_fullscreen":guard let enabled=params["enabled"] as? Bool else{reject(request,reply,code:"invalid_request",message:"Fullscreen selection is invalid.");return};setGameFullscreen?(enabled);reply(response(request,result:["enabled":enabled]),nil)
        case "file_select_import": selectImport(request,reply:reply,firmware:nil)
        case "firmware_import_selected":
            guard let system=params["system"] as? String,let role=params["role"] as? String else {reject(request,reply,code:"invalid_request",message:"Firmware role is missing.");return}
            selectImport(request,reply:reply,firmware:["system":system,"role":role])
        case "file_select_export": createExport(request,params:params,reply:reply)
        case "disc_select_folder":selectDisc(request,reply:reply)
        default:
            core.request(request,owner:documentLifetime) { [weak self] value in
                guard let self=self else{reply(nil,"The Game Hub closed.");return}
                guard epoch==self.generation,self.alive(request) else{self.reject(request,reply,code:"cancelled",message:"The operation belongs to an expired Game Hub request.");return}
                switch value {
                case .success(let response):
                    if response["status"] as? String=="ok",["save_export","save_export_upload_commit","diagnostics_export","performance_run_export"].contains(method) {
                        guard let grant=params["destination_grant_id"] as? String,let export=self.exports.removeValue(forKey:grant) else{self.reject(request,reply,code:"permission_expired",message:"The export selection expired. Choose a new destination.");return}
                        var delivered=response;var result=response["result"] as? [String:Any] ?? [:];result["delivery"]="document_provider";delivered["result"]=result
                        self.presentExport(export.url,request:request,response:delivered,reply:reply)
                    }else{reply(response,nil)}
                case .failure(let error):self.reject(request,reply,code:"storage_failure",message:error.localizedDescription)
                }
            }
        }
    }
    private func selectImport(_ request:[String:Any],reply:@escaping Reply,firmware:[String:String]?) {
        guard selection==nil,!preparingFolder,let presenter=presenter,presenter.presentedViewController==nil else{reject(request,reply,code:"busy",message:"Finish the current document selection first.");return}
        selection = .importFile(request,reply,firmware)
        let picker=UIDocumentPickerViewController(forOpeningContentTypes:[.data,.item],asCopy:false)
        self.picker=picker;picker.allowsMultipleSelection=false;picker.delegate=self;presenter.present(picker,animated:true)
    }
    private func selectDisc(_ request:[String:Any],reply:@escaping Reply){
        guard selection==nil,!preparingFolder,let presenter=presenter,presenter.presentedViewController==nil else{reject(request,reply,code:"busy",message:"Finish the current document selection first.");return}
        selection = .discFolder(request,reply)
        let picker=UIDocumentPickerViewController(forOpeningContentTypes:[.folder],asCopy:false)
        self.picker=picker;picker.allowsMultipleSelection=false;picker.delegate=self;presenter.present(picker,animated:true)
    }
    private func createExport(_ request:[String:Any],params:[String:Any],reply:@escaping Reply) {
        let epoch=generation
        guard let name=params["suggested_name"] as? String, !name.isEmpty,name != ".",name != "..",name.utf8.count<=128,name.utf8.allSatisfy({(48...57).contains($0)||(65...90).contains($0)||(97...122).contains($0)||[45,95,46].contains($0)}),exports.count<16 else{reject(request,reply,code:"invalid_request",message:"The export name or pending export count is invalid.");return}
        let directory=core.root.appendingPathComponent("NativeExports",isDirectory:true).appendingPathComponent(UUID().uuidString,isDirectory:true),url=directory.appendingPathComponent(name)
        guard documentPermit.wait(timeout:.now()) == .success else{reject(request,reply,code:"busy",message:"Document I/O is still finishing.");return}
        let lifetime=documentLifetime
        io.async {
            defer{self.documentPermit.signal()}
            do {guard lifetime.active,((request["deadline_ms"] as? NSNumber)?.doubleValue ?? 0) > Date().timeIntervalSince1970*1000 else{throw OmniFailure.message("Document operation expired.")};try FileManager.default.createDirectory(at:directory,withIntermediateDirectories:true,attributes:[.protectionKey:FileProtectionType.completeUntilFirstUserAuthentication])
                DispatchQueue.main.async {
                    guard epoch==self.generation,self.alive(request) else{self.removePrivateExport(url);self.reject(request,reply,code:"cancelled",message:"Export selection expired.");return}
                    self.core.authorize(["kind":"export","path":url.path]){result in
                        guard epoch==self.generation,self.alive(request) else{self.removePrivateExport(url);self.reject(request,reply,code:"cancelled",message:"Export selection expired.");return}
                        switch result{case .success(let value):if let grant=value["grant_id"] as? String{self.exports[grant]=Export(url:url,created:Date())};reply(self.response(request,result:value),nil)
                        case .failure(let error):self.removePrivateExport(url);self.reject(request,reply,code:"storage_failure",message:error.localizedDescription)}
                    }
                }
            }catch{DispatchQueue.main.async{self.reject(request,reply,code:"storage_failure",message:self.documentError(error))}}
        }
    }
    private func presentExport(_ url:URL,request:[String:Any],response:[String:Any],reply:@escaping Reply) {
        guard selection==nil,!preparingFolder,alive(request),let presenter=presenter,presenter.presentedViewController==nil else{removePrivateExport(url);reject(request,reply,code:"busy",message:"Finish the current document selection before exporting.");return}
        selection = .exportFile(response,reply,url)
        let picker=UIDocumentPickerViewController(forExporting:[url],asCopy:true);self.picker=picker;picker.delegate=self;presenter.present(picker,animated:true)
    }
    func documentPicker(_ controller:UIDocumentPickerViewController,didPickDocumentsAt urls:[URL]) {
        guard controller===picker,let selection=selection else{return};self.selection=nil;picker=nil
        switch selection {
        case .exportFile(let response,let reply,let source):removePrivateExport(source);reply(response,nil)
        case .importFile(let request,let reply,let firmware):
            guard urls.count==1,let source=urls.first,alive(request) else{reject(request,reply,code:"cancelled",message:"No document was selected before the deadline.");return}
            stageImport(source,request:request,reply:reply,firmware:firmware)
        case .discFolder(let request,let reply):
            guard urls.count==1,let source=urls.first,alive(request) else{reject(request,reply,code:"cancelled",message:"No disc folder was selected before the deadline.");return}
            prepareDisc(source,request:request,reply:reply)
        }
    }
    func documentPickerWasCancelled(_ controller:UIDocumentPickerViewController) {
        guard controller===picker,let selection=selection else{return};self.selection=nil;picker=nil
        switch selection{case .importFile(let request,let reply,_):reject(request,reply,code:"cancelled",message:"Document selection was cancelled.")
        case .exportFile(let response,let reply,let source):removePrivateExport(source);reject(response,reply,code:"cancelled",message:"Export was cancelled.")
        case .discFolder(let request,let reply):reject(request,reply,code:"cancelled",message:"Disc folder selection was cancelled.")}
    }
    private func alive(_ request:[String:Any])->Bool{guard let deadline=request["deadline_ms"] as? NSNumber else{return false};return deadline.doubleValue>Date().timeIntervalSince1970*1000}
    private func prepareDisc(_ source:URL,request:[String:Any],reply:@escaping Reply){
        let epoch=generation;preparingFolder=true
        core.expireCapabilities{[weak self]result in
            guard let self=self else{reply(nil,"The Game Hub closed.");return}
            guard epoch==self.generation,self.alive(request) else{if epoch==self.generation{self.preparingFolder=false};self.reject(request,reply,code:"cancelled",message:"Disc folder selection expired.");return}
            if case .failure = result{self.preparingFolder=false;self.reject(request,reply,code:"storage_failure",message:"Local application storage is unavailable.");return}
            let profile=self.core.profileID,deadline=(request["deadline_ms"] as? NSNumber)?.doubleValue ?? 0
            self.io.async{
                do{
                    let folder=try SelectedDiscFolder(profile:profile,source:source,requestDeadlineMilliseconds:deadline)
                    DispatchQueue.main.async{
                        guard epoch==self.generation,self.alive(request) else{folder.cancel();if epoch==self.generation{self.preparingFolder=false};self.reject(request,reply,code:"cancelled",message:"Disc folder selection expired.");return}
                        self.folders.insert(folder)
                        self.io.async{
                            do{
                                let metadata=try folder.prepare()
                                DispatchQueue.main.async{
                                    guard epoch==self.generation,self.alive(request) else{folder.cancel();if epoch==self.generation{self.preparingFolder=false};self.reject(request,reply,code:"cancelled",message:"Disc folder selection expired.");return}
                                    self.core.authorizeMedia(metadata,folder:folder){result in
                                        guard epoch==self.generation else{folder.cancel();return};self.preparingFolder=false
                                        guard self.alive(request) else{folder.cancel();self.reject(request,reply,code:"cancelled",message:"Disc folder selection expired.");return}
                                        switch result{case .success(let value):reply(self.response(request,result:value),nil)
                                        case .failure:folder.cancel();self.reject(request,reply,code:"storage_failure",message:"The selected disc folder could not be authorized. Choose it again.")}
                                    }
                                }
                            }catch{folder.cancel();DispatchQueue.main.async{if epoch==self.generation{self.preparingFolder=false;self.reject(request,reply,code:"storage_failure",message:"Choose a readable folder with at most 64 CUE, M3U, CHD, ISO or PBP entry points. Ambiguous names, inaccessible providers or oversized directories cannot be imported.")}}}
                        }
                    }
                }catch{DispatchQueue.main.async{if epoch==self.generation{self.preparingFolder=false;self.reject(request,reply,code:"storage_failure",message:"The folder is unavailable or too many provider operations are still finishing. Choose it again.")}}}
            }
        }
    }
    private func expireSelection(_ id:String){
        guard let current=selection else{return};let request:[String:Any]
        switch current{case .importFile(let value,_,_):request=value;case .discFolder(let value,_):request=value;case .exportFile(let value,_,_):request=value}
        guard request["id"] as? String==id else{return};selection=nil;picker?.dismiss(animated:true);picker=nil
        if case .exportFile(_,_,let source)=current{removePrivateExport(source)}
    }
    func resetDocument(){
        documentLifetime.invalidate();documentLifetime=NativeDocumentLifetime()
        generation &+= 1;folders.cancelAll();preparingFolder=false
        if case .exportFile(_,_,let source)?=selection{removePrivateExport(source)}
        selection=nil;picker?.dismiss(animated:false);picker=nil
        let requests=Array(pending.values);requests.forEach{$0.cancel()}
        let files=Array(exports.values);exports.removeAll();files.forEach{removePrivateExport($0.url)}
        core.detach()
    }
    private func stageImport(_ source:URL,request:[String:Any],reply:@escaping Reply,firmware:[String:String]?) {
        let root=core.root,originalTitle=String(source.deletingPathExtension().lastPathComponent.prefix(120))
        let epoch=generation,deadline=(request["deadline_ms"] as? NSNumber)?.doubleValue ?? 0
        let maximum:UInt64=firmware==nil ? 4*1024*1024*1024:128*1024*1024
        guard documentPermit.wait(timeout:.now()) == .success else{reject(request,reply,code:"busy",message:"Document I/O is still finishing.");return}
        let lifetime=documentLifetime
        io.async {
            defer{self.documentPermit.signal()}
            let scoped=source.startAccessingSecurityScopedResource();defer{if scoped{source.stopAccessingSecurityScopedResource()}}
            let extensionName=source.pathExtension.lowercased().filter{$0.isASCII&&($0.isLetter||$0.isNumber)}
            let target=root.appendingPathComponent("NativeInbox",isDirectory:true).appendingPathComponent(UUID().uuidString.lowercased()+"."+String(extensionName.prefix(12)))
            do {
                guard scoped,lifetime.active,deadline>Date().timeIntervalSince1970*1000 else{throw OmniFailure.message("Document access was denied or expired.")}
                try FileManager.default.createDirectory(at:target.deletingLastPathComponent(),withIntermediateDirectories:true,attributes:[.protectionKey:FileProtectionType.completeUntilFirstUserAuthentication])
                var coordinationError:NSError?,copyError:Error?
                NSFileCoordinator().coordinate(readingItemAt:source,options:[],error:&coordinationError){ readable in
                    do {
                        let values=try readable.resourceValues(forKeys:[.isRegularFileKey,.isSymbolicLinkKey,.fileSizeKey])
                        guard values.isRegularFile==true,values.isSymbolicLink != true,(values.fileSize ?? 0)>0,UInt64(values.fileSize ?? 0)<=maximum else{throw OmniFailure.message("Choose a regular file within the content size limit.")}
                        guard FileManager.default.createFile(atPath:target.path,contents:nil,attributes:[.protectionKey:FileProtectionType.completeUntilFirstUserAuthentication])else{throw OmniFailure.message("The private import staging file could not be created.")}
                        let input=try FileHandle(forReadingFrom:readable),output=try FileHandle(forWritingTo:target);defer{try? input.close();try? output.close()}
                        var total:UInt64=0
                        while let block=try input.read(upToCount:256*1024),!block.isEmpty{total+=UInt64(block.count);guard lifetime.active,total<=maximum,deadline>Date().timeIntervalSince1970*1000 else{throw OmniFailure.message("The selected file exceeds its import size or time limit.")};try output.write(contentsOf:block)}
                        guard total==UInt64(values.fileSize ?? 0)else{throw OmniFailure.message("The document changed during import.")};try output.synchronize()
                    }catch{copyError=error}
                }
                if let error=coordinationError{throw error};if let error=copyError{throw error}
                DispatchQueue.main.async {
                    guard epoch==self.generation,self.alive(request) else{try? FileManager.default.removeItem(at:target);self.reject(request,reply,code:"cancelled",message:"Document import expired.");return}
                    var authorization:[String:Any]=["kind":firmware==nil ? "import":"firmware","path":target.path]
                    if let firmware=firmware{authorization["system"]=firmware["system"];authorization["role"]=firmware["role"]}
                    self.core.authorize(authorization){ result in
                        // Rust retains the opened import descriptor. The private staging
                        // pathname can be unlinked without retaining a second ROM copy.
                        try? FileManager.default.removeItem(at:target)
                        guard epoch==self.generation,self.alive(request) else{self.reject(request,reply,code:"cancelled",message:"Document import expired.");return}
                        switch result{case .success(var value):if firmware==nil{value["suggested_title"]=originalTitle};reply(self.response(request,result:value),nil)
                        case .failure(let error):self.reject(request,reply,code:"storage_failure",message:error.localizedDescription)}
                    }
                }
            }catch{try? FileManager.default.removeItem(at:target);DispatchQueue.main.async{self.reject(request,reply,code:"storage_failure",message:self.documentError(error))}}
        }
    }
    private func removePrivateExport(_ url:URL){let base=core.root.appendingPathComponent("NativeExports").standardizedFileURL;let parent=url.deletingLastPathComponent().standardizedFileURL;guard parent.deletingLastPathComponent()==base,UUID(uuidString:parent.lastPathComponent) != nil else{return};try? FileManager.default.removeItem(at:url);try? FileManager.default.removeItem(at:parent)}
    private func pruneExports(){for(key,value)in exports where Date().timeIntervalSince(value.created)>300{removePrivateExport(value.url);exports.removeValue(forKey:key)}}
}
