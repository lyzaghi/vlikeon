#import "VLKMacCapture.h"
#include "bounded_samples.hpp"
#import <AppKit/AppKit.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreVideo/CoreVideo.h>
#include <libproc.h>
#include <mach/mach_time.h>
#include <sys/proc.h>
#include <unistd.h>
#include <cmath>
#include <utility>

namespace {
NSError *failure(NSInteger code, NSString *message) {
  return [NSError errorWithDomain:@"vn.vlike.capture" code:code
                         userInfo:@{NSLocalizedDescriptionKey:message}];
}
NSString *boundedLabel(NSString *label, NSString *fallback) {
  if (!label.length) return fallback;
  NSString *plain = [[label componentsSeparatedByCharactersInSet:
      NSCharacterSet.controlCharacterSet] componentsJoinedByString:@" "];
  return [plain substringToIndex:MIN(plain.length,120)];
}
uint64_t nowNanoseconds() {
  mach_timebase_info_data_t timebase{};
  if (mach_timebase_info(&timebase) != KERN_SUCCESS || !timebase.denom) return UINT64_MAX;
  const __uint128_t converted = static_cast<__uint128_t>(mach_continuous_time()) *
      timebase.numer / timebase.denom;
  return converted > UINT64_MAX ? UINT64_MAX : static_cast<uint64_t>(converted);
}
bool liveTarget(const VLKCaptureTarget& target) {
  proc_bsdinfo info{};
  return target.pid > 1 && target.pid != getpid() && target.windowID &&
      target.startMicroseconds < 1000000 &&
      (target.startSeconds || target.startMicroseconds) &&
      nowNanoseconds() < target.deadlineNanoseconds &&
      proc_pidinfo(target.pid, PROC_PIDTBSDINFO, 0, &info, sizeof(info)) == sizeof(info) &&
      info.pbi_pid == static_cast<uint32_t>(target.pid) &&
      info.pbi_ppid == static_cast<uint32_t>(getpid()) && info.pbi_status != SZOMB &&
      info.pbi_start_tvsec == target.startSeconds &&
      info.pbi_start_tvusec == target.startMicroseconds;
}
bool validProfile(const VLKCaptureProfile& profile) {
  return ((profile.width == 1280 && profile.height == 720) ||
          (profile.width == 1920 && profile.height == 1080)) &&
      (profile.fps == 30 || profile.fps == 60);
}
// At most two complete video surfaces and eight <=100ms audio buffers. The
// consumer owns copies it removes; it must maintain its own bounded budget.
class Sample {
  CMSampleBufferRef value_ = nullptr;
public:
  explicit Sample(CMSampleBufferRef value) : value_(value) { CFRetain(value); }
  Sample(const Sample&) = delete;
  Sample& operator=(const Sample&) = delete;
  Sample(Sample&& other) noexcept : value_(std::exchange(other.value_,nullptr)) {}
  Sample& operator=(Sample&& other) noexcept {
    if (this != &other) { if (value_) CFRelease(value_); value_ = std::exchange(other.value_,nullptr); }
    return *this;
  }
  ~Sample() { if (value_) CFRelease(value_); }
  CMSampleBufferRef release() { return std::exchange(value_,nullptr); }
};
using Samples = vlikeon::capture::BoundedSamples<Sample,2,8>;
}

@interface VLKMacCapture () <SCStreamOutput, SCStreamDelegate> {
  VLKCaptureTarget _target;
  VLKCaptureProfile _profile;
  VLKCaptureState _state;
  BOOL _applicationAudioSelected;
  SCStream *_stream;
  dispatch_queue_t _samplesQueue;
  dispatch_source_t _watchdog;
  NSAlert *_confirmation;
  Samples _samples;
  void (^_startCompletion)(NSError *);
  NSMutableArray *_stopCompletions;
  BOOL _stopIssued;
  uint64_t _startupDeadline;
  NSError *_stopError;
}
- (void)finishStart:(NSError * _Nullable)error;
- (void)beginStop;
- (void)finishStop:(NSError * _Nullable)error;
@end

