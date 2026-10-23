#import "VLKMacEncoder.h"
#import <VideoToolbox/VideoToolbox.h>
#import <CoreVideo/CoreVideo.h>
#include <mach/mach_time.h>
#include <atomic>
#include <deque>
#include <memory>
#include <mutex>
#include <utility>

namespace {
uint64_t continuousNow() {
  mach_timebase_info_data_t base{};
  if (mach_timebase_info(&base) != KERN_SUCCESS || !base.denom) return UINT64_MAX;
  const __uint128_t value = static_cast<__uint128_t>(mach_continuous_time()) * base.numer / base.denom;
  return value > UINT64_MAX ? UINT64_MAX : static_cast<uint64_t>(value);
}
BOOL fail(NSError **error, NSInteger code) {
  if (error) *error = [NSError errorWithDomain:@"vn.vlike.encoder" code:code
      userInfo:@{NSLocalizedDescriptionKey:@"The native video encoder is unavailable, expired or backlogged."}];
  return NO;
}
struct Encoded {
  CMSampleBufferRef sample;
  size_t bytes;
  uint64_t received;
  Encoded(CMSampleBufferRef value, size_t size) : sample(value), bytes(size), received(continuousNow()) { CFRetain(sample); }
  Encoded(const Encoded&) = delete;
  Encoded& operator=(const Encoded&) = delete;
  Encoded(Encoded&& other) noexcept : sample(std::exchange(other.sample,nullptr)), bytes(other.bytes), received(other.received) {}
  ~Encoded() { if (sample) CFRelease(sample); }
};
struct Output {
  std::mutex mutex;
  bool closed = false;
  unsigned pending = 0;
  size_t bytes = 0;
  uint64_t deadline;
  CMTime last = kCMTimeInvalid;
  std::deque<Encoded> queue;
  explicit Output(uint64_t expires) : deadline(expires) {}
  void closeLocked() { closed = true; queue.clear(); bytes = 0; }
};
struct Ticket {
  std::atomic<bool> completed{false};
  uint64_t submitted = continuousNow();
};
bool numeric(CMTime value) { return CMTIME_IS_NUMERIC(value) && value.epoch == 0 && value.value >= 0; }
bool syncSample(CMSampleBufferRef sample) {
  CFArrayRef attachments = CMSampleBufferGetSampleAttachmentsArray(sample,false);
  if (!attachments || CFArrayGetCount(attachments) != 1) return false;
  auto dictionary = static_cast<CFDictionaryRef>(CFArrayGetValueAtIndex(attachments,0));
  CFTypeRef notSync=CFDictionaryGetValue(dictionary,kCMSampleAttachmentKey_NotSync);
  return !notSync || CFEqual(notSync,kCFBooleanFalse);
}
void finish(const std::shared_ptr<Output>& output, const std::shared_ptr<Ticket>& ticket,
    OSStatus status, VTEncodeInfoFlags flags, CMSampleBufferRef sample,
    CMVideoCodecType codec, uint32_t width, uint32_t height, bool requireSync) {
  if (ticket->completed.exchange(true)) return;
  std::lock_guard lock(output->mutex);
  if (output->pending) --output->pending;
  if (output->closed) return;
  uint64_t now = continuousNow();
  if (status != noErr || (flags & kVTEncodeInfo_FrameDropped) || !sample ||
      now >= output->deadline || now < ticket->submitted || now-ticket->submitted > 100000000 ||
      !CMSampleBufferDataIsReady(sample) || CMSampleBufferGetNumSamples(sample) != 1) {
    output->closeLocked(); return;
  }
  auto format = CMSampleBufferGetFormatDescription(sample);
  CMTime pts = CMSampleBufferGetPresentationTimeStamp(sample);
  size_t bytes = CMSampleBufferGetTotalSampleSize(sample);
  if (!format || CMFormatDescriptionGetMediaType(format) != kCMMediaType_Video ||
      CMFormatDescriptionGetMediaSubType(format) != codec || !numeric(pts) ||
      (numeric(output->last) && CMTimeCompare(pts,output->last) <= 0) ||
      !bytes || bytes > 2*1024*1024 || output->queue.size() >= 4 ||
      output->bytes + bytes > 4*1024*1024 || (requireSync && !syncSample(sample))) {
    output->closeLocked(); return;
  }
  auto dimensions = CMVideoFormatDescriptionGetDimensions(format);
  if (dimensions.width != static_cast<int32_t>(width) || dimensions.height != static_cast<int32_t>(height)) {
    output->closeLocked(); return;
  }
  output->last = pts; output->bytes += bytes;
  output->queue.emplace_back(sample,bytes);
}
}

