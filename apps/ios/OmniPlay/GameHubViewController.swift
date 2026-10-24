import UIKit
import WebKit
import AVFAudio

@MainActor
final class GameHubViewController: UIViewController, WKNavigationDelegate, WKUIDelegate, UITextFieldDelegate {
    private let core: NativeCore
    private var server: AssetServer?
    private var bridge: HubBridge?
    private(set) var webView: WKWebView?
    private let status=UILabel()
    private var backgroundTask:UIBackgroundTaskIdentifier = .invalid
    private var pendingSuspend=false
    private var suspensionID:UUID?
    private var suspensionExpiry:DispatchWorkItem?
    private var retryButton:UIButton?
    private var suspendWaiters:[()->Void]=[]
    private var fullScreenGame=false
    private var hubVisible=false
    private let pairingButton=UIButton(type:.system)
    private var pairingButtonHeight:NSLayoutConstraint?
    private var pairingFlow:NativeDocumentLifetime?
    private weak var pairingAlert:UIAlertController?
    private var pairingExpiry:DispatchWorkItem?
    private var pairingStartupChecked=false
    private var pairingStartupOwner:NativeDocumentLifetime?
    override var prefersStatusBarHidden:Bool{fullScreenGame}
    override var prefersHomeIndicatorAutoHidden:Bool{fullScreenGame}
    var openWebsite: ((URL)->Void)?

