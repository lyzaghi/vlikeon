import UIKit
import WebKit

@MainActor
final class BrowserViewController:UIViewController,UITextFieldDelegate,WKNavigationDelegate,WKUIDelegate,WKDownloadDelegate {
    private struct Tab {let id:UUID;let web:WKWebView;let isPrivate:Bool}
    private struct Download {let value:WKDownload;let url:URL;let began:TimeInterval}
    private let store=BrowserStore()
    private var tabs:[Tab]=[]
    private var selected:UUID?
    private var privateStore=WKWebsiteDataStore.nonPersistent()
    private let address=UITextField(),security=UILabel(),surface=UIView(),progress=UIProgressView(progressViewStyle:.bar)
    private let back=UIButton(type:.system),forward=UIButton(type:.system),reload=UIButton(type:.system),tabButton=UIButton(type:.system),menu=UIButton(type:.system)
    private var observations:[NSKeyValueObservation]=[]
    private var downloads:[ObjectIdentifier:Download]=[:]
    private var retiringDownloads:[ObjectIdentifier:Download]=[:]
    private var downloadTimer:Timer?
    private var removingWebsiteData=false
    private var current:Tab?{tabs.first{$0.id==selected}}
    private let maximumDownload:Int64=2*1024*1024*1024

    override func viewDidLoad(){
        super.viewDidLoad();view.backgroundColor = .systemBackground
        address.borderStyle = .roundedRect;address.placeholder="Nhập địa chỉ HTTPS…";address.keyboardType = .URL;address.autocapitalizationType = .none;address.autocorrectionType = .no;address.returnKeyType = .go;address.clearButtonMode = .whileEditing;address.delegate=self;address.accessibilityLabel="Địa chỉ website"
        security.font = .preferredFont(forTextStyle:.caption1);security.numberOfLines=1;security.accessibilityLabel="Bảo mật kết nối"
        let addressRow=UIStackView(arrangedSubviews:[security,address]);addressRow.axis = .horizontal;addressRow.spacing=8
        configure(back,symbol:"chevron.left",label:"Quay lại"){[weak self]in self?.current?.web.goBack()}
        configure(forward,symbol:"chevron.right",label:"Đi tới"){[weak self]in self?.current?.web.goForward()}
        configure(reload,symbol:"arrow.clockwise",label:"Tải lại"){[weak self]in self?.current?.web.reload()}
        configure(tabButton,symbol:"square.on.square",label:"Quản lý thẻ"){[weak self]in self?.showTabs()}
        configure(menu,symbol:"ellipsis.circle",label:"Dấu trang và lịch sử"){[weak self]in self?.showMenu()}
        let controls=UIStackView(arrangedSubviews:[back,forward,reload,tabButton,menu]);controls.distribution = .fillEqually
        let stack=UIStackView(arrangedSubviews:[addressRow,progress,surface,controls]);stack.axis = .vertical;stack.spacing=6;stack.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(stack)
        NSLayoutConstraint.activate([stack.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:8),stack.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-8),stack.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor,constant:6),stack.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor),address.heightAnchor.constraint(greaterThanOrEqualToConstant:44),controls.heightAnchor.constraint(equalToConstant:46)])
        let restored=store.restoredTabs.compactMap(URL.init(string:))
        if restored.isEmpty{newTab(privateMode:false,url:nil)}else{for url in restored{newTab(privateMode:false,url:url)}}
    }
    private func configure(_ button:UIButton,symbol:String,label:String,action:@escaping()->Void){button.setImage(UIImage(systemName:symbol),for:.normal);button.accessibilityLabel=label;button.addAction(UIAction{_ in action()},for:.touchUpInside)}
    func open(_ url:URL){loadViewIfNeeded();guard allowed(url),!removingWebsiteData else{return};if current==nil{newTab(privateMode:false,url:url)}else{current?.web.load(URLRequest(url:url))}}
    private func allowed(_ url:URL)->Bool{["https","http"].contains(url.scheme?.lowercased() ?? "")&&url.host != nil&&url.user==nil&&url.password==nil&&url.absoluteString.utf8.count<=8192}
    private func newTab(privateMode:Bool,url:URL?){
        guard !removingWebsiteData else{return}
        guard tabs.count<32 else{notice("Đã đạt giới hạn 32 thẻ. Đóng một thẻ để mở thêm.");return}
        let config=WKWebViewConfiguration();config.websiteDataStore=privateMode ? privateStore:.default();config.allowsInlineMediaPlayback=true;config.mediaTypesRequiringUserActionForPlayback = .all
        // Browsing configurations never receive the Game Hub's native message handler.
        let web=WKWebView(frame:.zero,configuration:config);web.navigationDelegate=self;web.uiDelegate=self;web.allowsBackForwardNavigationGestures=true;web.isInspectable=false
        let tab=Tab(id:UUID(),web:web,isPrivate:privateMode);tabs.append(tab);select(tab.id)
        if let url=url,allowed(url){web.load(URLRequest(url:url))}
    }
    private func select(_ id:UUID){
        guard let tab=tabs.first(where:{$0.id==id})else{return};selected=id;observations.removeAll();surface.subviews.forEach{$0.removeFromSuperview()}
        let web=tab.web;web.translatesAutoresizingMaskIntoConstraints=false;surface.addSubview(web);NSLayoutConstraint.activate([web.leadingAnchor.constraint(equalTo:surface.leadingAnchor),web.trailingAnchor.constraint(equalTo:surface.trailingAnchor),web.topAnchor.constraint(equalTo:surface.topAnchor),web.bottomAnchor.constraint(equalTo:surface.bottomAnchor)])
        for key in [\WKWebView.estimatedProgress]{observations.append(web.observe(key,options:[.initial,.new]){[weak self]_,_ in DispatchQueue.main.async{self?.refresh()}})}
        observations.append(web.observe(\.url,options:[.new]){[weak self]_,_ in DispatchQueue.main.async{self?.refresh()}})
        observations.append(web.observe(\.canGoBack,options:[.new]){[weak self]_,_ in DispatchQueue.main.async{self?.refresh()}})
        observations.append(web.observe(\.canGoForward,options:[.new]){[weak self]_,_ in DispatchQueue.main.async{self?.refresh()}})
        refresh()
    }
    private func refresh(){guard let tab=current else{return};if !address.isFirstResponder{address.text=tab.web.url?.absoluteString ?? ""};security.text=(tab.isPrivate ? "◐ ":"")+(tab.web.url?.scheme=="https" ? "🔒":"HTTP");security.textColor=tab.web.url?.scheme=="https" ? .secondaryLabel:.systemOrange;progress.progress=Float(tab.web.estimatedProgress);progress.isHidden=tab.web.estimatedProgress>=1||tab.web.url==nil;back.isEnabled=tab.web.canGoBack;forward.isEnabled=tab.web.canGoForward;tabButton.accessibilityValue="\(tabs.count) thẻ"}
    private func saveSession(){store.saveTabs(tabs.filter{!$0.isPrivate}.compactMap{$0.web.url})}
    func textFieldShouldReturn(_ textField:UITextField)->Bool{
        let value=(address.text ?? "").trimmingCharacters(in:.whitespacesAndNewlines)
        guard let url=URL(string:value.contains("://") ? value:"https://"+value),allowed(url)else{notice("Nhập một địa chỉ HTTP hoặc HTTPS hợp lệ. Các đường dẫn tệp và mã thực thi không được mở từ thanh địa chỉ.");return false}
        address.resignFirstResponder();open(url);return true
    }
    private func showTabs(){
        let sheet=UIAlertController(title:"Thẻ trình duyệt",message:nil,preferredStyle:.actionSheet)
        for tab in tabs{let title=(tab.isPrivate ? "◐ ":"")+String((tab.web.title ?? tab.web.url?.host ?? "Thẻ mới").prefix(60));sheet.addAction(UIAlertAction(title:title,style:.default){[weak self]_ in self?.select(tab.id)})}
        sheet.addAction(UIAlertAction(title:"Thẻ mới",style:.default){[weak self]_ in self?.newTab(privateMode:false,url:nil)})
        sheet.addAction(UIAlertAction(title:"Thẻ riêng tư mới",style:.default){[weak self]_ in self?.newTab(privateMode:true,url:nil)})
        sheet.addAction(UIAlertAction(title:"Đóng thẻ hiện tại",style:.destructive){[weak self]_ in self?.closeTab()});sheet.addAction(UIAlertAction(title:"Hủy",style:.cancel));presentSheet(sheet,from:tabButton)
    }
    private func closeTab(){
        guard let selected=selected,let index=tabs.firstIndex(where:{$0.id==selected})else{return};tabs[index].web.stopLoading();tabs[index].web.navigationDelegate=nil;tabs[index].web.uiDelegate=nil;tabs.remove(at:index)
        if !tabs.contains(where:{$0.isPrivate}){privateStore = .nonPersistent()}
        if tabs.isEmpty{newTab(privateMode:false,url:nil)}else{select(tabs[min(index,tabs.count-1)].id)};saveSession()
    }
    private func showMenu(){
        let sheet=UIAlertController(title:"OmniPlay",message:current?.isPrivate==true ? "Thẻ riêng tư không được ghi vào lịch sử hoặc danh sách khôi phục thẻ.":nil,preferredStyle:.actionSheet)
        sheet.addAction(UIAlertAction(title:"Lưu dấu trang",style:.default){[weak self]_ in guard let self=self,let tab=self.current,let url=tab.web.url else{return};self.store.bookmark(url,title:tab.web.title ?? url.host ?? "Website");self.notice(self.store.persistenceError ?? "Đã lưu dấu trang theo yêu cầu của bạn.")})
        sheet.addAction(UIAlertAction(title:"Dấu trang",style:.default){[weak self]_ in self?.showEntries(bookmarks:true)})
        sheet.addAction(UIAlertAction(title:"Lịch sử",style:.default){[weak self]_ in self?.showEntries(bookmarks:false)})
        sheet.addAction(UIAlertAction(title:"Quản lý dấu trang",style:.default){[weak self]_ in self?.manageBookmarks()})
        sheet.addAction(UIAlertAction(title:"Xóa lịch sử cục bộ",style:.destructive){[weak self]_ in self?.confirmClearHistory()})
        sheet.addAction(UIAlertAction(title:"Xóa cookie và dữ liệu website",style:.destructive){[weak self]_ in self?.confirmClearWebsiteData()})
        if !downloads.isEmpty{sheet.addAction(UIAlertAction(title:"Dừng các lượt tải",style:.destructive){[weak self]_ in self?.cancelDownloads()})}
        sheet.addAction(UIAlertAction(title:"Hủy",style:.cancel));presentSheet(sheet,from:menu)
    }
    private func showEntries(bookmarks:Bool){let entries=bookmarks ? store.bookmarks:store.history;let sheet=UIAlertController(title:bookmarks ? "Dấu trang":"Lịch sử cục bộ",message:entries.isEmpty ? "Chưa có mục nào.":nil,preferredStyle:.actionSheet);for entry in entries.prefix(30){sheet.addAction(UIAlertAction(title:String(entry.title.prefix(64)),style:.default){[weak self]_ in if let url=URL(string:entry.url){self?.open(url)}})};sheet.addAction(UIAlertAction(title:"Đóng",style:.cancel));presentSheet(sheet,from:menu)}
    private func manageBookmarks(){
        let sheet=UIAlertController(title:"Quản lý dấu trang",message:store.bookmarks.isEmpty ? "Chưa có dấu trang.":nil,preferredStyle:.actionSheet)
        for entry in store.bookmarks{sheet.addAction(UIAlertAction(title:String(entry.title.prefix(64)),style:.default){[weak self]_ in self?.editBookmark(entry)})}
        sheet.addAction(UIAlertAction(title:"Đóng",style:.cancel));presentSheet(sheet,from:menu)
    }
    private func editBookmark(_ entry:BrowserStore.Entry){
        let alert=UIAlertController(title:"Chỉnh sửa dấu trang",message:entry.url,preferredStyle:.alert)
        alert.addTextField{$0.text=entry.title}
        alert.addAction(UIAlertAction(title:"Hủy",style:.cancel))
        alert.addAction(UIAlertAction(title:"Lưu tên",style:.default){[weak self,weak alert]_ in guard let self=self else{return};self.store.renameBookmark(entry.id,title:alert?.textFields?.first?.text ?? "");if let error=self.store.persistenceError{self.notice(error)}})
        alert.addAction(UIAlertAction(title:"Xóa dấu trang",style:.destructive){[weak self]_ in guard let self=self else{return};self.store.removeBookmark(entry.id);if let error=self.store.persistenceError{self.notice(error)}})
        present(alert,animated:true)
    }
    private func confirmClearHistory(){
        let alert=UIAlertController(title:"Xóa lịch sử duyệt web?",message:"Chỉ xóa danh sách lịch sử cục bộ. Cookie, dấu trang và dữ liệu trò chơi vẫn được giữ.",preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:"Hủy",style:.cancel));alert.addAction(UIAlertAction(title:"Xóa lịch sử",style:.destructive){[weak self]_ in guard let self=self else{return};self.store.clearHistory();if let error=self.store.persistenceError{self.notice(error)}});present(alert,animated:true)
    }
    private func confirmClearWebsiteData(){
        guard !removingWebsiteData else{return}
        guard downloads.isEmpty,retiringDownloads.isEmpty else{notice("Hãy dừng các lượt tải và chờ hoàn tất hủy trước khi xóa dữ liệu website.");return}
        let alert=UIAlertController(title:"Xóa dữ liệu website?",message:"Đóng các thẻ thông thường và xóa cookie, bộ nhớ đệm, dữ liệu website. Bạn có thể bị đăng xuất. Thư viện game, điểm lưu, dấu trang và thẻ riêng tư được giữ.",preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:"Hủy",style:.cancel));alert.addAction(UIAlertAction(title:"Xóa dữ liệu",style:.destructive){[weak self]_ in
            guard let self=self else{return};self.removingWebsiteData=true
            for tab in self.tabs where !tab.isPrivate{tab.web.stopLoading();tab.web.navigationDelegate=nil;tab.web.uiDelegate=nil;tab.web.removeFromSuperview()}
            self.tabs.removeAll{!$0.isPrivate};self.observations.removeAll();self.selected=nil
            if let tab=self.tabs.first{self.select(tab.id)}else{self.surface.subviews.forEach{$0.removeFromSuperview()}}
            self.saveSession()
            WKWebsiteDataStore.default().removeData(ofTypes:WKWebsiteDataStore.allWebsiteDataTypes(),modifiedSince:.distantPast){[weak self] in
                guard let self=self else{return};self.removingWebsiteData=false
                if self.tabs.isEmpty{self.newTab(privateMode:false,url:nil)}
                self.notice("Đã hoàn tất yêu cầu xóa dữ liệu website thông thường.")
            }
        });present(alert,animated:true)
    }
    private func presentSheet(_ sheet:UIAlertController,from source:UIView){sheet.popoverPresentationController?.sourceView=source;sheet.popoverPresentationController?.sourceRect=source.bounds;present(sheet,animated:true)}
    private func notice(_ message:String){guard presentedViewController==nil else{return};let alert=UIAlertController(title:"OmniPlay",message:message,preferredStyle:.alert);alert.addAction(UIAlertAction(title:"OK",style:.default));present(alert,animated:true)}

    func webView(_ webView:WKWebView,decidePolicyFor action:WKNavigationAction,decisionHandler:@escaping(WKNavigationActionPolicy)->Void){
        guard let url=action.request.url else{decisionHandler(.cancel);return}
        if allowed(url){decisionHandler(action.shouldPerformDownload ? .download:.allow);return}
        if action.shouldPerformDownload,url.scheme=="blob",let inner=URL(string:String(url.absoluteString.dropFirst(5))),let current=webView.url,inner.scheme==current.scheme,inner.host==current.host,inner.port==current.port{decisionHandler(.download);return}
        if url.absoluteString=="about:blank"{decisionHandler(.allow);return}
        decisionHandler(.cancel)
        if action.navigationType == .linkActivated,["mailto","tel"].contains(url.scheme?.lowercased() ?? ""){let alert=UIAlertController(title:"Mở ứng dụng khác?",message:url.scheme,preferredStyle:.alert);alert.addAction(UIAlertAction(title:"Hủy",style:.cancel));alert.addAction(UIAlertAction(title:"Mở",style:.default){_ in UIApplication.shared.open(url)});if presentedViewController==nil{present(alert,animated:true)}}
    }
    func webView(_ webView:WKWebView,decidePolicyFor response:WKNavigationResponse,decisionHandler:@escaping(WKNavigationResponsePolicy)->Void){decisionHandler(response.canShowMIMEType ? .allow:.download)}
    func webView(_ webView:WKWebView,didFinish navigation:WKNavigation!){if let tab=tabs.first(where:{$0.web===webView}),!tab.isPrivate,let url=webView.url{store.visit(url,title:webView.title ?? url.host ?? "Website")};saveSession();if current?.web===webView{refresh()}}
    func webView(_ webView:WKWebView,didFailProvisionalNavigation navigation:WKNavigation!,withError error:Error){if (error as NSError).code != NSURLErrorCancelled{notice(error.localizedDescription)}}
    func webViewWebContentProcessDidTerminate(_ webView:WKWebView){if current?.web===webView{notice("Thẻ đã dừng. Bạn có thể tải lại để tiếp tục.")}}
    func webView(_ webView:WKWebView,createWebViewWith configuration:WKWebViewConfiguration,for action:WKNavigationAction,windowFeatures:WKWindowFeatures)->WKWebView?{guard let url=action.request.url,allowed(url),let source=tabs.first(where:{$0.web===webView})else{return nil};let alert=UIAlertController(title:"Mở thẻ mới?",message:url.host,preferredStyle:.alert);alert.addAction(UIAlertAction(title:"Hủy",style:.cancel));alert.addAction(UIAlertAction(title:"Mở",style:.default){[weak self]_ in self?.newTab(privateMode:source.isPrivate,url:url)});if presentedViewController==nil{present(alert,animated:true)};return nil}
    func webView(_ webView:WKWebView,requestMediaCapturePermissionFor origin:WKSecurityOrigin,initiatedByFrame frame:WKFrameInfo,type:WKMediaCaptureType,decisionHandler:@escaping(WKPermissionDecision)->Void){guard current?.web===webView,view.window != nil,origin.protocol=="https",frame.isMainFrame else{decisionHandler(.deny);return};decisionHandler(.prompt)}
    func webView(_ webView:WKWebView,runJavaScriptAlertPanelWithMessage message:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping()->Void){guard current?.web===webView,presentedViewController==nil else{completionHandler();return};let alert=UIAlertController(title:frame.securityOrigin.host,message:String(message.prefix(2000)),preferredStyle:.alert);alert.addAction(UIAlertAction(title:"OK",style:.default){_ in completionHandler()});present(alert,animated:true)}
    func webView(_ webView:WKWebView,runJavaScriptConfirmPanelWithMessage message:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping(Bool)->Void){guard current?.web===webView,presentedViewController==nil else{completionHandler(false);return};let alert=UIAlertController(title:frame.securityOrigin.host,message:String(message.prefix(2000)),preferredStyle:.alert);alert.addAction(UIAlertAction(title:"Hủy",style:.cancel){_ in completionHandler(false)});alert.addAction(UIAlertAction(title:"OK",style:.default){_ in completionHandler(true)});present(alert,animated:true)}
    func webView(_ webView:WKWebView,runJavaScriptTextInputPanelWithPrompt prompt:String,defaultText:String?,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping(String?)->Void){guard current?.web===webView,presentedViewController==nil else{completionHandler(nil);return};let alert=UIAlertController(title:frame.securityOrigin.host,message:String(prompt.prefix(2000)),preferredStyle:.alert);alert.addTextField{$0.text=String((defaultText ?? "").prefix(4000))};alert.addAction(UIAlertAction(title:"Hủy",style:.cancel){_ in completionHandler(nil)});alert.addAction(UIAlertAction(title:"OK",style:.default){[weak alert]_ in completionHandler(alert?.textFields?.first?.text)});present(alert,animated:true)}

    func webView(_ webView:WKWebView,navigationAction:WKNavigationAction,didBecome download:WKDownload){download.delegate=self}
    func webView(_ webView:WKWebView,navigationResponse:WKNavigationResponse,didBecome download:WKDownload){download.delegate=self}
    func download(_ download:WKDownload,decideDestinationUsing response:URLResponse,suggestedFilename:String,completionHandler:@escaping(URL?)->Void){
        guard !removingWebsiteData,downloads.count+retiringDownloads.count<2,response.expectedContentLength<=maximumDownload,presentedViewController==nil else{completionHandler(nil);return}
        let safe=String(suggestedFilename.unicodeScalars.filter{!CharacterSet.controlCharacters.contains($0)&&$0 != "/"&&$0 != "\\"&&$0 != ":"}.map(String.init).joined().prefix(120))
        guard !safe.isEmpty,safe != ".",safe != ".." else{completionHandler(nil);return}
        let alert=UIAlertController(title:"Tải tệp?",message:"\(response.url?.host ?? "Website")\n\(safe)\nGiới hạn 2 GiB. Tệp không được chạy hoặc nhập vào thư viện tự động.",preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:"Hủy",style:.cancel){_ in completionHandler(nil)})
        alert.addAction(UIAlertAction(title:"Tải",style:.default){[weak self]_ in
            guard let self=self,!self.removingWebsiteData,self.downloads.count+self.retiringDownloads.count<2 else{completionHandler(nil);return}
            let directory=FileManager.default.temporaryDirectory.appendingPathComponent("OmniPlayDownloads",isDirectory:true).appendingPathComponent(UUID().uuidString,isDirectory:true),target=directory.appendingPathComponent(safe)
            do{try FileManager.default.createDirectory(at:directory,withIntermediateDirectories:true,attributes:[.protectionKey:FileProtectionType.completeUntilFirstUserAuthentication]);self.downloads[ObjectIdentifier(download)]=Download(value:download,url:target,began:ProcessInfo.processInfo.systemUptime);self.monitorDownloads();completionHandler(target)}catch{self.removeDownload(target);completionHandler(nil)}
        });present(alert,animated:true)
    }
    private func monitorDownloads(){guard downloadTimer==nil else{return};downloadTimer=Timer.scheduledTimer(withTimeInterval:1,repeats:true){[weak self]_ in guard let self=self else{return};for(key,value)in Array(self.downloads){let bytes=(try? FileManager.default.attributesOfItem(atPath:value.url.path)[.size] as? NSNumber)?.int64Value ?? 0;if value.value.progress.completedUnitCount>self.maximumDownload||bytes>self.maximumDownload||ProcessInfo.processInfo.systemUptime-value.began>1200{self.retireDownload(key,value:value)}};if self.downloads.isEmpty{self.downloadTimer?.invalidate();self.downloadTimer=nil}}}
    func downloadDidFinish(_ download:WKDownload){guard let value=downloads.removeValue(forKey:ObjectIdentifier(download))else{return};let bytes=(try? FileManager.default.attributesOfItem(atPath:value.url.path)[.size] as? NSNumber)?.int64Value;guard let bytes=bytes,bytes>=0,bytes<=maximumDownload,ProcessInfo.processInfo.systemUptime-value.began<=1200 else{removeDownload(value.url);notice("Tệp tải vượt giới hạn kích thước hoặc thời gian.");return};let controller=UIActivityViewController(activityItems:[value.url],applicationActivities:nil);controller.completionWithItemsHandler={[weak self]_,_,_,_ in self?.removeDownload(value.url)};controller.popoverPresentationController?.sourceView=menu;controller.popoverPresentationController?.sourceRect=menu.bounds;if presentedViewController==nil{present(controller,animated:true)}else{removeDownload(value.url)}}
    func download(_ download:WKDownload,didFailWithError error:Error,resumeData:Data?){if let value=downloads.removeValue(forKey:ObjectIdentifier(download)){removeDownload(value.url)};notice("Tải tệp chưa hoàn tất. \(error.localizedDescription)")}
    private func retireDownload(_ key:ObjectIdentifier,value:Download){
        guard downloads.removeValue(forKey:key) != nil else{return};retiringDownloads[key]=value
        // Keep admission and the destination until WebKit confirms cancellation.
        value.value.cancel{[weak self]_ in DispatchQueue.main.async{guard let self=self,self.retiringDownloads.removeValue(forKey:key) != nil else{return};self.removeDownload(value.url)}}
    }
    private func cancelDownloads(){for(key,value)in Array(downloads){retireDownload(key,value:value)};downloadTimer?.invalidate();downloadTimer=nil}
    private func removeDownload(_ url:URL){let root=FileManager.default.temporaryDirectory.appendingPathComponent("OmniPlayDownloads").standardizedFileURL,parent=url.deletingLastPathComponent().standardizedFileURL;guard parent.deletingLastPathComponent()==root,UUID(uuidString:parent.lastPathComponent) != nil else{return};try? FileManager.default.removeItem(at:url);try? FileManager.default.removeItem(at:parent)}
}
