#pragma once
#import <Foundation/Foundation.h>
#import <CoreMedia/CoreMedia.h>
#import "VLKMacCapture.h"

NS_ASSUME_NONNULL_BEGIN
typedef NS_ENUM(NSInteger, VLKVideoCodec) { VLKVideoH264, VLKVideoHEVC };
typedef struct {
  VLKCaptureProfile profile;
  VLKVideoCodec codec;
  uint32_t bitrate;
  BOOL requireHardware;
  // Host-supplied original grant deadline in mach continuous nanoseconds.
  uint64_t deadlineNanoseconds;
} VLKEncoderConfiguration;

// Private adapter, not a permission API or standalone supervised service.
// Init/submit/stop must run on one dedicated helper queue, never the UI queue.
// The admitted host checks owner/pair/capture authority before each submission
// and publication, and calls closeDelivery immediately on loss of authority.
API_AVAILABLE(macos(13.0))
@interface VLKMacEncoder : NSObject
- (nullable instancetype)initWithConfiguration:(VLKEncoderConfiguration)configuration
                                         error:(NSError * _Nullable * _Nullable)error NS_DESIGNATED_INITIALIZER;
- (instancetype)init NS_UNAVAILABLE;
// Borrows a complete BGRA sample from VLKMacCapture for this call. Never mutate
// its pixels after submission. A full two-frame encoder window returns NO;
// the caller may discard that raw input, not an encoded dependent frame.
- (BOOL)submitSample:(CMSampleBufferRef)sample forceKeyframe:(BOOL)forceKeyframe
                error:(NSError * _Nullable * _Nullable)error;
// Thread-safe, +1 sample ownership. Carries the actual CMFormatDescription
// (parameter sets/length-prefixed bitstream), not an RTP/Annex-B conversion.
- (nullable CMSampleBufferRef)copyNextEncodedSample CF_RETURNS_RETAINED;
// Thread-safe output fence. Does not wait for or claim OS teardown.
- (void)closeDelivery;
// Invalidates and releases the OS session on the helper queue. May block in OS;
// the future process supervisor must bound that wait externally.
- (void)stop;
@property(nonatomic, readonly) BOOL hardwareAccelerated;
// Thread-safe: lets the consumer distinguish an empty queue from a terminal
// output fault. A terminal encoder must be stopped, never silently reopened.
@property(nonatomic, readonly) BOOL deliveryClosed;
@end
NS_ASSUME_NONNULL_END
