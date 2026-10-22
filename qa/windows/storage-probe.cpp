// Final-pass fixture only. Never package this executable as a product core.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <userenv.h>
#include <sddl.h>
#include <aclapi.h>
#include <objbase.h>
#include <cstdio>
#include <string>
#include <vector>

static std::wstring private_folder() {
  HANDLE token=nullptr;
  if(!OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&token))return {};
  DWORD bytes=0;GetTokenInformation(token,TokenAppContainerSid,nullptr,0,&bytes);
  if(bytes<sizeof(TOKEN_APPCONTAINER_INFORMATION)||bytes>4096){CloseHandle(token);return {};}
  std::vector<unsigned char> buffer(bytes);
  if(!GetTokenInformation(token,TokenAppContainerSid,buffer.data(),bytes,&bytes)){CloseHandle(token);return {};}
  CloseHandle(token);
  const auto sid=reinterpret_cast<TOKEN_APPCONTAINER_INFORMATION*>(buffer.data())->TokenAppContainer;
  if(!sid||!IsValidSid(sid))return {};
  LPWSTR sid_string=nullptr;
  if(!ConvertSidToStringSidW(sid,&sid_string))return {};
  LPWSTR path=nullptr;const auto result=GetAppContainerFolderPath(sid_string,&path);LocalFree(sid_string);
  if(FAILED(result)||!path){if(path)CoTaskMemFree(path);return {};}
  std::wstring value(path);CoTaskMemFree(path);return value;
}

static bool create_file(const std::wstring& path,bool expect_denied,DWORD& error) {
  const auto file=CreateFileW(path.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);
  error=file==INVALID_HANDLE_VALUE?GetLastError():ERROR_SUCCESS;
  if(file==INVALID_HANDLE_VALUE)return expect_denied&&error==ERROR_ACCESS_DENIED;
  static constexpr char contents[]="OmniPlay final storage probe";DWORD written=0;
  const auto size=static_cast<DWORD>(sizeof(contents));
  const bool wrote=WriteFile(file,contents,size,&written,nullptr)&&written==size&&FlushFileBuffers(file);
  CloseHandle(file);const bool removed=DeleteFileW(path.c_str())!=0;
  return !expect_denied&&wrote&&removed;
}

static bool create_directory_denied(const std::wstring& path) {
  if(CreateDirectoryW(path.c_str(),nullptr)){RemoveDirectoryW(path.c_str());return false;}
  return GetLastError()==ERROR_ACCESS_DENIED;
}

static bool write_dacl_denied(const std::wstring& path) {
  const auto file=CreateFileW(path.c_str(),WRITE_DAC,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,nullptr,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS,nullptr);
  if(file==INVALID_HANDLE_VALUE)return GetLastError()==ERROR_ACCESS_DENIED;
  CloseHandle(file);return false;
}

static bool lower_label_denied(const std::wstring& path) {
  PSECURITY_DESCRIPTOR descriptor=nullptr;
  if(!ConvertStringSecurityDescriptorToSecurityDescriptorW(L"S:(ML;OICI;NW;;;LW)",SDDL_REVISION_1,&descriptor,nullptr))return false;
  BOOL present=FALSE,defaulted=FALSE;PACL sacl=nullptr;
  const bool obtained=GetSecurityDescriptorSacl(descriptor,&present,&sacl,&defaulted)&&present&&sacl;
  const auto result=obtained?SetNamedSecurityInfoW(const_cast<LPWSTR>(path.c_str()),SE_FILE_OBJECT,LABEL_SECURITY_INFORMATION,nullptr,nullptr,nullptr,sacl):ERROR_INVALID_SECURITY_DESCR;
  LocalFree(descriptor);return result==ERROR_ACCESS_DENIED;
}