@interface VLKMacEncoder () {
  VLKEncoderConfiguration _configuration;
  VTCompressionSessionRef _session;
  std::shared_ptr<Output> _output;
  CMTime _lastInput;
  BOOL _hardwareAccelerated;
}
@end
@implementation VLKMacEncoder
- (instancetype)initWithConfiguration:(VLKEncoderConfiguration)config error:(NSError **)error {
  if (!(self = [super init])) return nil;
  _configuration = config; _lastInput = kCMTimeInvalid;
  uint64_t now = continuousNow();
  const auto& p = config.profile;
  if (!((p.width==1280 && p.height==720)||(p.width==1920 && p.height==1080)) ||
      !(p.fps==30 || p.fps==60) || config.bitrate<500000 || config.bitrate>30000000 ||
      !(config.codec==VLKVideoH264 || config.codec==VLKVideoHEVC) ||
      config.deadlineNanoseconds<=now || config.deadlineNanoseconds-now>1800000000000ULL) {
    fail(error,1); return nil;
  }
  _output = std::make_shared<Output>(config.deadlineNanoseconds);
  NSDictionary *spec = config.requireHardware ?
      @{(__bridge NSString *)kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder:@YES} : @{};
  NSDictionary *pixels = @{(__bridge NSString *)kCVPixelBufferPixelFormatTypeKey:@(kCVPixelFormatType_32BGRA),
      (__bridge NSString *)kCVPixelBufferWidthKey:@(p.width),(__bridge NSString *)kCVPixelBufferHeightKey:@(p.height)};
  CMVideoCodecType codec = config.codec==VLKVideoH264 ? kCMVideoCodecType_H264 : kCMVideoCodecType_HEVC;
  OSStatus status = VTCompressionSessionCreate(kCFAllocatorDefault,p.width,p.height,codec,
      (__bridge CFDictionaryRef)spec,(__bridge CFDictionaryRef)pixels,nullptr,nullptr,nullptr,&_session);
  if (status!=noErr || !_session) { fail(error,2); return nil; }
  NSDictionary *properties = @{
      (__bridge NSString *)kVTCompressionPropertyKey_RealTime:@YES,
      (__bridge NSString *)kVTCompressionPropertyKey_AllowFrameReordering:@NO,
      (__bridge NSString *)kVTCompressionPropertyKey_AverageBitRate:@(config.bitrate),
      (__bridge NSString *)kVTCompressionPropertyKey_ExpectedFrameRate:@(p.fps),
      (__bridge NSString *)kVTCompressionPropertyKey_MaxKeyFrameInterval:@(p.fps*2),
      (__bridge NSString *)kVTCompressionPropertyKey_MaxFrameDelayCount:@1,
      (__bridge NSString *)kVTCompressionPropertyKey_ProfileLevel:(__bridge NSString *)(
          config.codec==VLKVideoH264 ? kVTProfileLevel_H264_Main_AutoLevel : kVTProfileLevel_HEVC_Main_AutoLevel)};
  status = VTSessionSetProperties(_session,(__bridge CFDictionaryRef)properties);
  if (status==noErr) status=VTCompressionSessionPrepareToEncodeFrames(_session);
  CFTypeRef hardware = nullptr;
  if (status==noErr) status=VTSessionCopyProperty(_session,kVTCompressionPropertyKey_UsingHardwareAcceleratedVideoEncoder,kCFAllocatorDefault,&hardware);
  _hardwareAccelerated = hardware && CFEqual(hardware,kCFBooleanTrue);
  if (hardware) CFRelease(hardware);
  if (status!=noErr || (config.requireHardware && !_hardwareAccelerated) || continuousNow()>=config.deadlineNanoseconds) {
    [self stop]; fail(error,3); return nil;
  }
  return self;
}
- (BOOL)hardwareAccelerated { return _hardwareAccelerated; }
- (BOOL)deliveryClosed {
  if (!_output) return YES;
  std::lock_guard lock(_output->mutex);
  if (continuousNow()>=_output->deadline) _output->closeLocked();
  return _output->closed;
}
- (BOOL)submitSample:(CMSampleBufferRef)sample forceKeyframe:(BOOL)force error:(NSError **)error {
  if (!_session || !sample || !CMSampleBufferDataIsReady(sample) || CMSampleBufferGetNumSamples(sample)!=1) return fail(error,4);
  CVImageBufferRef image=CMSampleBufferGetImageBuffer(sample);
  CMTime pts=CMSampleBufferGetPresentationTimeStamp(sample);
  auto p=_configuration.profile;
  if (!image || !numeric(pts) || (numeric(_lastInput) && CMTimeCompare(pts,_lastInput)<=0) ||
      CVPixelBufferGetPixelFormatType(image)!=kCVPixelFormatType_32BGRA || CVPixelBufferIsPlanar(image) ||
      CVPixelBufferGetWidth(image)!=p.width || CVPixelBufferGetHeight(image)!=p.height ||
      CVPixelBufferGetBytesPerRow(image)<p.width*4 || CVPixelBufferGetBytesPerRow(image)>p.width*16 ||
      CVPixelBufferGetDataSize(image)>static_cast<size_t>(p.width)*16*p.height) return fail(error,5);
  auto output=_output;
  {
    std::lock_guard lock(output->mutex);
    if (output->closed || continuousNow()>=output->deadline) { output->closeLocked(); return fail(error,6); }
    if (output->pending>=2) return fail(error,7);
    ++output->pending;
  }
  bool keyframe=force || !numeric(_lastInput); _lastInput=pts;
  auto ticket=std::make_shared<Ticket>();
  NSDictionary *properties=keyframe ? @{(__bridge NSString *)kVTEncodeFrameOptionKey_ForceKeyFrame:@YES} : @{};
  CMVideoCodecType codec=_configuration.codec==VLKVideoH264 ? kCMVideoCodecType_H264 : kCMVideoCodecType_HEVC;
  VTEncodeInfoFlags flags=0;
  OSStatus status=VTCompressionSessionEncodeFrameWithOutputHandler(_session,image,pts,CMTimeMake(1,p.fps),
      (__bridge CFDictionaryRef)properties,&flags,^(OSStatus result,VTEncodeInfoFlags info,CMSampleBufferRef encoded) {
        finish(output,ticket,result,info,encoded,codec,p.width,p.height,keyframe);
      });
  // Callback may have run synchronously, including on failure. Ticket claims
  // completion once; no manual pixel-buffer ownership is shared with callbacks.
  if (status!=noErr || (flags & kVTEncodeInfo_FrameDropped)) finish(output,ticket,status,flags,nullptr,codec,p.width,p.height,keyframe);
  if (status!=noErr || (flags & kVTEncodeInfo_FrameDropped)) { [self closeDelivery]; return fail(error,8); }
  std::lock_guard lock(output->mutex);
  return output->closed ? fail(error,9) : YES;
}
- (CMSampleBufferRef)copyNextEncodedSample {
  if (!_output) return nullptr;
  std::lock_guard lock(_output->mutex);
  uint64_t now=continuousNow();
  if (_output->closed || now>=_output->deadline) { _output->closeLocked(); return nullptr; }
  if (_output->queue.empty()) return nullptr;
  auto& front=_output->queue.front();
  if (now<front.received || now-front.received>100000000) { _output->closeLocked(); return nullptr; }
  CMSampleBufferRef sample=std::exchange(front.sample,nullptr);
  _output->bytes-=front.bytes; _output->queue.pop_front(); return sample;
}
- (void)closeDelivery {
  if (_output) { std::lock_guard lock(_output->mutex); _output->closeLocked(); }
}
- (void)stop {
  [self closeDelivery];
  if (_session) { VTCompressionSessionInvalidate(_session); CFRelease(_session); _session=nullptr; }
}
- (void)dealloc { [self stop]; }
@end
