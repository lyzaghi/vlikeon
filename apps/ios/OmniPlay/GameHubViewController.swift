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
    override var prefersStatusBarHidden:Bool{fullScreenGame}
    override var prefersHomeIndicatorAutoHidden:Bool{fullScreenGame}
    var openWebsite: ((URL)->Void)?

    init(core:NativeCore){self.core=core;super.init(nibName:nil,bundle:nil)}
    required init?(coder:NSCoder){fatalError("Use the profile initializer")}
    override func viewDidLoad(){
        super.viewDidLoad();view.backgroundColor=UIColor(red:0.05,green:0.08,blue:0.065,alpha:1)
        status.text="Đang chuẩn bị thư viện…";status.textColor = .white;status.numberOfLines=0;status.textAlignment = .center;status.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(status)
        NSLayoutConstraint.activate([status.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:24),status.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-24),status.centerYAnchor.constraint(equalTo:view.centerYAnchor)])
        NotificationCenter.default.addObserver(self,selector:#selector(willResign),name:UIApplication.willResignActiveNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(didBackground),name:UIApplication.didEnterBackgroundNotification,object:nil)
        NotificationCenter.default.addObserver(self,selector:#selector(didActivate),name:UIApplication.didBecomeActiveNotification,object:nil)
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
        bridge.setGameFullscreen={ [weak self]enabled in guard let self=self else{return};self.fullScreenGame=enabled;self.tabBarController?.tabBar.isHidden=enabled;self.setNeedsStatusBarAppearanceUpdate();self.setNeedsUpdateOfHomeIndicatorAutoHidden();self.view.setNeedsLayout() }
        configuration.userContentController.addScriptMessageHandler(bridge,contentWorld:.page,name:"omniplay")
        let webView=WKWebView(frame:.zero,configuration:configuration);webView.isInspectable=false;webView.navigationDelegate=self;webView.uiDelegate=self;webView.scrollView.contentInsetAdjustmentBehavior = .never
        bridge.webView=webView;self.webView=webView;webView.translatesAutoresizingMaskIntoConstraints=false;view.addSubview(webView)
        NSLayoutConstraint.activate([webView.leadingAnchor.constraint(equalTo:view.leadingAnchor),webView.trailingAnchor.constraint(equalTo:view.trailingAnchor),webView.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor),webView.bottomAnchor.constraint(equalTo:view.safeAreaLayoutGuide.bottomAnchor)])
        webView.load(URLRequest(url:url))
    }
    private func showFailure(_ error:Error){bridge?.resetDocument();core.setForeground(false);fullScreenGame=false;tabBarController?.tabBar.isHidden=false;setNeedsStatusBarAppearanceUpdate();status.text=error.localizedDescription;status.isHidden=false;webView?.isHidden=true;showRetry()}
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
    @objc private func willResign(){suspend()}
    @objc private func didBackground(){suspend()}
    @objc private func didActivate(){if !pendingSuspend&&hubVisible&&UIApplication.shared.applicationState == .active&&webView?.isHidden != true{core.setForeground(true)}}
    override func viewDidAppear(_ animated:Bool){super.viewDidAppear(animated);hubVisible=true;didActivate()}
    func suspend(completion:@escaping()->Void = {}){
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
