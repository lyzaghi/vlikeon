#include <jni.h>
#include <cstdint>
#include <array>
#include <memory>
#include <mutex>
#include <vector>
#include "omniplay_mobile.h"

namespace {
constexpr size_t kCredentialBytes=4096;
static_assert(sizeof(OmniPairingAddress)==68,"pairing address ABI size changed");
struct WipeBytes{std::vector<uint8_t>& bytes;~WipeBytes(){volatile uint8_t* value=bytes.data();for(size_t i=0;i<bytes.size();++i)value[i]=0;}};
struct ReleaseBuffer{OmniBuffer value;~ReleaseBuffer(){om_mobile_free(value);}};
bool exception(JNIEnv* env){if(!env->ExceptionCheck())return false;env->ExceptionClear();return true;}
struct JavaAccess{
  JavaVM* vm;JNIEnv* env=nullptr;bool attached=false,frame=false;
  explicit JavaAccess(JavaVM* value):vm(value){
    const auto status=vm->GetEnv(reinterpret_cast<void**>(&env),JNI_VERSION_1_6);
    if(status==JNI_EDETACHED){env=nullptr;if(vm->AttachCurrentThread(&env,nullptr)!=JNI_OK){env=nullptr;return;}attached=true;}
    else if(status!=JNI_OK){env=nullptr;return;}
    if(env->PushLocalFrame(8)<0){exception(env);env=nullptr;return;}frame=true;
  }
  ~JavaAccess(){if(frame)env->PopLocalFrame(nullptr);if(attached)vm->DetachCurrentThread();}
};
struct WipeJavaBytes{
  JNIEnv* env;jbyteArray bytes;jsize length;
  ~WipeJavaBytes(){if(bytes&&length>=0&&length<=static_cast<jsize>(kCredentialBytes)){exception(env);std::array<jbyte,kCredentialBytes> zero{};env->SetByteArrayRegion(bytes,0,length,zero.data());exception(env);}}
};
struct CredentialContext{JavaVM* vm=nullptr;jobject provider=nullptr;jmethodID read=nullptr,write=nullptr,remove=nullptr,check=nullptr;};
constexpr size_t kMediaBytes=64*1024;
struct MediaContext{
  JavaVM* vm=nullptr;jobject provider=nullptr;jbyteArray buffer=nullptr;
  jmethodID open=nullptr,read=nullptr,close=nullptr,cancel=nullptr,release=nullptr,retain=nullptr;
  std::mutex reader;
};
void release_media(void* opaque){
  std::unique_ptr<MediaContext> value(static_cast<MediaContext*>(opaque));if(!value)return;
  JavaAccess access(value->vm);if(!access.env)return;auto* env=access.env;
  if(value->provider){env->CallVoidMethod(value->provider,value->release);exception(env);env->DeleteGlobalRef(value->provider);}
  if(value->buffer)env->DeleteGlobalRef(value->buffer);
}
void release_credentials(void* opaque){
  std::unique_ptr<CredentialContext> value(static_cast<CredentialContext*>(opaque));if(!value)return;
  JavaAccess access(value->vm);if(access.env&&value->provider)access.env->DeleteGlobalRef(value->provider);
}
jbyteArray java_profile(JNIEnv* env,const uint8_t* profile){
  auto value=env->NewByteArray(16);if(exception(env)||!value)return nullptr;
  env->SetByteArrayRegion(value,0,16,reinterpret_cast<const jbyte*>(profile));return exception(env)?nullptr:value;
}
// Dedicated pairing ABI: four UUIDs followed by network-order u32 kind.
// Never memcpy the C struct's platform-endian integer/padding into Java.
jbyteArray java_pairing_address(JNIEnv* env,const OmniPairingAddress* address){
  if(!address||(address->kind!=1&&address->kind!=2))return nullptr;
  std::array<jbyte,68> encoded{};
  for(size_t i=0;i<16;++i){encoded[i]=static_cast<jbyte>(address->profile[i]);encoded[16+i]=static_cast<jbyte>(address->device[i]);
    encoded[32+i]=static_cast<jbyte>(address->item[i]);encoded[48+i]=static_cast<jbyte>(address->generation[i]);}
  for(size_t i=0;i<4;++i)encoded[64+i]=static_cast<jbyte>((address->kind>>(24-8*i))&0xff);
  auto value=env->NewByteArray(68);if(exception(env)||!value)return nullptr;
  env->SetByteArrayRegion(value,0,68,encoded.data());return exception(env)?nullptr:value;
}
int32_t check_pairing(void* opaque,const OmniPairingAddress* address){
  if(!opaque||!address)return -3;auto& context=*static_cast<CredentialContext*>(opaque);
  JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_pairing_address(env,address);if(!identity)return -3;
  const jint status=env->CallIntMethod(context.provider,context.check,identity);return exception(env)?-3:status;
}
int64_t read_pairing(void* opaque,const OmniPairingAddress* address,uint8_t* output,size_t capacity){
  if(!opaque||!address||!output||capacity!=137)return -3;auto& context=*static_cast<CredentialContext*>(opaque);
  JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_pairing_address(env,address);if(!identity)return -3;
  auto bytes=env->NewByteArray(137);if(exception(env)||!bytes)return -3;WipeJavaBytes wipe{env,bytes,137};
  const jint count=env->CallIntMethod(context.provider,context.read,identity,bytes);if(exception(env))return -3;
  if(count==-1||count==-2||count==-3)return count;if(count!=137)return -3;
  env->GetByteArrayRegion(bytes,0,137,reinterpret_cast<jbyte*>(output));return exception(env)?-3:137;
}
int32_t write_pairing(void* opaque,const OmniPairingAddress* address,const uint8_t* secret,size_t length){
  if(!opaque||!address||!secret||length!=137)return -3;auto& context=*static_cast<CredentialContext*>(opaque);
  JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_pairing_address(env,address);if(!identity)return -3;
  auto bytes=env->NewByteArray(137);if(exception(env)||!bytes)return -3;WipeJavaBytes wipe{env,bytes,137};
  env->SetByteArrayRegion(bytes,0,137,reinterpret_cast<const jbyte*>(secret));if(exception(env))return -3;
  const jint status=env->CallIntMethod(context.provider,context.write,identity,bytes);return exception(env)?-3:status;
}
int32_t remove_pairing(void* opaque,const OmniPairingAddress* address){
  if(!opaque||!address)return -3;auto& context=*static_cast<CredentialContext*>(opaque);
  JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_pairing_address(env,address);if(!identity)return -3;
  const jint status=env->CallIntMethod(context.provider,context.remove,identity);return exception(env)?-3:status;
}
int32_t open_media(void* opaque,const uint8_t* profile,const uint8_t* relative,size_t length,OmniMediaStream* stream){
  if(!opaque||!profile||!relative||!stream||length==0||length>2048)return -3;
  auto& context=*static_cast<MediaContext*>(opaque);JavaAccess access(context.vm);auto* env=access.env;if(!env)return -5;
  auto identity=java_profile(env,profile);if(!identity)return -5;auto path=env->NewByteArray(static_cast<jsize>(length));if(exception(env)||!path)return -5;
  env->SetByteArrayRegion(path,0,static_cast<jsize>(length),reinterpret_cast<const jbyte*>(relative));if(exception(env))return -5;
  auto result=static_cast<jlongArray>(env->CallObjectMethod(context.provider,context.open,identity,path));if(exception(env)||!result)return -5;
  const auto count=env->GetArrayLength(result);if(exception(env)||count!=3)return -5;
  std::array<jlong,3> values{};env->GetLongArrayRegion(result,0,3,values.data());if(exception(env))return -5;
  if(values[0]<0&&values[0]>=-5)return static_cast<int32_t>(values[0]);
  if(values[0]!=0||values[1]<=0||values[2]<-1||values[2]==0||values[2]>static_cast<jlong>(4ULL*1024*1024*1024)){
    if(values[1]>0){env->CallVoidMethod(context.provider,context.close,values[1]);exception(env);}return -5;
  }
  stream->token=static_cast<uint64_t>(values[1]);stream->byte_length=values[2];return 0;
}
int64_t read_media(void* opaque,uint64_t token,uint8_t* output,size_t capacity){
  if(!opaque||token==0||token>INT64_MAX||!output||capacity==0||capacity>kMediaBytes)return -3;
  auto& context=*static_cast<MediaContext*>(opaque);std::lock_guard lock(context.reader);JavaAccess access(context.vm);auto* env=access.env;if(!env)return -5;
  const auto count=env->CallIntMethod(context.provider,context.read,static_cast<jlong>(token),context.buffer,static_cast<jint>(capacity));if(exception(env))return -5;
  if(count<0)return count;if(static_cast<size_t>(count)>capacity)return -5;
  if(count)env->GetByteArrayRegion(context.buffer,0,count,reinterpret_cast<jbyte*>(output));return exception(env)?-5:count;
}
void close_media(void* opaque,uint64_t token){
  if(!opaque||token==0||token>INT64_MAX)return;auto& context=*static_cast<MediaContext*>(opaque);JavaAccess access(context.vm);
  if(access.env){access.env->CallVoidMethod(context.provider,context.close,static_cast<jlong>(token));exception(access.env);}
}
void cancel_media(void* opaque){
  if(!opaque)return;auto& context=*static_cast<MediaContext*>(opaque);JavaAccess access(context.vm);
  if(access.env){access.env->CallVoidMethod(context.provider,context.cancel);exception(access.env);}
}
int32_t check_credentials(void* opaque,const uint8_t* profile){
  if(!opaque||!profile)return -3;auto& context=*static_cast<CredentialContext*>(opaque);JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_profile(env,profile);if(!identity)return -3;const jint status=env->CallIntMethod(context.provider,context.check,identity);return exception(env)?-3:status;
}
int64_t read_credentials(void* opaque,const uint8_t* profile,uint8_t* output,size_t capacity){
  if(!opaque||!profile||!output||capacity!=kCredentialBytes)return -3;auto& context=*static_cast<CredentialContext*>(opaque);JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_profile(env,profile);if(!identity)return -3;auto bytes=env->NewByteArray(static_cast<jsize>(capacity));if(exception(env)||!bytes)return -3;WipeJavaBytes wipe{env,bytes,static_cast<jsize>(capacity)};
  const jint count=env->CallIntMethod(context.provider,context.read,identity,bytes);if(exception(env))return -3;
  if(count<0)return count;if(count==0||static_cast<size_t>(count)>capacity)return -3;
  env->GetByteArrayRegion(bytes,0,count,reinterpret_cast<jbyte*>(output));return exception(env)?-3:count;
}
int32_t write_credentials(void* opaque,const uint8_t* profile,const uint8_t* secret,size_t length){
  if(!opaque||!profile||!secret||length==0||length>kCredentialBytes)return -3;auto& context=*static_cast<CredentialContext*>(opaque);JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_profile(env,profile);if(!identity)return -3;auto bytes=env->NewByteArray(static_cast<jsize>(length));if(exception(env)||!bytes)return -3;WipeJavaBytes wipe{env,bytes,static_cast<jsize>(length)};
  env->SetByteArrayRegion(bytes,0,static_cast<jsize>(length),reinterpret_cast<const jbyte*>(secret));if(exception(env))return -3;
  const jint status=env->CallIntMethod(context.provider,context.write,identity,bytes);return exception(env)?-3:status;
}
int32_t remove_credentials(void* opaque,const uint8_t* profile){
  if(!opaque||!profile)return -3;auto& context=*static_cast<CredentialContext*>(opaque);JavaAccess access(context.vm);auto* env=access.env;if(!env)return -3;
  auto identity=java_profile(env,profile);if(!identity)return -3;const jint status=env->CallIntMethod(context.provider,context.remove,identity);return exception(env)?-3:status;
}
jbyteArray response_bytes(JNIEnv* env,OmniBuffer response){
  ReleaseBuffer release{response};if(!response.data||response.length>1024*1024)return nullptr;
  auto output=env->NewByteArray(static_cast<jsize>(response.length));if(exception(env)||!output)return nullptr;
  env->SetByteArrayRegion(output,0,static_cast<jsize>(response.length),reinterpret_cast<const jbyte*>(response.data));return exception(env)?nullptr:output;
}
} // namespace