@implementation VLKMacCapture
- (instancetype)initWithTarget:(VLKCaptureTarget)target profile:(VLKCaptureProfile)profile {
  if ((self = [super init])) {
    _target = target; _profile = profile; _state = VLKCaptureCreated;
    _samplesQueue = dispatch_queue_create("vn.vlike.capture.samples", DISPATCH_QUEUE_SERIAL);
    _stopCompletions = [NSMutableArray array];
  }
  return self;
}
- (VLKCaptureState)state { NSAssert([NSThread isMainThread], @"Main queue required"); return _state; }
- (BOOL)applicationAudioSelected { NSAssert([NSThread isMainThread], @"Main queue required"); return _applicationAudioSelected; }
- (void)finishStart:(NSError *)error {
  auto completion = _startCompletion; _startCompletion = nil;
  if (completion) completion(error);
}
- (void)startWithCompletion:(void (^)(NSError *))completion {
  NSAssert([NSThread isMainThread], @"Main queue required");
  if (_state != VLKCaptureCreated) { completion(failure(1,@"Capture has already been used.")); return; }
  const uint64_t now = nowNanoseconds();
  if (!validProfile(_profile) || !liveTarget(_target) ||
      _target.deadlineNanoseconds - now > 30ULL * 60 * 1000000000) {
    _state = VLKCaptureStopped; completion(failure(2,@"The owned game or capture lifetime is invalid.")); return;
  }
  if (!CGPreflightScreenCaptureAccess()) {
    _state = VLKCaptureStopped; completion(failure(3,@"Screen Recording permission is required.")); return;
  }
  _state = VLKCaptureSelecting; _startCompletion = [completion copy];
  _startupDeadline = now + 45ULL * 1000000000;
  __weak VLKMacCapture *weakSelf = self;
  _watchdog = dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER,0,0,dispatch_get_main_queue());
  dispatch_source_set_timer(_watchdog,dispatch_time(DISPATCH_TIME_NOW,0),100*NSEC_PER_MSEC,10*NSEC_PER_MSEC);
  dispatch_source_set_event_handler(_watchdog,^{
    VLKMacCapture *strong = weakSelf;
    if (strong && (!liveTarget(strong->_target) ||
        (strong->_state != VLKCaptureRunning && nowNanoseconds() >= strong->_startupDeadline))) [strong beginStop];
  });
  dispatch_resume(_watchdog);
  [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:YES
      completionHandler:^(SCShareableContent *content, NSError *error) {
    dispatch_async(dispatch_get_main_queue(),^{
      VLKMacCapture *strong = weakSelf;
      if (!strong || strong->_state != VLKCaptureSelecting) return;
      if (error || !liveTarget(strong->_target)) {
        [strong finishStart:failure(4,@"The game window is unavailable.")]; [strong beginStop]; return;
      }
      SCWindow *selected = nil;
      for (SCWindow *window in content.windows) {
        if (window.windowID == strong->_target.windowID &&
            window.owningApplication.processID == strong->_target.pid &&
            window.isOnScreen && window.windowLayer == 0 &&
            std::isfinite(window.frame.size.width) && std::isfinite(window.frame.size.height) &&
            window.frame.size.width > 0 && window.frame.size.height > 0) { selected = window; break; }
      }
      if (!selected) { [strong finishStart:failure(4,@"The owned game window is unavailable.")]; [strong beginStop]; return; }
      NSAlert *alert = [NSAlert new]; strong->_confirmation = alert;
      alert.messageText = @"Share this game window?";
      NSString *name = boundedLabel(selected.owningApplication.applicationName,@"Game");
      NSString *title = boundedLabel(selected.title,@"Untitled window");
      alert.informativeText = [NSString stringWithFormat:@"Application: %@\nWindow: %@\n\nShare this window with the paired viewer. Stop sharing from Vlikeon at any time.",name,title];
      [alert addButtonWithTitle:@"Share Window"]; [alert addButtonWithTitle:@"Cancel"];
      if (strong->_profile.offerApplicationAudio) {
        alert.showsSuppressionButton = YES;
        alert.suppressionButton.title = @"Also share audio from this application";
        alert.suppressionButton.state = NSControlStateValueOff;
      }
      const NSModalResponse response = [alert runModal];
      strong->_confirmation = nil;
      if (strong->_state != VLKCaptureSelecting) return;
      if (response != NSAlertFirstButtonReturn || !liveTarget(strong->_target)) {
        [strong finishStart:failure(5,@"Sharing was cancelled or the game ended.")]; [strong beginStop]; return;
      }
      strong->_applicationAudioSelected = strong->_profile.offerApplicationAudio && alert.suppressionButton.state == NSControlStateValueOn;
      SCContentFilter *filter = [[SCContentFilter alloc] initWithDesktopIndependentWindow:selected];
      SCStreamConfiguration *configuration = [SCStreamConfiguration new];
      configuration.width = strong->_profile.width; configuration.height = strong->_profile.height;
      configuration.minimumFrameInterval = CMTimeMake(1,strong->_profile.fps);
      configuration.pixelFormat = kCVPixelFormatType_32BGRA; configuration.queueDepth = 3;
      configuration.showsCursor = YES; configuration.capturesAudio = strong->_applicationAudioSelected;
      configuration.excludesCurrentProcessAudio = YES; configuration.sampleRate = 48000; configuration.channelCount = 2;
      SCStream *stream = [[SCStream alloc] initWithFilter:filter configuration:configuration delegate:strong];
      NSError *outputError = nil;
      if (![stream addStreamOutput:strong type:SCStreamOutputTypeScreen sampleHandlerQueue:strong->_samplesQueue error:&outputError] ||
          (strong->_applicationAudioSelected && ![stream addStreamOutput:strong type:SCStreamOutputTypeAudio sampleHandlerQueue:strong->_samplesQueue error:&outputError])) {
        [strong finishStart:failure(6,@"Capture outputs could not be configured.")]; [strong beginStop]; return;
      }
      strong->_stream = stream; strong->_state = VLKCaptureStarting;
      [stream startCaptureWithCompletionHandler:^(NSError *startError) {
        dispatch_async(dispatch_get_main_queue(),^{
          VLKMacCapture *active = weakSelf;
          if (!active) { [stream stopCaptureWithCompletionHandler:^(NSError *){}]; return; }
          if (active->_state != VLKCaptureStarting) {
            [stream stopCaptureWithCompletionHandler:^(NSError *){}]; return;
          }
          if (startError || !liveTarget(active->_target)) {
            [active finishStart:failure(7,@"Capture could not start for the owned game.")]; [active beginStop]; return;
          }
          if (!active->_samples.open()) { [active beginStop]; return; }
          active->_state = VLKCaptureRunning; [active finishStart:nil];
        });
      }];
    });
  }];
}
- (void)stopWithCompletion:(void (^)(NSError *))completion {
  NSAssert([NSThread isMainThread], @"Main queue required");
  if (_state == VLKCaptureStopped) { completion(nil); return; }
  if (_state == VLKCaptureStopping && _stopError) { completion(_stopError); return; }
  [_stopCompletions addObject:[completion copy]]; [self beginStop];
}
- (void)beginStop {
  if (_state == VLKCaptureStopping || _state == VLKCaptureStopped) return;
  _state = VLKCaptureStopping; _samples.close();
  if (_watchdog) { dispatch_source_cancel(_watchdog); _watchdog = nil; }
  if (_confirmation && NSApp.modalWindow == _confirmation.window) { [NSApp abortModal]; [_confirmation.window orderOut:nil]; }
  [self finishStart:failure(5,@"Capture was stopped.")];
  if (!_stream) { [self finishStop:nil]; return; }
  _stopIssued = YES;
  [ _stream stopCaptureWithCompletionHandler:^(NSError *error) {
    dispatch_async(dispatch_get_main_queue(),^{ [self finishStop:error ? failure(8,@"Capture stop was not acknowledged.") : nil]; });
  }];
}
- (void)finishStop:(NSError *)error {
  // A failed OS stop is not success and cannot be restarted/reported stopped.
  if (_state != VLKCaptureStopping) return;
  _stopError = error;
  if (!error) { _stream = nil; _state = VLKCaptureStopped; }
  NSArray *completions = [_stopCompletions copy]; [_stopCompletions removeAllObjects];
  for (void (^completion)(NSError *) in completions) completion(error);
}
- (void)stream:(SCStream *)stream didStopWithError:(NSError *)error {
  dispatch_async(dispatch_get_main_queue(),^{
    if (stream != self->_stream) return;
    // Delegate confirms source termination, including target-window loss.
    self->_samples.close();
    if (self->_watchdog) { dispatch_source_cancel(self->_watchdog); self->_watchdog = nil; }
    self->_state = VLKCaptureStopping; [self finishStart:failure(9,@"The capture source ended.")];
    [self finishStop:nil];
  });
}
- (void)stream:(SCStream *)stream didOutputSampleBuffer:(CMSampleBufferRef)sample ofType:(SCStreamOutputType)type {
  (void)stream;
  if (!CMSampleBufferIsValid(sample) || !CMSampleBufferDataIsReady(sample) || !liveTarget(_target)) return;
  if (type == SCStreamOutputTypeScreen) {
    NSArray *attachments = (__bridge NSArray *)CMSampleBufferGetSampleAttachmentsArray(sample,false);
    NSNumber *status = attachments.firstObject[SCStreamFrameInfoStatus];
    CVPixelBufferRef image = CMSampleBufferGetImageBuffer(sample);
    if (!status || status.integerValue != SCFrameStatusComplete || !image ||
        CVPixelBufferGetPixelFormatType(image) != kCVPixelFormatType_32BGRA ||
        CVPixelBufferGetWidth(image) != _profile.width || CVPixelBufferGetHeight(image) != _profile.height ||
        CVPixelBufferIsPlanar(image) || CVPixelBufferGetBytesPerRow(image) < _profile.width * 4 ||
        CVPixelBufferGetBytesPerRow(image) > _profile.width * 16 ||
        CVPixelBufferGetDataSize(image) > static_cast<size_t>(_profile.width) * 16 * _profile.height) return;
    _samples.push(Sample(sample),false);
  } else if (type == SCStreamOutputTypeAudio && _applicationAudioSelected) {
    const auto format = CMSampleBufferGetFormatDescription(sample);
    const AudioStreamBasicDescription *audio = format ? CMAudioFormatDescriptionGetStreamBasicDescription(format) : nullptr;
    const CMItemCount count = CMSampleBufferGetNumSamples(sample);
    if (!audio || audio->mFormatID != kAudioFormatLinearPCM || audio->mSampleRate != 48000 ||
        audio->mChannelsPerFrame != 2 || audio->mBitsPerChannel != 32 ||
        !(audio->mFormatFlags & kAudioFormatFlagIsFloat) || count <= 0 || count > 4800 ||
        CMSampleBufferGetTotalSampleSize(sample) < static_cast<size_t>(count) * 8 ||
        CMSampleBufferGetTotalSampleSize(sample) > 65536) return;
    _samples.push(Sample(sample),true);
  }
}
- (CMSampleBufferRef)copyNextVideoSample { auto sample = _samples.take(false); return sample ? sample->release() : nullptr; }
- (CMSampleBufferRef)copyNextAudioSample { auto sample = _samples.take(true); return sample ? sample->release() : nullptr; }
- (void)dealloc {
  _samples.close();
  if (_watchdog) dispatch_source_cancel(_watchdog);
  if (_stream && !_stopIssued) [_stream stopCaptureWithCompletionHandler:^(NSError *){}];
}
@end
