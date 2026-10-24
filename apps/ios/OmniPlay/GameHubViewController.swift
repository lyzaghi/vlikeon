import UIKit
import WebKit
import AVFAudio

@MainActor
final class GameHubViewController: UIViewController, WKNavigationDelegate, WKUIDelegate {
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
        pairingButton.addAction(UIAction{[weak self]_ in self?.beginPairingIdentity()},for:.touchUpInside)
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
    @objc private func cancelPairingIdentity(){
        guard let owner=pairingFlow else{return};owner.invalidate();pairingFlow=nil
        pairingExpiry?.cancel();pairingExpiry=nil;pairingButton.isEnabled=true
        pairingAlert?.dismiss(animated:false);pairingAlert=nil;core.cancelPairingIdentityReview()
    }
    private func pairingNotice(_ message:String){
        guard hubVisible,presentedViewController==nil,UIApplication.shared.applicationState == .active else{return}
        let alert=UIAlertController(title:"Remote Play",message:message,preferredStyle:.alert)
        alert.addAction(UIAlertAction(title:"Đóng",style:.cancel));present(alert,animated:true)
    }
    @objc private func willResign(){suspend()}
    @objc private func didBackground(){suspend()}
    @objc private func didActivate(){if !pendingSuspend&&hubVisible&&UIApplication.shared.applicationState == .active&&webView?.isHidden != true{core.setForeground(true)}}
    override func viewDidAppear(_ animated:Bool){super.viewDidAppear(animated);hubVisible=true;didActivate()}
    func suspend(completion:@escaping()->Void = {}){
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