extern "C" JNIEXPORT void JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativeSetForeground(JNIEnv*,jclass,jlong handle,jboolean foreground){om_mobile_set_foreground(static_cast<uint64_t>(handle),foreground?1:0);}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativePairingScope(JNIEnv* env,jclass,jlong handle){
  return response_bytes(env,om_mobile_pairing_scope(static_cast<uint64_t>(handle)));
}
// Dedicated native-UI entry, deliberately separate from renderer nativeCall.
extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativePairingIdentity(JNIEnv* env,jclass,jint operation,jlong handle,jlong revision,jbyteArray input){
  if(handle<=0||revision<0)return nullptr;
  const auto owned=static_cast<uint64_t>(handle);
  switch(operation){
    case 0:return response_bytes(env,om_mobile_pairing_status(owned));
    case 1:return response_bytes(env,om_mobile_pairing_prepare_identity_v1(owned,static_cast<uint64_t>(revision),25000));
    case 2:return response_bytes(env,om_mobile_pairing_review_identity_v1(owned,static_cast<uint64_t>(revision)));
    case 3:{
      if(!input)return nullptr;const auto length=env->GetArrayLength(input);
      if(exception(env)||length<=0||length>128)return nullptr;
      uint8_t bytes[128]{};
      env->GetByteArrayRegion(input,0,length,reinterpret_cast<jbyte*>(bytes));
      if(exception(env))return nullptr;
      return response_bytes(env,om_mobile_pairing_confirm_identity_v1(owned,bytes,static_cast<size_t>(length),25000));
    }
    case 4:return response_bytes(env,om_mobile_pairing_cancel_identity_review(owned));
    default:return nullptr;
  }
}
extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativeAttachPairing(JNIEnv* env,jclass,jlong handle,jobject provider){
  if(!provider)return nullptr;auto context=std::make_unique<CredentialContext>();if(env->GetJavaVM(&context->vm)!=JNI_OK)return nullptr;
  const auto type=env->GetObjectClass(provider);if(exception(env)||!type)return nullptr;
  context->read=env->GetMethodID(type,"read","([B[B)I");if(exception(env)||!context->read)return nullptr;
  context->write=env->GetMethodID(type,"write","([B[B)I");if(exception(env)||!context->write)return nullptr;
  context->remove=env->GetMethodID(type,"remove","([B)I");if(exception(env)||!context->remove)return nullptr;
  context->check=env->GetMethodID(type,"check","([B)I");if(exception(env)||!context->check)return nullptr;
  env->DeleteLocalRef(type);context->provider=env->NewGlobalRef(provider);if(exception(env)||!context->provider)return nullptr;
  OmniPairingCredentialCallbacks callbacks{1,context.release(),read_pairing,write_pairing,remove_pairing,check_pairing,release_credentials};
  // Rust consumes this retained object on every attach return. No retry with
  // the same native context; no Java object/secret appears in renderer protocol.
  return response_bytes(env,om_mobile_attach_pairing_credentials_v1(static_cast<uint64_t>(handle),callbacks));
}
extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativePairingRecovery(JNIEnv* env,jclass,jlong handle,jbyteArray input,jint budget){
  if(handle<=0||!input||budget<1000||budget>25000)return nullptr;
  const auto length=env->GetArrayLength(input);if(exception(env)||length<=0||length>1024)return nullptr;
  uint8_t bytes[1024]{};
  env->GetByteArrayRegion(input,0,length,reinterpret_cast<jbyte*>(bytes));if(exception(env))return nullptr;
  return response_bytes(env,om_mobile_pairing_advance_metadata_v1(static_cast<uint64_t>(handle),bytes,static_cast<size_t>(length),static_cast<uint32_t>(budget)));
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativeOpen(JNIEnv* env,jclass,jbyteArray input,jobject provider){
  if(!input||!provider)return nullptr;const jsize length=env->GetArrayLength(input);if(exception(env)||length<=0||length>1024*1024)return nullptr;
  std::vector<uint8_t> bytes(static_cast<size_t>(length));WipeBytes wipe{bytes};env->GetByteArrayRegion(input,0,length,reinterpret_cast<jbyte*>(bytes.data()));if(exception(env))return nullptr;
  auto context=std::make_unique<CredentialContext>();if(env->GetJavaVM(&context->vm)!=JNI_OK)return nullptr;
  const auto type=env->GetObjectClass(provider);if(exception(env)||!type)return nullptr;
  context->read=env->GetMethodID(type,"read","([B[B)I");if(exception(env)||!context->read)return nullptr;
  context->write=env->GetMethodID(type,"write","([B[B)I");if(exception(env)||!context->write)return nullptr;
  context->remove=env->GetMethodID(type,"remove","([B)I");if(exception(env)||!context->remove)return nullptr;
  context->check=env->GetMethodID(type,"check","([B)I");if(exception(env)||!context->check)return nullptr;
  env->DeleteLocalRef(type);context->provider=env->NewGlobalRef(provider);if(exception(env)||!context->provider)return nullptr;
  OmniCredentialCallbacks callbacks{2,context.release(),read_credentials,write_credentials,remove_credentials,release_credentials,check_credentials};
  // Rust consumes the retained provider even on a rejected/duplicate profile.
  return response_bytes(env,om_mobile_open_with_credentials_v2(bytes.data(),bytes.size(),callbacks));
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativeAuthorizeMedia(JNIEnv* env,jclass,jlong handle,jbyteArray input,jobject provider){
  if(!input||!provider)return nullptr;const jsize length=env->GetArrayLength(input);if(exception(env)||length<=0||length>1024*1024)return nullptr;
  std::vector<uint8_t> bytes(static_cast<size_t>(length));WipeBytes wipe{bytes};env->GetByteArrayRegion(input,0,length,reinterpret_cast<jbyte*>(bytes.data()));if(exception(env))return nullptr;
  auto context=std::make_unique<MediaContext>();if(env->GetJavaVM(&context->vm)!=JNI_OK)return nullptr;
  const auto type=env->GetObjectClass(provider);if(exception(env)||!type)return nullptr;
  context->open=env->GetMethodID(type,"open","([B[B)[J");if(exception(env)||!context->open)return nullptr;
  context->read=env->GetMethodID(type,"read","(J[BI)I");if(exception(env)||!context->read)return nullptr;
  context->close=env->GetMethodID(type,"close","(J)V");if(exception(env)||!context->close)return nullptr;
  context->cancel=env->GetMethodID(type,"cancel","()V");if(exception(env)||!context->cancel)return nullptr;
  context->release=env->GetMethodID(type,"release","()V");if(exception(env)||!context->release)return nullptr;
  context->retain=env->GetMethodID(type,"retainForRust","()Z");if(exception(env)||!context->retain)return nullptr;
  env->DeleteLocalRef(type);auto buffer=env->NewByteArray(static_cast<jsize>(kMediaBytes));if(exception(env)||!buffer)return nullptr;
  context->provider=env->NewGlobalRef(provider);if(exception(env)||!context->provider)return nullptr;
  context->buffer=static_cast<jbyteArray>(env->NewGlobalRef(buffer));if(exception(env)||!context->buffer){env->DeleteGlobalRef(context->provider);return nullptr;}
  const auto retained=env->CallBooleanMethod(provider,context->retain);
  if(exception(env)||!retained){env->DeleteGlobalRef(context->buffer);env->DeleteGlobalRef(context->provider);return nullptr;}
  OmniMediaCallbacks callbacks{1,context.release(),open_media,read_media,close_media,cancel_media,release_media};
  // Every Rust admission path consumes the provider; Java release is idempotent.
  return response_bytes(env,om_mobile_authorize_media(static_cast<uint64_t>(handle),bytes.data(),bytes.size(),callbacks));
}

// Deliberately separate from Chromium's browser JNI registration. No JNI method
// accepts a file path except native-owned authorization JSON from SAF delegates.
extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativeCall(
    JNIEnv* env, jclass, jint operation, jlong handle, jbyteArray input) {
  if (!input) return nullptr;
  const jsize length = env->GetArrayLength(input);
  if (length < 0 || length > 1024 * 1024) return nullptr;
  std::vector<uint8_t> bytes(static_cast<size_t>(length));
  WipeBytes wipe{bytes};
  if (length) env->GetByteArrayRegion(input, 0, length, reinterpret_cast<jbyte*>(bytes.data()));
  if (exception(env)) return nullptr;
  OmniBuffer response{};
  switch (operation) {
    case 0: response = om_mobile_open(bytes.data(), bytes.size()); break;
    case 1: response = om_mobile_request(static_cast<uint64_t>(handle), bytes.data(), bytes.size()); break;
    case 2: response = om_mobile_authorize(static_cast<uint64_t>(handle), bytes.data(), bytes.size()); break;
    case 3: response = om_mobile_suspend(static_cast<uint64_t>(handle)); break;
    case 4: response = om_mobile_detach(static_cast<uint64_t>(handle)); break;
    case 5: response = om_mobile_expire_capabilities(static_cast<uint64_t>(handle)); break;
    case 6: response = om_mobile_background_policy(static_cast<uint64_t>(handle)); break;
    default: return nullptr;
  }
  return response_bytes(env,response);
}

extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativeBackgroundStart(JNIEnv* env,jclass,jlong handle,jint budget){
  if(budget<1000||budget>25000)return nullptr;
  return response_bytes(env,om_mobile_background_start(static_cast<uint64_t>(handle),static_cast<uint32_t>(budget)));
}
extern "C" JNIEXPORT jbyteArray JNICALL
Java_org_chromium_chrome_browser_omniplay_NativeCore_nativeBackgroundStatus(JNIEnv* env,jclass,jlong handle,jbyteArray input,jboolean cancel){
  if(!input||env->GetArrayLength(input)!=36)return nullptr;
  uint8_t bytes[36];env->GetByteArrayRegion(input,0,36,reinterpret_cast<jbyte*>(bytes));if(exception(env))return nullptr;
  return response_bytes(env,om_mobile_background_status(static_cast<uint64_t>(handle),bytes,36,cancel?1:0));
}
