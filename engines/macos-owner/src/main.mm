#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include <xpc/xpc.h>
#include <atomic>
#include <limits>
#include <memory>
#include <mutex>
#include <set>
#include <map>
#include <initializer_list>
#include <csignal>
#include <cstring>
#include "process.hpp"
#include "session.hpp"

namespace {
constexpr uint64_t kProtocol = 1;
constexpr size_t kMaximumDictionaryBytes = 64 * 1024;
// The user explicitly authorized source activation before target qualification.
// Only the admitted schema-2 diskless guest route is enabled; signature, image,
// profile and resource checks remain mandatory. This is not build/test evidence.
constexpr bool kExecutionBoundaryComplete = true;

bool uuid(const char* value) {
  if (!value || strnlen(value, 37) != 36) return false;
  bool nonzero = false;
  for (size_t index = 0; index < 36; ++index) {
    if (index == 8 || index == 13 || index == 18 || index == 23) {
      if (value[index] != '-') return false;
    } else {
      if (!((value[index] >= '0' && value[index] <= '9') ||
            (value[index] >= 'a' && value[index] <= 'f'))) return false;
      nonzero |= value[index] != '0';
    }
  }
  return nonzero;
}

uint64_t allocate_owner() {
  static std::atomic_uint64_t next{1};
  uint64_t value = next.load();
  while (value != std::numeric_limits<uint64_t>::max())
    if (next.compare_exchange_weak(value, value + 1)) return value;
  return 0; // Exhaustion never wraps into another connection's authority.
}

bool checked_peer(xpc_object_t message) {
  SecCodeRef guest = nullptr;
  // Public Security API derives identity from this kernel-authenticated XPC
  // message, not a numeric PID or an unauthenticated request field. Apple's
  // SecCode.cpp requires default flags for code lookup; strict/all-architecture
  // signature validation below is unchanged, against the same sealed broker
  // requirement. Do not substitute a connection name for message admission.
  OSStatus status = SecCodeCreateWithXPCMessage(message,kSecCSDefaultFlags,&guest);
  if (status != errSecSuccess || !guest) return false;
  NSString* text = NSBundle.mainBundle.infoDictionary[@"OmniPlayBrokerRequirement"];
  if (![text isKindOfClass:NSString.class] || text.length == 0 || text.length > 4096) {
    CFRelease(guest); return false;
  }
  SecRequirementRef requirement = nullptr;
  status = SecRequirementCreateWithString((__bridge CFStringRef)text,
      kSecCSDefaultFlags, &requirement);
  if (status == errSecSuccess && requirement) {
    status = SecCodeCheckValidity(guest,
        kSecCSStrictValidate | kSecCSCheckAllArchitectures, requirement);
  }
  if (requirement) CFRelease(requirement);
  CFRelease(guest);
  return status == errSecSuccess;
}

void reject(xpc_connection_t peer, xpc_object_t request) {
  xpc_object_t reply = xpc_dictionary_create_reply(request);
  if (!reply) { xpc_connection_cancel(peer); return; }
  xpc_dictionary_set_uint64(reply, "protocol", kProtocol);
  const char* id=xpc_dictionary_get_string(request,"request_id");
  if(uuid(id))xpc_dictionary_set_string(reply,"request_id",id);
  xpc_dictionary_set_string(reply, "status", "rejected");
  xpc_connection_send_message(peer, reply);
}

std::mutex profiles_mutex;
std::map<std::string,uint64_t> profiles;
struct Connection {
  uint64_t owner;
  std::string profile;
  std::set<std::string> sessions;
  bool closed=false;
  std::mutex mutex;
  explicit Connection(uint64_t id):owner(id){}
};
void disconnect(Connection& state){
  if(state.closed)return;state.closed=true;
  omniplay::macos::stop_connection(state.owner);
  std::lock_guard lock(profiles_mutex);
  auto found=profiles.find(state.profile);
  if(found!=profiles.end()&&found->second==state.owner)profiles.erase(found);
}
bool exact(xpc_object_t object,std::initializer_list<const char*> keys){
  if(xpc_dictionary_get_count(object)!=keys.size())return false;
  for(const auto* key:keys)if(!xpc_dictionary_get_value(object,key))return false;
  return true;
}
bool integer(xpc_object_t object,const char* key,uint64_t low,uint64_t high){
  auto value=xpc_dictionary_get_value(object,key);
  return value&&xpc_get_type(value)==XPC_TYPE_UINT64&&xpc_uint64_get_value(value)>=low&&xpc_uint64_get_value(value)<=high;
}
bool bounded(xpc_object_t value,size_t& budget,unsigned depth=0){
  if(depth>8||budget<16)return false;budget-=16;auto type=xpc_get_type(value);
  if(type==XPC_TYPE_STRING){const size_t bytes=xpc_string_get_length(value);if(bytes>4096||bytes>budget||strnlen(xpc_string_get_string_ptr(value),bytes+1)!=bytes)return false;budget-=bytes;return true;}
  if(type==XPC_TYPE_UINT64||type==XPC_TYPE_BOOL||type==XPC_TYPE_FD)return true;
  if(type==XPC_TYPE_ARRAY){const size_t count=xpc_array_get_count(value);if(count>144)return false;for(size_t i=0;i<count;++i)if(!bounded(xpc_array_get_value(value,i),budget,depth+1))return false;return true;}
  if(type==XPC_TYPE_DICTIONARY){if(xpc_dictionary_get_count(value)>32)return false;size_t* remaining=&budget;__block bool valid=true;
    xpc_dictionary_apply(value,^(const char* key,xpc_object_t item){const size_t bytes=strnlen(key,65);if(bytes>64||bytes>*remaining){valid=false;return false;}*remaining-=bytes;valid=bounded(item,*remaining,depth+1);return valid;});return valid;}
  return false;
}
void receive(xpc_connection_t peer,xpc_object_t event,Connection& state){
  if(state.closed)return;
  if(xpc_get_type(event)!=XPC_TYPE_DICTIONARY||!checked_peer(event)){
    disconnect(state);xpc_connection_cancel(peer);return;
  }
  size_t budget=kMaximumDictionaryBytes;if(!bounded(event,budget)){reject(peer,event);return;}
  const char* operation=xpc_dictionary_get_string(event,"operation");
  const char* request=xpc_dictionary_get_string(event,"request_id");
  if(!integer(event,"protocol",kProtocol,kProtocol)||!operation||strnlen(operation,65)>64||!uuid(request)){
    reject(peer,event);return;
  }
  auto reply=xpc_dictionary_create_reply(event);
  if(!reply){disconnect(state);xpc_connection_cancel(peer);return;}
  xpc_dictionary_set_uint64(reply,"protocol",kProtocol);
  xpc_dictionary_set_string(reply,"request_id",request);
  xpc_dictionary_set_string(reply,"status","rejected");
  if(!strcmp(operation,"capabilities")&&exact(event,{"protocol","operation","request_id"})){
    xpc_dictionary_set_string(reply,"status","ok");
    xpc_dictionary_set_bool(reply,"profile_binding",true);
    xpc_dictionary_set_bool(reply,"session_stage",true);
    xpc_dictionary_set_bool(reply,"session_discard",true);
    xpc_dictionary_set_bool(reply,"session_recover_previous_boot",true);
    xpc_dictionary_set_bool(reply,"session_launch",kExecutionBoundaryComplete);
    xpc_dictionary_set_uint64(reply,"maximum_control_bytes",kMaximumDictionaryBytes);
  }else if(!strcmp(operation,"bind_profile")&&exact(event,{"protocol","operation","request_id","profile_id"})){
    const char* profile=xpc_dictionary_get_string(event,"profile_id");
    if(uuid(profile)&&(state.profile.empty()||state.profile==profile)){
      std::lock_guard lock(profiles_mutex);
      auto found=profiles.find(profile);
      if(found==profiles.end()||found->second==state.owner){
        if(found!=profiles.end()||profiles.size()<4){profiles[profile]=state.owner;state.profile=profile;xpc_dictionary_set_string(reply,"status","ok");}
      }
    }
  }else{
    const char* profile=xpc_dictionary_get_string(event,"profile_id");
    const char* session=xpc_dictionary_get_string(event,"session_id");
    if(!state.profile.empty()&&uuid(profile)&&state.profile==profile&&uuid(session)){
      using namespace omniplay::macos;
      if(!strcmp(operation,"recover_session")){
        const auto status=recover_session(event,state.profile);
        if(status==StageStatus::ok)state.sessions.erase(session);
        xpc_dictionary_set_string(reply,"status",status==StageStatus::ok?"ok":status==StageStatus::storage_failure?"storage_failure":"rejected");
      }else if(!strcmp(operation,"stage_session")){
        if(state.sessions.contains(session)||state.sessions.size()<4){
          const auto status=stage_session(event,state.profile);
          if(status==StageStatus::ok){state.sessions.insert(session);xpc_dictionary_set_bool(reply,"staged",true);}
          xpc_dictionary_set_string(reply,"status",status==StageStatus::ok?"ok":status==StageStatus::storage_failure?"storage_failure":"rejected");
        }
      }else if(!strcmp(operation,"discard_session")){
        const auto status=discard_session(event,state.profile);
        if(status==StageStatus::ok)state.sessions.erase(session);
        xpc_dictionary_set_string(reply,"status",status==StageStatus::ok?"ok":status==StageStatus::storage_failure?"storage_failure":"rejected");
      }else if(kExecutionBoundaryComplete&&!strcmp(operation,"launch_session")&&exact(event,{"protocol","operation","request_id","profile_id","session_id","runtime_id","maximum_width","maximum_height","volume","haptics"})&&
          integer(event,"maximum_width",1,4096)&&integer(event,"maximum_height",1,4096)&&integer(event,"volume",0,100)){
        auto haptics=xpc_dictionary_get_value(event,"haptics");
        const char* runtime_id=xpc_dictionary_get_string(event,"runtime_id");
        if(runtime_id&&haptics&&xpc_get_type(haptics)==XPC_TYPE_BOOL&&(state.sessions.contains(session)||state.sessions.size()<4)){
          auto runtime=admit_runtime(runtime_id);auto storage=admit_staged_session(session,runtime_id,state.profile);
          if(runtime&&storage){
            LaunchParameters parameters{session,static_cast<uint32_t>(xpc_dictionary_get_uint64(event,"maximum_width")),static_cast<uint32_t>(xpc_dictionary_get_uint64(event,"maximum_height")),static_cast<uint32_t>(xpc_dictionary_get_uint64(event,"volume")),xpc_bool_get_value(haptics)};
            // Record authority before spawn: even a failed reply retains a
            // connection owner which is stopped on disconnect.
            state.sessions.insert(session);
            auto endpoints=launch_pair(*runtime,std::move(*storage),parameters,state.owner);
            if(endpoints){xpc_dictionary_set_fd(reply,"input",endpoints->input);xpc_dictionary_set_fd(reply,"output",endpoints->output);xpc_dictionary_set_string(reply,"status","ok");}
          }
        }
      }else if(state.sessions.contains(session)&&exact(event,{"protocol","operation","request_id","profile_id","session_id"})){
        if(!strcmp(operation,"session_status")){
          xpc_dictionary_set_bool(reply,"active",session_active(session));xpc_dictionary_set_string(reply,"status","ok");
        }else if(!strcmp(operation,"activate_session")){
          if(activate_session(session,state.owner))xpc_dictionary_set_string(reply,"status","ok");
        }else if(!strcmp(operation,"stop_session")){
          const bool accepted=stop_session(session,state.owner);
          if(accepted||!session_active(session)){xpc_dictionary_set_bool(reply,"active",session_active(session));xpc_dictionary_set_string(reply,"status","ok");}
        }
      }
    }
  }
  xpc_connection_send_message(peer,reply);
}
}
int main(){
  struct sigaction ignored{};ignored.sa_handler=SIG_IGN;
  if(sigemptyset(&ignored.sa_mask)!=0||sigaction(SIGPIPE,&ignored,nullptr)!=0)return 70;
  @autoreleasepool {
    xpc_main(^(xpc_connection_t peer){
      const uint64_t owner=allocate_owner();if(!owner){xpc_connection_cancel(peer);return;}
      auto state=std::make_shared<Connection>(owner);
      xpc_connection_set_event_handler(peer,^(xpc_object_t event){
        std::lock_guard lock(state->mutex);
        try{@autoreleasepool{receive(peer,event,*state);}}
        catch(...){disconnect(*state);xpc_connection_cancel(peer);}
      });
      xpc_connection_resume(peer);
    });
  }
  return 70;
}
