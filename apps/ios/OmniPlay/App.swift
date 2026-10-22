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
        let core:NativeCore
        do{guard let value = try NativeCore.primary() else{return};core=value}catch{return}
        let hub=GameHubViewController(core:core),browser=BrowserViewController(),tabs=UITabBarController()
        hub.tabBarItem=UITabBarItem(title:"Thư viện",image:UIImage(systemName:"gamecontroller"),tag:0)
        browser.tabBarItem=UITabBarItem(title:"Duyệt web",image:UIImage(systemName:"globe"),tag:1)
        hub.openWebsite={ [weak tabs,weak browser]url in browser?.open(url);tabs?.selectedIndex=1 }
        tabs.viewControllers=[hub,browser];tabs.tabBar.tintColor=UIColor(red:0.45,green:0.78,blue:0.35,alpha:1)
        let window=UIWindow(windowScene:scene);window.rootViewController=tabs;window.makeKeyAndVisible();self.window=window
    }
    func sceneDidEnterBackground(_ scene:UIScene){BackgroundSync.shared.refresh()}
}