struct RegistryObservation {bool create_denied=false;bool set_denied=false;HRESULT lookup=E_FAIL;DWORD create=ERROR_INVALID_FUNCTION;HRESULT write=E_FAIL;};
static RegistryObservation registry_writes_denied(const std::wstring& name) {
  RegistryObservation observed;HKEY readonly=nullptr;
  observed.lookup=GetAppContainerRegistryLocation(KEY_READ,&readonly);
  if(FAILED(observed.lookup)||!readonly){if(readonly)RegCloseKey(readonly);return observed;}
  // RegCreateKeyEx checks the key's security descriptor, not merely the access
  // mask on this readonly handle. It can create the child if the policy leaks.
  HKEY child=nullptr;DWORD disposition=0;
  const auto created=RegCreateKeyExW(readonly,name.c_str(),0,nullptr,REG_OPTION_NON_VOLATILE,KEY_READ|KEY_WRITE,nullptr,&child,&disposition);
  observed.create=created;
  observed.create_denied=created==ERROR_ACCESS_DENIED;
  if(child)RegCloseKey(child);
  if(created==ERROR_SUCCESS&&disposition==REG_CREATED_NEW_KEY)RegDeleteKeyW(readonly,name.c_str());
  RegCloseKey(readonly);
  HKEY writable=nullptr;const auto opened=GetAppContainerRegistryLocation(KEY_SET_VALUE,&writable);
  observed.write=opened;
  observed.set_denied=opened==HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED);
  if(writable)RegCloseKey(writable);
  return observed;
}

static int invalid_observation(const char* stage,int code) {
  std::printf("storage-probe-invalid %s %lu\n",stage,GetLastError());
  std::fflush(stdout);return code;
}

int wmain() {
  const auto folder=private_folder();const auto split=folder.find_last_of(L"\\/");
  if(split==std::wstring::npos||_wcsicmp(folder.substr(split+1).c_str(),L"AC"))return invalid_observation("private-folder",2);
  const auto package=folder.substr(0,split);
  const auto name=L"omniplay-probe-"+std::to_wstring(GetCurrentProcessId());
  wchar_t temporary[32768];const auto length=GetEnvironmentVariableW(L"TMP",temporary,32768);
  if(!length||length>=32768)return invalid_observation("scratch-environment",3);
  DWORD default_error=0,scratch_error=0;
  const bool denied_file=create_file(folder+L"\\"+name+L".tmp",true,default_error);
  const bool denied_directory=create_directory_denied(package+L"\\"+name);
  const bool denied_dacl=write_dacl_denied(package);
  const bool denied_label=lower_label_denied(package);
  const auto registry=registry_writes_denied(name);
  const auto scratch_attributes=GetFileAttributesW(temporary);
  if(scratch_attributes==INVALID_FILE_ATTRIBUTES||!(scratch_attributes&FILE_ATTRIBUTE_DIRECTORY))return invalid_observation("scratch-directory",4);
#ifdef OMNIPLAY_EXPECT_SCRATCH_DENIED
  const bool scratch_allowed=create_file(std::wstring(temporary)+L"\\"+name+L".tmp",true,scratch_error);
  const char* protocol="storage-probe-readonly-v1";
#else
  const bool scratch_allowed=create_file(std::wstring(temporary)+L"\\"+name+L".tmp",false,scratch_error);
  const char* protocol="storage-probe-v2";
#endif
  std::printf("%s %d %d %d %d %d %d %d\n",protocol,denied_file,denied_directory,denied_dacl,denied_label,registry.create_denied,registry.set_denied,scratch_allowed);
  if(!denied_file||!registry.create_denied||!registry.set_denied||!scratch_allowed){
    std::printf("storage-probe-errors default=%lu scratch=%lu registry-read=%08lx registry-create=%lu registry-write=%08lx\n",default_error,scratch_error,static_cast<unsigned long>(registry.lookup),registry.create,static_cast<unsigned long>(registry.write));
  }
  std::fflush(stdout);
  return denied_file&&denied_directory&&denied_dacl&&denied_label&&registry.create_denied&&registry.set_denied&&scratch_allowed?0:1;
}
