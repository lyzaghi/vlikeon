#import <Foundation/Foundation.h>
#import <Security/Security.h>
#include <xpc/xpc.h>
#include <dispatch/dispatch.h>
#include "macos_xpc.h"
#include <atomic>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <fcntl.h>
#include <sys/codesign.h>
#include <string>
#include <unistd.h>

namespace {
constexpr size_t maximum = 65536;
std::atomic_uint pending_calls{0};
struct Client {
  xpc_connection_t connection = nullptr;
  SecRequirementRef requirement = nullptr;
  std::string executable;
  std::atomic_bool dead{false};
  ~Client(){if(connection)xpc_connection_cancel(connection);if(requirement)CFRelease(requirement);}
};
struct Pending {
  dispatch_semaphore_t signal = dispatch_semaphore_create(0);
  xpc_object_t reply = nullptr;
  ~Pending(){pending_calls.fetch_sub(1);}
};
bool peer(xpc_object_t message,const Client& client){
  SecCodeRef code=nullptr;
  // Public Security API derives the peer from the kernel-backed XPC message.
  // Lookup has default flags; the actual signature validity check stays strict.
  if(SecCodeCreateWithXPCMessage(message,kSecCSDefaultFlags,&code)!=errSecSuccess||!code)return false;
  auto status=SecCodeCheckValidity(code,kSecCSStrictValidate|kSecCSCheckAllArchitectures,client.requirement);
  CFDictionaryRef raw=nullptr;
  if(status==errSecSuccess)status=SecCodeCopySigningInformation(code,kSecCSSigningInformation,&raw);
  CFRelease(code);if(status!=errSecSuccess||!raw){if(raw)CFRelease(raw);return false;}
  NSDictionary* info=CFBridgingRelease(raw);
  NSNumber* flags=info[(__bridge id)kSecCodeInfoFlags];
  NSURL* executable=info[(__bridge id)kSecCodeInfoMainExecutable];
  NSDictionary* entitlements=info[(__bridge id)kSecCodeInfoEntitlementsDict];
  if(![flags isKindOfClass:NSNumber.class]||(flags.unsignedIntValue&CS_RUNTIME)==0||
      ![executable isKindOfClass:NSURL.class]||!executable.isFileURL||
      ![executable.URLByResolvingSymlinksInPath.path isEqual:[NSString stringWithUTF8String:client.executable.c_str()]]||
      ![entitlements isKindOfClass:NSDictionary.class]||![entitlements[@"com.apple.security.app-sandbox"] isEqual:@YES]||![entitlements[@"com.apple.security.virtualization"] isEqual:@YES])return false;
  NSSet* allowed=[NSSet setWithArray:@[@"com.apple.security.app-sandbox",@"com.apple.security.virtualization",@"com.apple.application-identifier",@"com.apple.developer.team-identifier",@"keychain-access-groups"]];
  for(id key in entitlements)if(![key isKindOfClass:NSString.class]||![allowed containsObject:key])return false;
  NSString* signing=info[(__bridge id)kSecCodeInfoIdentifier];NSString* team=info[(__bridge id)kSecCodeInfoTeamIdentifier];
  if(![signing isKindOfClass:NSString.class]||![team isKindOfClass:NSString.class])return false;
  NSString* application=[NSString stringWithFormat:@"%@.%@",team,signing];
  id groups=entitlements[@"keychain-access-groups"];
  return (!entitlements[@"com.apple.application-identifier"]||[entitlements[@"com.apple.application-identifier"] isEqual:application])&&
      (!entitlements[@"com.apple.developer.team-identifier"]||[entitlements[@"com.apple.developer.team-identifier"] isEqual:team])&&
      (!groups||([groups isKindOfClass:NSArray.class]&&[groups count]==1&&[groups[0] isEqual:application]));
}
bool debit(size_t& budget,size_t count){if(count>budget)return false;budget-=count;return true;}
xpc_object_t encode(id object,size_t& budget,unsigned depth){
  if(depth>16||!debit(budget,16))return nullptr;
  if([object isKindOfClass:NSString.class]){const char* text=[object UTF8String];if(!text)return nullptr;const size_t count=strnlen(text,4097);if(count>4096||[object length]!=count||!debit(budget,count))return nullptr;return xpc_string_create(text);}
  if([object isKindOfClass:NSNumber.class]){
    if(CFGetTypeID((__bridge CFTypeRef)object)==CFBooleanGetTypeID())return xpc_bool_create([object boolValue]);
    if(CFNumberIsFloatType((__bridge CFNumberRef)object)||[object longLongValue]<0)return nullptr;
    return xpc_uint64_create([object unsignedLongLongValue]);
  }
  if([object isKindOfClass:NSArray.class]){if([object count]>144)return nullptr;xpc_object_t result=xpc_array_create(nullptr,0);for(id item in object){auto value=encode(item,budget,depth+1);if(!value)return nullptr;xpc_array_append_value(result,value);}return result;}
  if([object isKindOfClass:NSDictionary.class]){if([object count]>32)return nullptr;xpc_object_t result=xpc_dictionary_create(nullptr,nullptr,0);for(id key in object){if(![key isKindOfClass:NSString.class]||[key length]>64)return nullptr;auto value=encode(object[key],budget,depth+1);if(!value)return nullptr;xpc_dictionary_set_value(result,[key UTF8String],value);}return result;}
  return nullptr;
}
id decode(xpc_object_t object,size_t& budget,unsigned depth){
  if(depth>16||!debit(budget,16))return nil;
  auto type=xpc_get_type(object);
  if(type==XPC_TYPE_STRING){const char* text=xpc_string_get_string_ptr(object);const size_t count=xpc_string_get_length(object);if(count>4096||strnlen(text,count+1)!=count||!debit(budget,count))return nil;return [[NSString alloc]initWithBytes:text length:count encoding:NSUTF8StringEncoding];}
  if(type==XPC_TYPE_UINT64)return @(xpc_uint64_get_value(object));
  if(type==XPC_TYPE_BOOL)return @(xpc_bool_get_value(object));
  if(type==XPC_TYPE_ARRAY){const size_t count=xpc_array_get_count(object);if(count>144)return nil;NSMutableArray* result=[NSMutableArray arrayWithCapacity:count];for(size_t i=0;i<count;++i){id item=decode(xpc_array_get_value(object,i),budget,depth+1);if(!item)return nil;[result addObject:item];}return result;}
  if(type==XPC_TYPE_DICTIONARY){if(xpc_dictionary_get_count(object)>32)return nil;NSMutableDictionary* result=[NSMutableDictionary dictionary];__block bool valid=true;size_t* remaining=&budget;
    xpc_dictionary_apply(object,^(const char* key,xpc_object_t value){if(!strcmp(key,"input")||!strcmp(key,"output")){if(depth||xpc_get_type(value)!=XPC_TYPE_FD){valid=false;return false;}return true;}const size_t count=strnlen(key,65);if(count>64||!debit(*remaining,count)){valid=false;return false;}NSString* name=[NSString stringWithUTF8String:key];id item=decode(value,*remaining,depth+1);if(!name||!item){valid=false;return false;}result[name]=item;return true;});return valid?result:nil;}
  return nil;
}
}
struct opmac_client { std::shared_ptr<Client> value; };
extern "C" opmac_client* opmac_open(const char* service,const char* requirement,const char* executable){
  @autoreleasepool {try{
    if(!service||!*service||!requirement||!*requirement||!executable||*executable!='/'||strnlen(executable,4097)>4096||strnlen(service,161)>160||strnlen(requirement,4097)>4096)return nullptr;
    auto client=std::make_shared<Client>();NSString* text=[NSString stringWithUTF8String:requirement];
    client->executable=executable;
    if(!text||SecRequirementCreateWithString((__bridge CFStringRef)text,kSecCSDefaultFlags,&client->requirement)!=errSecSuccess)return nullptr;
    client->connection=xpc_connection_create(service,nullptr);if(!client->connection)return nullptr;
    std::weak_ptr<Client> weak=client;
    xpc_connection_set_event_handler(client->connection,^(xpc_object_t event){if(xpc_get_type(event)==XPC_TYPE_ERROR){if(auto state=weak.lock())state->dead=true;}});
    xpc_connection_resume(client->connection);return new opmac_client{client};
  }catch(...){return nullptr;}}
}
extern "C" int opmac_call(opmac_client* handle,const unsigned char* bytes,size_t length,
    const int* descriptors,size_t count,unsigned timeout_ms,opmac_reply* output){
  if(!output)return 1;*output={nullptr,0,-1,-1};
  @autoreleasepool {try{
    if(!handle||!bytes||!length||length>maximum||count>144||(!descriptors&&count)||!timeout_ms||timeout_ms>300000)return 1;
    auto client=handle->value;if(client->dead)return 2;
    id value=[NSJSONSerialization JSONObjectWithData:[NSData dataWithBytes:bytes length:length] options:0 error:nil];
    if(![value isKindOfClass:NSDictionary.class])return 1;size_t budget=maximum;
    auto message=encode(value,budget,0);if(!message||xpc_get_type(message)!=XPC_TYPE_DICTIONARY)return 1;
    if(count){auto assets=xpc_dictionary_get_value(message,"assets");if(!assets||xpc_get_type(assets)!=XPC_TYPE_ARRAY||xpc_array_get_count(assets)!=count)return 1;for(size_t i=0;i<count;++i){auto asset=xpc_array_get_value(assets,i);if(descriptors[i]<0||xpc_get_type(asset)!=XPC_TYPE_DICTIONARY)return 1;xpc_dictionary_set_fd(asset,"fd",descriptors[i]);}}
    unsigned slots=pending_calls.load();do{if(slots>=16)return 3;}while(!pending_calls.compare_exchange_weak(slots,slots+1));
    std::shared_ptr<Pending> pending;try{pending=std::make_shared<Pending>();}catch(...){pending_calls.fetch_sub(1);throw;}
    if(!pending->signal)return 3;
    xpc_connection_send_message_with_reply(client->connection,message,dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0),^(xpc_object_t reply){pending->reply=reply;dispatch_semaphore_signal(pending->signal);});
    if(dispatch_semaphore_wait(pending->signal,dispatch_time(DISPATCH_TIME_NOW,static_cast<int64_t>(timeout_ms)*1000000))!=0){client->dead=true;xpc_connection_cancel(client->connection);return 2;}
    if(!pending->reply||xpc_get_type(pending->reply)!=XPC_TYPE_DICTIONARY||!peer(pending->reply,*client)){client->dead=true;xpc_connection_cancel(client->connection);return 2;}
    budget=maximum;id response=decode(pending->reply,budget,0);if(!response)return 2;
    NSData* encoded=[NSJSONSerialization dataWithJSONObject:response options:0 error:nil];if(!encoded||!encoded.length||encoded.length>maximum)return 2;
    output->json=static_cast<unsigned char*>(malloc(encoded.length));if(!output->json)return 3;output->bytes=encoded.length;memcpy(output->json,encoded.bytes,encoded.length);
    auto input=xpc_dictionary_get_value(pending->reply,"input"),out=xpc_dictionary_get_value(pending->reply,"output");
    if(input||out){if(!input||!out||xpc_get_type(input)!=XPC_TYPE_FD||xpc_get_type(out)!=XPC_TYPE_FD){opmac_reply_free(output);return 2;}output->input=xpc_dictionary_dup_fd(pending->reply,"input");output->output=xpc_dictionary_dup_fd(pending->reply,"output");if(output->input<0||output->output<0||fcntl(output->input,F_SETFD,FD_CLOEXEC)!=0||fcntl(output->output,F_SETFD,FD_CLOEXEC)!=0){opmac_reply_free(output);return 2;}}
    return 0;
  }catch(...){opmac_reply_free(output);return 3;}}
}
extern "C" void opmac_reply_free(opmac_reply* value){if(!value)return;if(value->json)free(value->json);if(value->input>=0)close(value->input);if(value->output>=0)close(value->output);*value={nullptr,0,-1,-1};}
extern "C" void opmac_close(opmac_client* value){if(!value)return;value->value->dead=true;xpc_connection_cancel(value->value->connection);delete value;}
