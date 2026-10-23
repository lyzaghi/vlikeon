#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include <bsm/libbsm.h>
#include <xpc/xpc.h>
#include <atomic>
#include <limits>
#include "process.hpp"
#include "session.hpp"

namespace {
constexpr uint64_t kProtocol = 1;
constexpr size_t kMaximumDictionaryBytes = 64 * 1024;

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

bool checked_peer(xpc_connection_t peer) {
  audit_token_t token{};
  xpc_connection_get_audit_token(peer, &token);
  CFDataRef audit = CFDataCreate(kCFAllocatorDefault,
      reinterpret_cast<const UInt8*>(&token), sizeof(token));
  if (!audit) return false;
  const void* keys[] = {kSecGuestAttributeAudit};
  const void* values[] = {audit};
  CFDictionaryRef attributes = CFDictionaryCreate(kCFAllocatorDefault, keys, values, 1,
      &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
  CFRelease(audit);
  if (!attributes) return false;
  SecCodeRef guest = nullptr;
  OSStatus status = SecCodeCopyGuestWithAttributes(nullptr, attributes,
      kSecCSStrictValidate | kSecCSCheckAllArchitectures, &guest);
  CFRelease(attributes);
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
  xpc_dictionary_set_string(reply, "status", "rejected");
  xpc_connection_send_message(peer, reply);
}

void receive(xpc_connection_t peer, xpc_object_t event, uint64_t owner) {
  if (xpc_get_type(event) != XPC_TYPE_DICTIONARY) {
    omniplay::macos::stop_connection(owner);
    xpc_connection_cancel(peer); return;
  }
  if (xpc_dictionary_get_uint64(event, "protocol") != kProtocol) {
    reject(peer, event); return;
  }
  const char* operation = xpc_dictionary_get_string(event, "operation");
  const char* request = xpc_dictionary_get_string(event, "request_id");
  if (!operation || strnlen(operation, 65) > 64 || !uuid(request)) {
    reject(peer, event); return;
  }
  if (strcmp(operation,"stage_session")==0) {
    auto status=omniplay::macos::stage_session(event);xpc_object_t reply=xpc_dictionary_create_reply(event);if(!reply){xpc_connection_cancel(peer);return;}xpc_dictionary_set_uint64(reply,"protocol",kProtocol);xpc_dictionary_set_string(reply,"request_id",request);xpc_dictionary_set_string(reply,"status",status==omniplay::macos::StageStatus::ok?"ok":status==omniplay::macos::StageStatus::storage_failure?"storage_failure":"rejected");xpc_dictionary_set_bool(reply,"staged",status==omniplay::macos::StageStatus::ok);xpc_connection_send_message(peer,reply);return;
  }
  if (strcmp(operation,"discard_session")==0) {
    auto status=omniplay::macos::discard_session(event);xpc_object_t reply=xpc_dictionary_create_reply(event);if(!reply){xpc_connection_cancel(peer);return;}xpc_dictionary_set_uint64(reply,"protocol",kProtocol);xpc_dictionary_set_string(reply,"request_id",request);xpc_dictionary_set_string(reply,"status",status==omniplay::macos::StageStatus::ok?"ok":status==omniplay::macos::StageStatus::storage_failure?"storage_failure":"rejected");xpc_connection_send_message(peer,reply);return;
  }
  if(strcmp(operation,"capabilities")!=0||xpc_dictionary_get_count(event)!=3){reject(peer,event);return;}
  xpc_object_t reply = xpc_dictionary_create_reply(event);
  if (!reply) { xpc_connection_cancel(peer); return; }
  xpc_dictionary_set_uint64(reply, "protocol", kProtocol);
  xpc_dictionary_set_string(reply, "request_id", request);
  xpc_dictionary_set_string(reply, "status", "ok");
  xpc_dictionary_set_bool(reply, "session_stage", true);
  xpc_dictionary_set_bool(reply, "session_discard", true);
  xpc_dictionary_set_bool(reply, "session_launch", false);
  xpc_dictionary_set_uint64(reply, "maximum_control_bytes", kMaximumDictionaryBytes);
  xpc_connection_send_message(peer, reply);
}
}

int main() {
  @autoreleasepool {
    xpc_main(^(xpc_connection_t peer) {
      if (!checked_peer(peer)) { xpc_connection_cancel(peer); return; }
      const uint64_t owner = allocate_owner();
      if (!owner) { xpc_connection_cancel(peer); return; }
      xpc_connection_set_event_handler(peer, ^(xpc_object_t event) {
        try { @autoreleasepool { receive(peer, event, owner); } }
        catch (...) { omniplay::macos::stop_connection(owner); xpc_connection_cancel(peer); }
      });
      xpc_connection_resume(peer);
    });
  }
  return 70;
}
