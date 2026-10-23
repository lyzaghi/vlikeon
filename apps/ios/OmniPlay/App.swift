import UIKit

@main
@MainActor
final class AppDelegate:UIResponder,UIApplicationDelegate{
    func application(_ application:UIApplication,didFinishLaunchingWithOptions launchOptions:[UIApplication.LaunchOptionsKey:Any]? = nil)->Bool{BackgroundSync.shared.register();return true}
    func applicationDidEnterBackground(_ application:UIApplication){BackgroundSync.shared.refresh()}
    func application(_ application:UIApplication,configurationForConnecting connectingSceneSession:UISceneSession,options:UIScene.ConnectionOptions)->UISceneConfiguration{let configuration=UISceneConfiguration(name:"OmniPlay",sessionRole:connectingSceneSession.role);configuration.delegateClass=SceneDelegate.self;return configuration}
}
@MainActor
final class SceneDelegate:UIResponder,UIWindowSceneDelegate{
    var window:UIWindow?
    func scene(_ scene:UIScene,willConnectTo session:UISceneSession,options connectionOptions:UIScene.ConnectionOptions){
        guard let scene=scene as? UIWindowScene else{return}
        let window=UIWindow(windowScene:scene);self.window=window
        install(in:window);window.makeKeyAndVisible()
    }
    private func install(in window:UIWindow){
        let hub:UIViewController,browser=BrowserViewController(),tabs=UITabBarController()
        do{
            guard let core=try NativeCore.primary()else{throw OmniFailure.message("Local profile is unavailable.")}
            let library=GameHubViewController(core:core)
            library.openWebsite={ [weak tabs,weak browser]url in browser?.open(url);tabs?.selectedIndex=1 }
            hub=library
        }catch{
            // A profile fault must not leave the whole app on a blank screen or
            // silently create another identity. Ordinary browsing stays usable.
            let fallback=UIViewController();fallback.view.backgroundColor = .systemBackground
            let message=UILabel();message.numberOfLines=0;message.textAlignment = .center;message.text="Chưa mở được thư viện cục bộ. Dữ liệu chưa bị xóa. Kiểm tra bộ nhớ và thử lại. Bạn vẫn có thể duyệt web."
            let retry=UIButton(type:.system);retry.setTitle("Thử mở lại thư viện",for:.normal)
            retry.addAction(UIAction{[weak self,weak window]_ in if let window=window{self?.install(in:window)}},for:.touchUpInside)
            let stack=UIStackView(arrangedSubviews:[message,retry]);stack.axis = .vertical;stack.spacing=20;stack.translatesAutoresizingMaskIntoConstraints=false;fallback.view.addSubview(stack)
            NSLayoutConstraint.activate([stack.leadingAnchor.constraint(equalTo:fallback.view.safeAreaLayoutGuide.leadingAnchor,constant:24),stack.trailingAnchor.constraint(equalTo:fallback.view.safeAreaLayoutGuide.trailingAnchor,constant:-24),stack.centerYAnchor.constraint(equalTo:fallback.view.centerYAnchor)])
            hub=fallback
        }
        hub.tabBarItem=UITabBarItem(title:"Thư viện",image:UIImage(systemName:"gamecontroller"),tag:0)
        browser.tabBarItem=UITabBarItem(title:"Duyệt web",image:UIImage(systemName:"globe"),tag:1)
        tabs.viewControllers=[hub,browser];tabs.tabBar.tintColor=UIColor(red:0.45,green:0.78,blue:0.35,alpha:1)
        window.rootViewController=tabs
    }
    func sceneDidEnterBackground(_ scene:UIScene){BackgroundSync.shared.refresh()}
}
