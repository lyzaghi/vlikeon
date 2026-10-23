#include <windows.h>
#include <shellapi.h>
#include <wrl.h>
#include <WebView2.h>
#include <algorithm>
#include <filesystem>
#include <string>
#include <vector>

using Microsoft::WRL::Callback;
using Microsoft::WRL::ComPtr;
namespace fs=std::filesystem;
namespace {
constexpr auto kPage=L"https://demo.vlikeon.local/index.html";
HWND window=nullptr;
ComPtr<ICoreWebView2Controller> controller;
ComPtr<ICoreWebView2> web;
bool ready=false,closing=false,fullscreen=false;
WINDOWPLACEMENT placement{sizeof(WINDOWPLACEMENT)};
DWORD previous_style=0;

bool trusted(const wchar_t* value){
  if(!value)return false;const std::wstring uri(value),page(kPage);
  return uri==page||uri.starts_with(page+L"#");
}
void failure(const wchar_t* message){MessageBoxW(window,message,L"Vlikeon Demo",MB_OK|MB_ICONERROR);}
void resize(){if(controller){RECT bounds{};GetClientRect(window,&bounds);controller->put_Bounds(bounds);}}
void display_fullscreen(bool enabled){
  if(fullscreen==enabled)return;fullscreen=enabled;
  if(enabled){previous_style=static_cast<DWORD>(GetWindowLongPtrW(window,GWL_STYLE));GetWindowPlacement(window,&placement);MONITORINFO info{sizeof(info)};if(GetMonitorInfoW(MonitorFromWindow(window,MONITOR_DEFAULTTONEAREST),&info)){SetWindowLongPtrW(window,GWL_STYLE,previous_style&~WS_OVERLAPPEDWINDOW);SetWindowPos(window,HWND_TOP,info.rcMonitor.left,info.rcMonitor.top,info.rcMonitor.right-info.rcMonitor.left,info.rcMonitor.bottom-info.rcMonitor.top,SWP_FRAMECHANGED|SWP_NOOWNERZORDER);}}
  else{SetWindowLongPtrW(window,GWL_STYLE,previous_style);SetWindowPlacement(window,&placement);SetWindowPos(window,nullptr,0,0,0,0,SWP_FRAMECHANGED|SWP_NOMOVE|SWP_NOSIZE|SWP_NOZORDER|SWP_NOOWNERZORDER);}
  resize();
}
void initialize(const fs::path& assets,const fs::path& data){
  const auto result=CreateCoreWebView2EnvironmentWithOptions(nullptr,data.c_str(),nullptr,
    Callback<ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler>([assets](HRESULT error,ICoreWebView2Environment* environment)->HRESULT{
      if(FAILED(error)||!environment){failure(L"Không mở được WebView2 Runtime. Hãy kiểm tra Microsoft Edge WebView2 Runtime rồi mở lại demo.");return S_OK;}
      return environment->CreateCoreWebView2Controller(window,Callback<ICoreWebView2CreateCoreWebView2ControllerCompletedHandler>([assets](HRESULT status,ICoreWebView2Controller* value)->HRESULT{
        if(FAILED(status)||!value){failure(L"Không tạo được cửa sổ game.");return S_OK;}
        controller=value;if(FAILED(controller->get_CoreWebView2(&web))||!web){failure(L"Không khởi tạo được Game Hub.");return S_OK;}
        ComPtr<ICoreWebView2_3> local;
        if(FAILED(web.As(&local))||FAILED(local->SetVirtualHostNameToFolderMapping(L"demo.vlikeon.local",assets.c_str(),COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_DENY_CORS))){failure(L"WebView2 Runtime chưa hỗ trợ tài nguyên cục bộ của demo.");return S_OK;}
        ComPtr<ICoreWebView2Settings> settings;
        if(SUCCEEDED(web->get_Settings(&settings))){settings->put_AreDevToolsEnabled(FALSE);settings->put_AreDefaultContextMenusEnabled(FALSE);settings->put_IsStatusBarEnabled(FALSE);}
        EventRegistrationToken token{};
        web->add_NavigationStarting(Callback<ICoreWebView2NavigationStartingEventHandler>([](ICoreWebView2*,ICoreWebView2NavigationStartingEventArgs* args)->HRESULT{LPWSTR uri=nullptr;if(SUCCEEDED(args->get_Uri(&uri))){if(!trusted(uri))args->put_Cancel(TRUE);CoTaskMemFree(uri);}else args->put_Cancel(TRUE);return S_OK;}).Get(),&token);
        web->add_NewWindowRequested(Callback<ICoreWebView2NewWindowRequestedEventHandler>([](ICoreWebView2*,ICoreWebView2NewWindowRequestedEventArgs* args)->HRESULT{args->put_Handled(TRUE);return S_OK;}).Get(),&token);
        web->add_PermissionRequested(Callback<ICoreWebView2PermissionRequestedEventHandler>([](ICoreWebView2*,ICoreWebView2PermissionRequestedEventArgs* args)->HRESULT{args->put_State(COREWEBVIEW2_PERMISSION_STATE_DENY);return S_OK;}).Get(),&token);
        web->add_WebMessageReceived(Callback<ICoreWebView2WebMessageReceivedEventHandler>([](ICoreWebView2*,ICoreWebView2WebMessageReceivedEventArgs* args)->HRESULT{
          LPWSTR source=nullptr,message=nullptr;
          const bool origin=SUCCEEDED(args->get_Source(&source))&&trusted(source);
          if(origin&&SUCCEEDED(args->TryGetWebMessageAsString(&message))&&message){
            if(std::wstring(message)==L"ready")ready=true;
            else if(closing&&std::wstring(message)==L"close-ready"){KillTimer(window,1);DestroyWindow(window);}
            else if(closing&&std::wstring(message)==L"close-failed"){KillTimer(window,1);closing=false;failure(L"Chưa lưu được điểm dừng. Hãy thử lưu trong game trước khi đóng.");}
          }
          CoTaskMemFree(source);CoTaskMemFree(message);return S_OK;
        }).Get(),&token);
        web->add_ContainsFullScreenElementChanged(Callback<ICoreWebView2ContainsFullScreenElementChangedEventHandler>([](ICoreWebView2* sender,IUnknown*)->HRESULT{BOOL value=FALSE;if(SUCCEEDED(sender->get_ContainsFullScreenElement(&value)))display_fullscreen(value!=FALSE);return S_OK;}).Get(),&token);
        resize();controller->put_IsVisible(TRUE);controller->MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
        return web->Navigate(kPage);
      }).Get());
    }).Get());
  if(FAILED(result))failure(L"Chưa khởi tạo được WebView2. Demo cần Microsoft Edge WebView2 Runtime.");
}
LRESULT CALLBACK procedure(HWND hwnd,UINT message,WPARAM wparam,LPARAM lparam){
  switch(message){
    case WM_SIZE:resize();return 0;
    case WM_SETFOCUS:if(controller)controller->MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);return 0;
    case WM_DPICHANGED:{const auto* bounds=reinterpret_cast<const RECT*>(lparam);SetWindowPos(hwnd,nullptr,bounds->left,bounds->top,bounds->right-bounds->left,bounds->bottom-bounds->top,SWP_NOZORDER|SWP_NOACTIVATE);resize();return 0;}
    case WM_GETMINMAXINFO:{auto* info=reinterpret_cast<MINMAXINFO*>(lparam);info->ptMinTrackSize={860,620};return 0;}
    case WM_CLOSE:
      if(!ready||!web){DestroyWindow(hwnd);return 0;}
      if(!closing){closing=true;SetTimer(hwnd,1,8000,nullptr);web->ExecuteScript(L"window.dispatchEvent(new Event('vlikeon-close'))",nullptr);}return 0;
    case WM_TIMER:
      if(wparam==1&&closing){KillTimer(hwnd,1);if(MessageBoxW(hwnd,L"Game chưa xác nhận lưu xong. Chọn Không để giữ cửa sổ mở. Chọn Có để thoát; điểm lưu trước đó vẫn được giữ.",L"Vlikeon Demo",MB_YESNO|MB_DEFBUTTON2|MB_ICONQUESTION)==IDYES)DestroyWindow(hwnd);else{closing=false;web->ExecuteScript(L"window.dispatchEvent(new Event('vlikeon-close-cancel'))",nullptr);}}return 0;
    case WM_DESTROY:if(controller)controller->Close();web.Reset();controller.Reset();PostQuitMessage(0);return 0;
  }
  return DefWindowProcW(hwnd,message,wparam,lparam);
}
}
int WINAPI wWinMain(HINSTANCE instance,HINSTANCE,PWSTR,int show){
  if(FAILED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)))return 1;
  try{
    std::vector<wchar_t> path(32768);const auto length=GetModuleFileNameW(nullptr,path.data(),static_cast<DWORD>(path.size()));if(!length||length>=path.size())throw 1;
    const auto root=fs::path(std::wstring(path.data(),length)).parent_path(),assets=root/L"assets",data=root/L"user-data";
    if(!fs::is_regular_file(assets/L"index.html")){failure(L"Thiếu thư mục assets. Hãy giải nén toàn bộ bản demo rồi mở VlikeonDemo.exe.");CoUninitialize();return 1;}
    fs::create_directories(data);
    WNDCLASSEXW type{sizeof(type)};type.lpfnWndProc=procedure;type.hInstance=instance;type.hCursor=LoadCursorW(nullptr,IDC_ARROW);type.hIcon=LoadIconW(instance,MAKEINTRESOURCEW(101));type.hIconSm=type.hIcon;type.hbrBackground=CreateSolidBrush(RGB(10,13,22));type.lpszClassName=L"VlikeonDemoWindow";
    if(!RegisterClassExW(&type))throw 1;
    const auto width=std::min(1320,GetSystemMetrics(SM_CXSCREEN)-60),height=std::min(880,GetSystemMetrics(SM_CYSCREEN)-70);
    window=CreateWindowExW(0,type.lpszClassName,L"Vlikeon — Game Hub Demo",WS_OVERLAPPEDWINDOW,CW_USEDEFAULT,CW_USEDEFAULT,width,height,nullptr,nullptr,instance,nullptr);if(!window)throw 1;
    ShowWindow(window,show);UpdateWindow(window);initialize(assets,data);
    MSG message{};while(GetMessageW(&message,nullptr,0,0)>0){TranslateMessage(&message);DispatchMessageW(&message);}
    CoUninitialize();return 0;
  }catch(...){failure(L"Không mở được dữ liệu demo. Hãy giải nén vào thư mục bạn có quyền ghi rồi thử lại.");CoUninitialize();return 1;}
}
