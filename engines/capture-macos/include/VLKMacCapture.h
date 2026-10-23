#pragma once
#import <Foundation/Foundation.h>
#import <CoreMedia/CoreMedia.h>
#include <stdint.h>

NS_ASSUME_NONNULL_BEGIN

// PRIVATE HOST API. Construct only while the broker still owns its unreaped
// directly launched child. Never accept these fields from a renderer/peer,
// persisted profile or process enumeration. Launcher descendants are separate.
typedef struct {
  int32_t pid;
  uint64_t startSeconds;
  uint64_t startMicroseconds;
  uint32_t windowID;
  // mach continuous-time nanoseconds, issued by the host before native consent.
  uint64_t deadlineNanoseconds;
} VLKCaptureTarget;

typedef struct {
  uint32_t width;   // 1280x720 or 1920x1080
  uint32_t height;
  uint32_t fps;     // 30 or 60
  BOOL offerApplicationAudio; // checkbox is still initially unchecked
} VLKCaptureProfile;

typedef NS_ENUM(NSInteger, VLKCaptureState) {
  VLKCaptureCreated, VLKCaptureSelecting, VLKCaptureStarting,
  VLKCaptureRunning, VLKCaptureStopping, VLKCaptureStopped
};

// All state/completion calls occur on the main queue. Media is polled through
// the copy methods from any thread. Each returned buffer has +1 ownership;
// the caller must CFRelease it. No file/network destination is accepted.
// Host MUST stop on originating document, authenticated pair or owner loss.
API_AVAILABLE(macos(13.0))
@interface VLKMacCapture : NSObject
- (instancetype)initWithTarget:(VLKCaptureTarget)target
                       profile:(VLKCaptureProfile)profile NS_DESIGNATED_INITIALIZER;
- (instancetype)init NS_UNAVAILABLE;
// Requires the normal macOS Screen Recording permission before entry.
// Presents a local native confirmation for the exact resolved window.
- (void)startWithCompletion:(void (^)(NSError * _Nullable error))completion;
// Prevents delivery immediately; completion reports actual SCStream stop ACK.
// A stalled OS stop is not reported as successful teardown.
- (void)stopWithCompletion:(void (^)(NSError * _Nullable error))completion;
- (nullable CMSampleBufferRef)copyNextVideoSample CF_RETURNS_RETAINED;
- (nullable CMSampleBufferRef)copyNextAudioSample CF_RETURNS_RETAINED;
@property(nonatomic, readonly) VLKCaptureState state;
@property(nonatomic, readonly) BOOL applicationAudioSelected;
@end

NS_ASSUME_NONNULL_END