    init(core:NativeCore){self.core=core;super.init(nibName:nil,bundle:nil)}
    required init?(coder:NSCoder){fatalError("Use the profile initializer")}
    override func viewDidLoad(){
        super.viewDidLoad();view.backgroundColor=UIColor(red:0.05,green:0.08,blue:0.065,alpha:1)
        pairingButton.setTitle("Danh tính Remote Play",for:.normal);pairingButton.accessibilityLabel="Thiết lập danh tính Remote Play trên thiết bị này"
        pairingButton.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(pairingButton)
        pairingButtonHeight=pairingButton.heightAnchor.constraint(equalToConstant:44)
        NSLayoutConstraint.activate([pairingButton.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:16),pairingButton.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-16),pairingButton.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),pairingButtonHeight!])
        pairingButton.menu=UIMenu(children:[UIAction(title:"Thiết lập danh tính"){[weak self]_ in self?.beginPairingIdentity()},UIAction(title:"Trạng thái và phục hồi"){[weak self]_ in self?.beginPairingRecovery()},UIAction(title:"Ghép đôi với host"){[weak self]_ in self?.beginClientPairing()}]);pairingButton.showsMenuAsPrimaryAction=true
        status.text="Đang chuẩn bị thư viện…";status.textColor = .white;status.numberOfLines=0;status.textAlignment = .center;status.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(status)
        NSLayoutConstraint.activate([status.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:24),status.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-24),status.centerYAnchor.constraint(equalTo:view.centerYAnchor)])
        NotificationCenter.default.addObserver(self,selector:#selector(willResign),name:UIApplication.willResignActiveNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(didBackground),name:UIApplication.didEnterBackgroundNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(didActivate),name:UIApplication.didBecomeActiveNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(cancelPairingIdentity),name:UIApplication.protectedDataWillBecomeUnavailableNotification,object:nil)
        core.open{[weak self] result in guard let self=self else{return};switch result{case .success:self.startHub();case .failure(let error):self.showFailure(error)}}
    }
    private func startHub(){
        do {
            guard let resources=Bundle.main.url(forResource:"GameHub",withExtension:nil)else{throw OmniFailure.message("The packaged Game Hub resources are missing.")}
            self.server?.stop()
            let server=try AssetServer(directory:resources);self.server=server
            server.start{[weak self] result in guard let self=self,self.server === server else{server.stop();return};switch result{case .success(let url):self.loadHub(url,server:server);case .failure(let error):self.showFailure(error)}}
        }catch{showFailure(error)}
    }
    private func loadHub(_ url:URL,server:AssetServer){
        let configuration=WKWebViewConfiguration();configuration.websiteDataStore = .nonPersistent();configuration.allowsInlineMediaPlayback=true
        configuration.mediaTypesRequiringUserActionForPlayback = .all
        let bridge=HubBridge(core:core,server:server,presenter:self);self.bridge=bridge
        bridge.setGameFullscreen={ [weak self]enabled in guard let self=self else{return};self.fullScreenGame=enabled;self.pairingButton.isHidden=enabled;self.pairingButtonHeight?.constant=enabled ? 0:44;if enabled{self.cancelPairingIdentity()};self.tabBarController?.tabBar.isHidden=enabled;self.setNeedsStatusBarAppearanceUpdate();self.setNeedsUpdateOfHomeIndicatorAutoHidden();self.view.setNeedsLayout() }
        configuration.userContentController.addScriptMessageHandler(bridge,contentWorld:.page,name:"omniplay")
        let webView=WKWebView(frame:.zero,configuration:configuration);webView.isInspectable=false;webView.navigationDelegate=self;webView.uiDelegate=self;webView.scrollView.contentInsetAdjustmentBehavior = .never
        bridge.webView=webView;self.webView=webView;webView.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(webView)
        NSLayoutConstraint.activate([webView.leadingAnchor.constraint(equalTo:view.leadingAnchor),webView.trailingAnchor.constraint(equalTo:view.trailingAnchor),webView.topAnchor.constraint(equalTo:pairingButton.bottomAnchor),webView.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor)])
        webView.load(URLRequest(url:url))
    }
    private func showFailure(_ error:Error){cancelPairingIdentity();bridge?.resetDocument();core.setForeground(false);fullScreenGame=false;pairingButton.isHidden=false;pairingButtonHeight?.constant=44;tabBarController?.tabBar.isHidden=false;setNeedsStatusBarAppearanceUpdate();status.text=error.localizedDescription;status.isHidden=false;webView?.isHidden=true;showRetry()}
    private func showRetry(){
        retryButton?.removeFromSuperview()
        let retry=UIButton(type:.system);retryButton=retry;retry.setTitle("Mở lại Thư viện",for:.normal)
        retry.addAction(UIAction{[weak self] _ in
            guard let self=self else{return};self.retryButton?.removeFromSuperview();self.retryButton=nil
            if let web=self.webView{web.isHidden=false;web.reload()}else{self.core.open{[weak self] result in guard let self=self else{return};switch result{case .success:self.startHub();case .failure(let error):self.showFailure(error)}}}
        },for:.touchUpInside)
        retry.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(retry);NSLayoutConstraint.activate([retry.centerXAnchor.constraint(equalTo:view.centerXAnchor),retry.topAnchor.constraint(equalTo:status.bottomAnchor,constant:20)])
    }
    func webView(_ webView:WKWebView,didStartProvisionalNavigation navigation:WKNavigation!){bridge?.resetDocument()}
    func webView(_ webView:WKWebView,didFinish navigation:WKNavigation!){retryButton?.removeFromSuperview();retryButton=nil;status.isHidden=true;didActivate()}
    func webView(_ webView:WKWebView,didFailProvisionalNavigation navigation:WKNavigation!,withError error:Error){if (error as NSError).code != NSURLErrorCancelled{showFailure(error)}}
    func webView(_ webView:WKWebView,didFail navigation:WKNavigation!,withError error:Error){if (error as NSError).code != NSURLErrorCancelled{showFailure(error)}}
    func webViewWebContentProcessDidTerminate(_ webView:WKWebView){core.suspend();showFailure(OmniFailure.message("Phiên WebKit đã dừng. Điểm đã lưu vẫn ở trên thiết bị. Mở lại Thư viện để tiếp tục."))}
    func webView(_ webView:WKWebView,decidePolicyFor navigationAction:WKNavigationAction,decisionHandler:@escaping(WKNavigationActionPolicy)->Void){
        guard let url=navigationAction.request.url else{decisionHandler(.cancel);return}
        if server?.owns(url)==true{decisionHandler(.allow);return}
        decisionHandler(.cancel)
        if navigationAction.navigationType == .linkActivated,["https","http"].contains(url.scheme?.lowercased() ?? ""){openWebsite?(url)}
    }
    func webView(_ webView:WKWebView,createWebViewWith configuration:WKWebViewConfiguration,for navigationAction:WKNavigationAction,windowFeatures:WKWindowFeatures)->WKWebView?{if let url=navigationAction.request.url,["https","http"].contains(url.scheme?.lowercased() ?? ""){openWebsite?(url)};return nil}
    func webView(_ webView:WKWebView,runJavaScriptAlertPanelWithMessage message:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping()->Void){let alert=UIAlertController(title:"OmniPlay",message:String(message.prefix(2000)),preferredStyle:.alert);alert.addAction(UIAlertAction(title:"OK",style:.default){_ in completionHandler()});present(alert,animated:true)}
    func webView(_ webView:WKWebView,runJavaScriptConfirmPanelWithMessage message:String,initiatedByFrame frame:WKFrameInfo,completionHandler:@escaping(Bool)->Void){let alert=UIAlertController(title:"OmniPlay",message:String(message.prefix(2000)),preferredStyle:.alert);alert.addAction(UIAlertAction(title:"Hủy",style:.cancel){_ in completionHandler(false)});alert.addAction(UIAlertAction(title:"Tiếp tục",style:.default){_ in completionHandler(true)});present(alert,animated:true)}
    private func beginClientPairing(){
        guard pairingFlow==nil,presentedViewController==nil,hubVisible,!fullScreenGame,!pendingSuspend,UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{return}
        let owner=NativeDocumentLifetime();pairingFlow=owner;pairingButton.isEnabled=false
        core.clientPairingSetup(owner:owner){[weak self] result in
            guard let self=self,self.pairingFlow === owner,owner.active else{return}
            guard self.peerReviewAvailable(owner) else{self.cancelPairingIdentity();return}
            switch result{
            case .failure(let error):self.cancelPairingIdentity();self.pairingNotice(error.localizedDescription)
            case .success(let setup):
                let sheet=UIAlertController(title:"Chọn dịch vụ ghép đôi",message:"Chỉ chọn relay được cấu hình trong ứng dụng. Phiên chuẩn bị hết hạn sau 30 giây.",preferredStyle:.actionSheet);self.pairingAlert=sheet
                for origin in setup.relayOrigins{
                    sheet.addAction(UIAlertAction(title:origin,style:.default){[weak self,weak sheet]_ in sheet?.dismiss(animated:false){[weak self] in self?.showClientInvitation(setup,origin:origin,owner:owner)}})
                }
                sheet.addAction(UIAlertAction(title:"Đóng",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
                sheet.popoverPresentationController?.sourceView=self.pairingButton;sheet.popoverPresentationController?.sourceRect=self.pairingButton.bounds
                let expiry=DispatchWorkItem{[weak self] in guard let self=self,self.pairingFlow === owner else{return};self.cancelPairingIdentity()}
                self.pairingExpiry=expiry;DispatchQueue.main.asyncAfter(deadline:setup.expires,execute:expiry);self.present(sheet,animated:true)
            }
        }
    }
    private func showClientInvitation(_ setup:NativeClientSetup,origin:String,owner:NativeDocumentLifetime){
        guard peerReviewAvailable(owner),DispatchTime.now()<setup.expires else{cancelPairingIdentity();return}
        let alert=UIAlertController(title:"Nhập lời mời của host",message:"Dịch vụ: \(origin)\nNhập mã mời, khóa host đã xác minh độc lập và nhãn thiết bị. Không lấy sự tin cậy của khóa chỉ từ mã mời.",preferredStyle:.alert);pairingAlert=alert
        alert.addTextField{[weak self] field in field.delegate=self;field.tag=8192;field.placeholder="Mã mời";field.isSecureTextEntry=true;field.autocorrectionType = .no;field.autocapitalizationType = .none;field.spellCheckingType = .no;field.textContentType = nil}
        alert.addTextField{[weak self] field in field.delegate=self;field.tag=43;field.placeholder="Khóa công khai host đã xác minh";field.autocorrectionType = .no;field.autocapitalizationType = .none;field.spellCheckingType = .no}
        alert.addTextField{[weak self] field in field.delegate=self;field.tag=64;field.placeholder="Nhãn thiết bị này";field.autocorrectionType = .no}
        alert.addAction(UIAlertAction(title:"Đóng",style:.cancel){[weak self,weak alert]_ in alert?.textFields?.forEach{$0.text=nil};self?.cancelPairingIdentity()})
        alert.addAction(UIAlertAction(title:"Đã xác minh khóa host, chuẩn bị",style:.default){[weak self,weak alert]_ in
            guard let self=self,self.pairingFlow === owner,owner.active,DispatchTime.now()<setup.expires,
                  let fields=alert?.textFields,fields.count==3 else{self?.cancelPairingIdentity();return}
            let code=fields[0].text ?? "",host=fields[1].text ?? "",label=fields[2].text ?? "";fields.forEach{$0.text=nil}
            guard !code.isEmpty,code.utf8.count<=8192,host.utf8.count==43,!label.isEmpty,label.utf8.count<=64 else{self.cancelPairingIdentity();self.pairingNotice("Thông tin không hợp lệ. Mở lại ghép đôi và nhập mã, khóa host, nhãn thiết bị trong giới hạn.");return}
            alert?.dismiss(animated:false){[weak self] in
                guard let self=self,self.peerReviewAvailable(owner) else{self?.cancelPairingIdentity();return}
                self.pairingExpiry?.cancel();self.pairingExpiry=nil
                self.core.beginPairingClient(setup,code:code,host:host,audience:origin,label:label,owner:owner){[weak self] result in
                    guard let self=self,self.pairingFlow === owner,owner.active else{return}
                    guard self.peerReviewAvailable(owner) else{self.cancelPairingIdentity();return}
                    do{
                        let value=try result.get()
                        guard let ticket=value["ticket"] as? String,let comparison=value["comparison_code"] as? String,let expiry=value["expires_at_ms"] as? NSNumber else{throw OmniFailure.message("Kết quả chuẩn bị không hợp lệ.")}
                        let remaining=min(300_000,expiry.doubleValue-Date().timeIntervalSince1970*1000)
                        guard remaining>0 else{throw OmniFailure.message("Lời mời đã hết hạn.")}
                        let expiryWork=DispatchWorkItem{[weak self] in guard let self=self,self.pairingFlow === owner else{return};self.cancelPairingIdentity()}
                        self.pairingExpiry=expiryWork;DispatchQueue.main.asyncAfter(deadline:.now() + .milliseconds(Int(remaining)),execute:expiryWork)
                        self.showClientJoin(ticket:ticket,comparison:comparison,owner:owner)
                    }catch{self.cancelPairingIdentity();self.pairingNotice("Chưa xác nhận được chuẩn bị ghép đôi. Mở lại trạng thái trước khi thử lại.")}
                }
            }
        })
        present(alert,animated:true)
    }
    private func showClientJoin(ticket:String,comparison:String,owner:NativeDocumentLifetime){
        guard peerReviewAvailable(owner) else{cancelPairingIdentity();return}
        let alert=UIAlertController(title:"Gửi yêu cầu ghép đôi?",message:"Mã đối chiếu: \(comparison)\nGửi yêu cầu rồi so sánh mã trên host. Gửi yêu cầu chưa cấp quyền peer hoặc điều khiển.",preferredStyle:.alert);pairingAlert=alert
        alert.addAction(UIAlertAction(title:"Đóng",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
        alert.addAction(UIAlertAction(title:"Gửi yêu cầu",style:.default){[weak self,weak alert]_ in
            alert?.dismiss(animated:false){[weak self] in
                guard let self=self,self.peerReviewAvailable(owner) else{self?.cancelPairingIdentity();return}
                do{
                    var input=try JSONSerialization.data(withJSONObject:["ticket":ticket])
                    self.core.pairingClient(.join,input:&input,owner:owner){[weak self] result in
                        guard let self=self,self.pairingFlow === owner,owner.active else{return}
                        switch result{case .success:self.showClientWaiting(ticket:ticket,owner:owner);case .failure:self.cancelPairingIdentity();self.pairingNotice("Chưa xác nhận được kết quả gửi. Mở lại trạng thái trước khi tiếp tục.")}
                    }
                }catch{self.cancelPairingIdentity();self.pairingNotice("Không gửi được yêu cầu. Hãy kiểm tra trạng thái.")}
            }
        })
        present(alert,animated:true)
    }
    func textField(_ textField:UITextField,shouldChangeCharactersIn range:NSRange,replacementString string:String)->Bool{
        guard [43,64,8192].contains(textField.tag) else{return true}
        let current=textField.text ?? ""
        guard let selection=Range(range,in:current) else{return false}
        return current.replacingCharacters(in:selection,with:string).utf8.count<=textField.tag
    }
    private func showClientWaiting(ticket:String,owner:NativeDocumentLifetime){
        guard peerReviewAvailable(owner) else{cancelPairingIdentity();return}
        let alert=UIAlertController(title:"Kiểm tra duyệt từ host",message:"Chưa xác nhận được grant hiện tại tại đây. Đối chiếu mã và duyệt trên host, rồi kiểm tra lại hoặc hủy cục bộ. Chỉ kiểm tra khi bạn yêu cầu.",preferredStyle:.alert);pairingAlert=alert
        alert.addAction(UIAlertAction(title:"Hủy cục bộ",style:.destructive){[weak self,weak alert]_ in alert?.dismiss(animated:false){[weak self] in self?.cancelPendingPeer(ticket:ticket,owner:owner)}})
        alert.addAction(UIAlertAction(title:"Kiểm tra host duyệt",style:.default){[weak self,weak alert]_ in alert?.dismiss(animated:false){[weak self] in self?.showPeerApproval(ticket:ticket,owner:owner)}})
        present(alert,animated:true)
    }
    private func beginPairingIdentity(){
        guard pairingFlow==nil,presentedViewController==nil,hubVisible,!fullScreenGame,!pendingSuspend,
              UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{return}
        let owner=NativeDocumentLifetime();pairingFlow=owner;pairingButton.isEnabled=false
        core.reviewPairingIdentity(owner:owner){[weak self] result in
            guard let self=self,self.pairingFlow === owner,owner.active else{return}
            guard self.hubVisible,UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable,self.presentedViewController==nil else{self.cancelPairingIdentity();return}
            switch result {
            case .failure(let error):self.cancelPairingIdentity();self.pairingNotice(error.localizedDescription)
            case .success(let review):
                guard DispatchTime.now()<review.expires else{self.cancelPairingIdentity();return}
                let message="Kích hoạt danh tính cục bộ trên thiết bị này?\n\nThiết bị: \(review.device)\nThế hệ: \(review.generation)\nKhóa công khai:\n\(review.publicKey)\n\nBước này chưa cho phép thiết bị khác kết nối hoặc điều khiển."
                let alert=UIAlertController(title:"Xác nhận danh tính Remote Play",message:message,preferredStyle:.alert);self.pairingAlert=alert
                alert.addAction(UIAlertAction(title:"Hủy",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
                alert.addAction(UIAlertAction(title:"Kích hoạt danh tính",style:.default){[weak self]_ in
                    guard let self=self,self.pairingFlow === owner,owner.active,DispatchTime.now()<review.expires,
                          self.hubVisible,UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{self?.cancelPairingIdentity();return}
                    self.core.confirmPairingIdentity(review,owner:owner){[weak self] result in
                        guard let self=self,self.pairingFlow === owner,owner.active else{return}
                        self.cancelPairingIdentity()
                        switch result{case .success:self.pairingNotice("Đã kích hoạt danh tính cục bộ. Chưa có thiết bị từ xa nào được chấp thuận.");case .failure(let error):self.pairingNotice(error.localizedDescription+"\nHãy mở lại trạng thái trước khi tiếp tục; không tự động thử lại.")}
                    }
                })
                let expiry=DispatchWorkItem{[weak self] in guard let self=self,self.pairingFlow === owner else{return};self.cancelPairingIdentity()}
                self.pairingExpiry=expiry;DispatchQueue.main.asyncAfter(deadline:review.expires,execute:expiry)
                self.present(alert,animated:true)
            }
        }
    }
    private func beginPairingRecovery(){
        guard pairingFlow==nil,presentedViewController==nil,hubVisible,!fullScreenGame,!pendingSuspend,
              UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{return}
        let owner=NativeDocumentLifetime();pairingFlow=owner;pairingButton.isEnabled=false
        core.pairingRecoveryStatus(owner:owner){[weak self] result in
            guard let self=self,self.pairingFlow === owner,owner.active else{return}
            guard self.presentedViewController==nil,self.hubVisible,UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{self.cancelPairingIdentity();return}
            switch result{
            case .failure(let error):self.cancelPairingIdentity();self.pairingNotice(error.localizedDescription)
            case .success(let snapshot):
                guard DispatchTime.now()<snapshot.expires else{self.cancelPairingIdentity();return}
                self.applyPairingStatusBadge(snapshot)
                if snapshot.records.isEmpty && snapshot.pendingClientTicket==nil{self.cancelPairingIdentity();self.pairingNotice("Thiết bị này chưa có danh tính hoặc cặp Remote Play.");return}
                let sheet=UIAlertController(title:"Trạng thái Remote Play",message:"Chọn bản ghi để xem chi tiết. Đọc trạng thái không làm thay đổi khóa hoặc kết nối.",preferredStyle:.actionSheet)
                self.pairingAlert=sheet
                if let ticket=snapshot.pendingClientTicket{
                    sheet.addAction(UIAlertAction(title:"Duyệt host của phiên đang chờ",style:.default){[weak self,weak sheet]_ in
                        sheet?.dismiss(animated:false){[weak self] in self?.showPeerApproval(ticket:ticket,owner:owner)}
                    })
                    sheet.addAction(UIAlertAction(title:"Hủy ghép đôi đang chờ tại thiết bị",style:.destructive){[weak self,weak sheet]_ in
                        sheet?.dismiss(animated:false){[weak self] in self?.cancelPendingPeer(ticket:ticket,owner:owner)}
                    })
                }
                for record in snapshot.records{
                    let title="\(record.isIdentity ? "Danh tính":"Cặp thiết bị") · \(record.generation)"
                    sheet.addAction(UIAlertAction(title:title,style:.default){[weak self,weak sheet]_ in
                        sheet?.dismiss(animated:false){[weak self] in self?.showPairingRecovery(record,snapshot:snapshot,owner:owner)}
                    })
                }
                sheet.addAction(UIAlertAction(title:"Đóng",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
                sheet.popoverPresentationController?.sourceView=self.pairingButton;sheet.popoverPresentationController?.sourceRect=self.pairingButton.bounds
                let expiry=DispatchWorkItem{[weak self] in guard let self=self,self.pairingFlow === owner else{return};self.cancelPairingIdentity()}
                self.pairingExpiry=expiry;DispatchQueue.main.asyncAfter(deadline:snapshot.expires,execute:expiry);self.present(sheet,animated:true)
            }
        }
    }
    private func cancelPendingPeer(ticket:String,owner:NativeDocumentLifetime){
        guard peerReviewAvailable(owner) else{cancelPairingIdentity();return}
        let alert=UIAlertController(title:"Hủy phiên ghép đôi tại thiết bị?",message:"Bỏ phiên trao đổi đang chờ trên thiết bị này. Thao tác không thu hồi cặp đã được dịch vụ duyệt và không xóa ghép đôi đã lưu.",preferredStyle:.alert)
        pairingAlert=alert
        alert.addAction(UIAlertAction(title:"Đóng",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
        alert.addAction(UIAlertAction(title:"Hủy cục bộ",style:.destructive){[weak self]_ in
            guard let self=self,self.pairingFlow === owner,owner.active,self.hubVisible,!self.pendingSuspend,UIApplication.shared.applicationState == .active else{self?.cancelPairingIdentity();return}
            do{
                var input=try JSONSerialization.data(withJSONObject:["ticket":ticket])
                self.core.pairingClient(.cancel,input:&input,owner:owner){[weak self] result in
                    guard let self=self,self.pairingFlow === owner,owner.active else{return};self.cancelPairingIdentity()
                    switch result{case .success:self.pairingNotice("Đã hủy phiên chờ tại thiết bị. Không thu hồi quyền đã duyệt trên dịch vụ.");case .failure:self.pairingNotice("Chưa xác nhận được thao tác hủy. Mở lại trạng thái trước khi tiếp tục.")}
                }
            }catch{self.cancelPairingIdentity();self.pairingNotice("Không gửi được thao tác hủy. Hãy mở lại trạng thái.")}
        })
        present(alert,animated:true)
    }
    private func peerReviewAvailable(_ owner:NativeDocumentLifetime)->Bool{
        pairingFlow === owner && owner.active && hubVisible && !fullScreenGame && !pendingSuspend && presentedViewController==nil && UIApplication.shared.applicationState == .active && UIApplication.shared.isProtectedDataAvailable
    }
    private func showPeerApproval(ticket:String,owner:NativeDocumentLifetime){
        guard peerReviewAvailable(owner) else{cancelPairingIdentity();return}
        core.reviewPairingPeer(ticket:ticket,owner:owner){[weak self] result in
            guard let self=self,self.pairingFlow === owner,owner.active else{return}
            guard self.peerReviewAvailable(owner) else{self.cancelPairingIdentity();return}
            switch result{
            case .failure:self.showClientWaiting(ticket:ticket,owner:owner)
            case .success(let review):
                guard DispatchTime.now()<review.expires else{self.cancelPairingIdentity();return}
                self.pairingExpiry?.cancel()
                let alert=UIAlertController(title:"Đối chiếu host Remote Play",message:"Khóa host:\n\(review.hostPublicKey)\nFingerprint:\n\(review.hostFingerprint)\nMã đối chiếu: \(review.comparisonCode)\nCặp: \(review.pairID)\n\nHãy xác minh host độc lập và đối chiếu mã trên cả hai thiết bị trước khi tiếp tục.",preferredStyle:.alert)
                self.pairingAlert=alert
                alert.addAction(UIAlertAction(title:"Hủy",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
                alert.addAction(UIAlertAction(title:"Đã xác minh host và đối chiếu mã",style:.default){[weak self,weak alert]_ in
                    alert?.dismiss(animated:false){[weak self] in self?.showPeerPermission(review,owner:owner)}
                })
                let expiry=DispatchWorkItem{[weak self] in guard let self=self,self.pairingFlow === owner else{return};self.cancelPairingIdentity()}
                self.pairingExpiry=expiry;DispatchQueue.main.asyncAfter(deadline:review.expires,execute:expiry);self.present(alert,animated:true)
            }
        }
    }
    private func showPeerPermission(_ review:NativePeerApproval,owner:NativeDocumentLifetime){
        guard peerReviewAvailable(owner),DispatchTime.now()<review.expires else{cancelPairingIdentity();return}
        let permission=review.inputAllowed ? "Cho phép điều khiển đầu vào":"Không cho phép điều khiển đầu vào"
        let expiry=DateFormatter.localizedString(from:review.grantExpiresAt,dateStyle:.medium,timeStyle:.short)
        let alert=UIAlertController(title:"Xác nhận quyền của cặp thiết bị",message:"Host: \(review.hostFingerprint)\nCặp: \(review.pairID)\nQuyền: \(permission)\nHạn dùng quyền: \(expiry)\n\nLưu ghép đôi đã xác minh; kết nối chưa được kích hoạt. Phiên xác nhận đóng trong tối đa một phút.",preferredStyle:.alert)
        pairingAlert=alert
        alert.addAction(UIAlertAction(title:"Hủy",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
        alert.addAction(UIAlertAction(title:"Đồng ý quyền và lưu ghép đôi",style:.default){[weak self]_ in
            guard let self=self,self.pairingFlow === owner,owner.active,DispatchTime.now()<review.expires,
                  self.hubVisible,!self.pendingSuspend,UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{self?.cancelPairingIdentity();return}
            self.core.confirmPairingPeer(review,owner:owner){[weak self] result in
                guard let self=self,self.pairingFlow === owner,owner.active else{return};self.cancelPairingIdentity()
                switch result{case .success:self.pairingNotice("Đã lưu ghép đôi. Mở lại trạng thái để kiểm tra; chưa khởi động kết nối.");case .failure:self.pairingNotice("Chưa xác nhận được kết quả. Mở lại trạng thái trước khi tiếp tục.")}
            }
        })
        present(alert,animated:true)
    }
    private func showPairingRecovery(_ record:NativeRecoveryRecord,snapshot:NativeRecoverySnapshot,owner:NativeDocumentLifetime){
        guard pairingFlow === owner,owner.active,DispatchTime.now()<snapshot.expires,presentedViewController==nil,
              hubVisible,UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{cancelPairingIdentity();return}
        let phases=["reserved":"Đã đặt trước","writing":"Ghi khóa chưa xác nhận","material_ready":"Chờ xác nhận danh tính","active":"Đã kích hoạt","revoking":"Đang vô hiệu hóa","retired":"Đã dọn dẹp"]
        let steps=["fence_generation":"Vô hiệu hóa bản ghi trước khi phục hồi","revalidate_before_use":"Cần xác thực lại trước khi sử dụng","retire_writer":"Chờ xác nhận tiến trình ghi khóa đã dừng","retire_local_sessions":"Chờ xác nhận các phiên cục bộ đã dừng","wait_for_pair_cleanup":"Chờ dọn dẹp các cặp thiết bị","revoke_relay":"Chờ xác nhận thu hồi từ relay","remove_material":"Chờ xóa và kiểm tra kho khóa","finalize_retirement":"Hoàn tất trạng thái dọn dẹp"]
        let canAdvance = !snapshot.namespaceProvenanceMissing && (record.phase=="active" || ["fence_generation","finalize_retirement","remove_material"].contains(record.nextStep ?? "") || (record.nextStep=="revoke_relay" && snapshot.relayRetirementAvailable))
        var message="Thiết bị: \(snapshot.device)\nThế hệ: \(record.generation)\nTrạng thái: \(phases[record.phase] ?? record.phase)\nKhóa công khai:\n\(record.publicKey)\n\n\(record.nextStep.flatMap{steps[$0]} ?? "Không còn bước dọn dẹp.")"
        if record.phase=="active" || record.nextStep=="fence_generation"{message += "\n\nVô hiệu hóa là vĩnh viễn đối với thế hệ này. Danh tính bị vô hiệu hóa sẽ vô hiệu hóa các cặp của nó; thao tác này chưa xác nhận kết nối đã dừng hoặc khóa đã được xóa."}
        if record.nextStep=="remove_material"{message += "\n\nXóa vĩnh viễn khóa của đúng thế hệ này khỏi kho khóa thiết bị. Không thể khôi phục bằng thao tác này; hệ thống chỉ cho phép xóa sau khi đã có đủ xác nhận dừng và thu hồi."}
        if record.nextStep=="revoke_relay"{message += snapshot.relayRetirementAvailable ? "\n\nGửi yêu cầu thu hồi cặp này đến dịch vụ kết nối đã cấu hình. Chỉ hoàn tất khi nhận đúng xác nhận của cặp; bước này chưa xóa khóa thiết bị.":"\n\nPhiên bản này chưa cấu hình dịch vụ kết nối để hoàn tất thu hồi."}
        if record.sessionRecoveryRequired{message += "\n\nPhiên kết nối cũ chưa được xác nhận đã dừng. Cần hoàn tất phục hồi trước khi tạo phiên mới; khởi động lại không tự giải phóng phiên cũ."}
        if snapshot.namespaceProvenanceMissing{message += "\n\nHồ sơ này chưa có thông tin xác thực vùng lưu trữ khóa. Cần phục hồi hồ sơ được kiểm tra trước khi thay đổi khóa hoặc trạng thái."}
        let alert=UIAlertController(title:"Chi tiết Remote Play",message:message,preferredStyle:.alert);pairingAlert=alert
        alert.addAction(UIAlertAction(title:"Đóng",style:.cancel){[weak self]_ in self?.cancelPairingIdentity()})
        if canAdvance{
            let title=record.phase=="active" ? "Vô hiệu hóa thế hệ này":(record.nextStep=="revoke_relay" ? "Thu hồi trên dịch vụ":(record.nextStep=="remove_material" ? "Xóa khóa thế hệ này":(record.nextStep=="fence_generation" ? "Vô hiệu hóa thế hệ này":"Hoàn tất trạng thái")))
            alert.addAction(UIAlertAction(title:title,style:record.nextStep=="finalize_retirement" ? .default:.destructive){[weak self]_ in
                guard let self=self,self.pairingFlow === owner,owner.active,DispatchTime.now()<snapshot.expires,self.hubVisible,UIApplication.shared.applicationState == .active,UIApplication.shared.isProtectedDataAvailable else{self?.cancelPairingIdentity();return}
                self.core.advancePairingRecovery(record,snapshot:snapshot,owner:owner){[weak self] result in
                    guard let self=self,self.pairingFlow === owner,owner.active else{return};self.cancelPairingIdentity()
                    switch result{case .success(let outcome):self.pairingNotice(outcome=="committed" ? "Đã cập nhật trạng thái. Hãy mở lại Trạng thái và phục hồi để xem bước tiếp theo.":"Còn bước cần xử lý. Hãy mở lại Trạng thái và phục hồi.");case .failure:self.pairingNotice("Chưa xác nhận được kết quả. Hãy mở lại Trạng thái và phục hồi trước khi tiếp tục.")}
                }
            })
        }
        present(alert,animated:true)
    }
    @objc private func cancelPairingIdentity(){
        guard let owner=pairingFlow else{return};owner.invalidate();pairingFlow=nil
        pairingExpiry?.cancel();pairingExpiry=nil;pairingButton.isEnabled=true
        pairingAlert?.textFields?.forEach{$0.text=nil};pairingAlert?.dismiss(animated:false);pairingAlert=nil;core.cancelPairingIdentityReview()
    }
    private func pairingNotice(_ message:String){
        guard hubVisible,presentedViewController==nil,UIApplication.shared.applicationState == .active else{return}
        let alert=UIAlertController(title:"Remote Play",message:message,preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:"Đóng",style:.cancel));present(alert,animated:true)
    }
    @objc private func willResign(){suspend()}
    @objc private func didBackground(){suspend()}
    @objc private func didActivate(){if !pendingSuspend&&hubVisible&&UIApplication.shared.applicationState == .active&&webView?.isHidden != true{core.setForeground(true);inspectPairingStartup()}}
    private func inspectPairingStartup(){
        guard !pairingStartupChecked,pairingStartupOwner==nil,pairingFlow==nil,status.isHidden,UIApplication.shared.isProtectedDataAvailable else{return}
        pairingStartupChecked=true;let owner=NativeDocumentLifetime();pairingStartupOwner=owner
        core.pairingRecoveryStatus(owner:owner){[weak self] result in
            guard let self=self,self.pairingStartupOwner === owner,owner.active else{return}
            self.pairingStartupOwner=nil;owner.invalidate()
            guard self.hubVisible,UIApplication.shared.applicationState == .active else{return}
            switch result{
            case .success(let snapshot):
                self.applyPairingStatusBadge(snapshot)
            case .failure:
                self.pairingButton.setTitle("Remote Play · Chưa kiểm tra được",for:.normal);self.pairingButton.accessibilityLabel="Chưa kiểm tra được Remote Play. Mở Trạng thái và phục hồi."
            }
        }
    }
    private func applyPairingStatusBadge(_ snapshot:NativeRecoverySnapshot){
        guard DispatchTime.now()<snapshot.expires else{pairingButton.setTitle("Remote Play · Chưa kiểm tra được",for:.normal);pairingButton.accessibilityLabel="Trạng thái Remote Play đã hết hạn. Mở Trạng thái và phục hồi.";return}
        pairingButton.setTitle(snapshot.needsAttention ? "Remote Play · Cần kiểm tra":"Danh tính Remote Play",for:.normal)
        pairingButton.accessibilityLabel=snapshot.needsAttention ? "Remote Play cần kiểm tra. Mở Trạng thái và phục hồi.":"Danh tính Remote Play. Mở thiết lập hoặc trạng thái và phục hồi."
    }
    override func viewDidAppear(_ animated:Bool){super.viewDidAppear(animated);hubVisible=true;didActivate()}
    func suspend(completion:@escaping()->Void = {}){
        if let owner=pairingStartupOwner{owner.invalidate();pairingStartupOwner=nil;pairingStartupChecked=false}
        cancelPairingIdentity()
        core.setForeground(false)
        if pendingSuspend{suspendWaiters.append(completion);return}
        guard let webView=webView else{core.suspend(completion:completion);return}
        pendingSuspend=true
        let id=UUID();suspensionID=id
        suspendWaiters.append(completion)
        if backgroundTask == .invalid {backgroundTask=UIApplication.shared.beginBackgroundTask(withName:"Save Vlikeon progress"){[weak self]in self?.finishSuspend(id)}}
        let finish:()->Void={ [weak self]in self?.finishSuspend(id) }
        webView.callAsyncJavaScript("if (window.omniplayLifecycle) { await window.omniplayLifecycle.suspend(); } return true;",arguments:[:],in:nil,in:.page){_ in finish()}
        let expiry=DispatchWorkItem(block:finish);suspensionExpiry=expiry;DispatchQueue.main.asyncAfter(deadline:.now()+15,execute:expiry)
    }
    private func finishSuspend(_ id:UUID){
        guard pendingSuspend,suspensionID==id else{return}
        pendingSuspend=false;suspensionID=nil;suspensionExpiry?.cancel();suspensionExpiry=nil
        let waiters=suspendWaiters;suspendWaiters.removeAll()
        // End this OS task now, including expiry, even if serialized native I/O
        // is stalled. Its eventual callback must never end a newer OS task.
        let task=backgroundTask;backgroundTask = .invalid
        if task != .invalid{UIApplication.shared.endBackgroundTask(task)}
        core.suspend{[weak self]in waiters.forEach{$0()};self?.didActivate()}
    }
    override func viewWillDisappear(_ animated:Bool){super.viewWillDisappear(animated);hubVisible=false;suspend()}
}
