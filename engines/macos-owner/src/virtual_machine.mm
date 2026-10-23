#import <Foundation/Foundation.h>
#import <Virtualization/Virtualization.h>
#include "virtual_machine.hpp"
#include <fcntl.h>
#include <unistd.h>
#include <utility>

namespace omniplay::macos {
struct VirtualMachine::State {
  dispatch_queue_t queue = dispatch_queue_create("vlikeon.guest.owner", DISPATCH_QUEUE_SERIAL);
  VZVirtualMachine* machine = nil;
  id<VZVirtualMachineDelegate> delegate = nil;
  std::atomic<bool> stopped{false}, failure{false}, stopping{false};
  std::atomic<bool> stop_queued{false};
  bool starting = false;
  bool stop_in_flight = false;
};
}

@interface OPGuestDelegate : NSObject <VZVirtualMachineDelegate>
@property(nonatomic) std::weak_ptr<omniplay::macos::VirtualMachine::State> state;
@end

namespace omniplay::macos {
namespace {
using State = VirtualMachine::State;
// Called only on State::queue. Release all VZ device/configuration owners before
// publishing a termination receipt; a failed stop retains capacity for retry.
void retired(const std::shared_ptr<State>& state, bool failure) {
  state->failure.store(state->failure.load() || failure);
  state->machine.delegate = nil;
  state->machine = nil;
  state->delegate = nil;
  state->stopped.store(true);
}
void stop_on_queue(const std::shared_ptr<State>& state) {
  if (state->stopped || state->starting || state->stop_in_flight) return;
  if (!state->machine || state->machine.state == VZVirtualMachineStateStopped ||
      state->machine.state == VZVirtualMachineStateError) { retired(state, false); return; }
  if (!state->machine.canStop) return;
  state->stop_in_flight = true;
  [state->machine stopWithCompletionHandler:^(NSError* error) {
    state->stop_in_flight = false;
    if (!error) retired(state, false);
    else state->failure.store(true);
  }];
}
NSFileHandle* duplicate(int descriptor) {
  int copy = fcntl(descriptor, F_DUPFD_CLOEXEC, 10);
  return copy < 0 ? nil : [[NSFileHandle alloc] initWithFileDescriptor:copy closeOnDealloc:YES];
}
VZVirtioConsolePortConfiguration* port(NSString* name, int input, int output) {
  NSFileHandle* read = duplicate(input);
  NSFileHandle* write = duplicate(output);
  if (!read || !write) return nil;
  VZVirtioConsolePortConfiguration* result = [VZVirtioConsolePortConfiguration new];
  result.name = name;
  result.isConsole = NO;
  result.attachment = [[VZFileHandleSerialPortAttachment alloc]
      initWithFileHandleForReading:read fileHandleForWriting:write];
  return result;
}
bool session_id(const std::string& value) {
  if (value.size() != 36) return false;
  for (size_t i = 0; i < value.size(); ++i) {
    if (i == 8 || i == 13 || i == 18 || i == 23) { if (value[i] != '-') return false; }
    else if (!((value[i] >= '0' && value[i] <= '9') || (value[i] >= 'a' && value[i] <= 'f'))) return false;
  }
  return true;
}
}

VirtualMachine::VirtualMachine(std::shared_ptr<State> state) : state_(std::move(state)) {}
VirtualMachine::~VirtualMachine() { stop(); }
bool VirtualMachine::ended() const { return state_->stopped.load(); }
bool VirtualMachine::failed() const { return state_->failure.load(); }
void VirtualMachine::stop() {
  auto state = state_;
  state->stopping.store(true);
  if (state->stopped || state->stop_queued.exchange(true)) return;
  dispatch_async(state->queue, ^{
    state->stop_queued.store(false);
    stop_on_queue(state);
  });
}
std::shared_ptr<VirtualMachine> VirtualMachine::launch(const GuestImage& image,
    const std::filesystem::path& inputs, const std::string& session,
    int control_read, int control_write, int presentation_read, int presentation_write) {
  if (!session_id(session) || !inputs.is_absolute() || image.memory_mb < 256 || image.memory_mb > 8192 ||
      image.cpu_count < 1 || image.cpu_count > 4 || image.scratch_mb < 16 ||
      image.scratch_mb > 512 || image.scratch_mb > image.memory_mb / 2) return {};
  auto state = std::make_shared<State>();
  // Allocate ownership before startup; allocation failure afterward must not
  // leave a running guest outside the session capacity accounting.
  auto result = std::shared_ptr<VirtualMachine>(new VirtualMachine(state));
  __block bool configured = false;
  // Configuration owns duplicated descriptors before returning to the caller.
  dispatch_sync(state->queue, ^{
    @autoreleasepool {
      if (![VZVirtualMachine isSupported]) return;
      auto url = [](const std::string& path) { return [NSURL fileURLWithPath:
          [NSString stringWithUTF8String:path.c_str()]]; };
      VZVirtualMachineConfiguration* config = [VZVirtualMachineConfiguration new];
      config.platform = [VZGenericPlatformConfiguration new];
      config.memorySize = static_cast<uint64_t>(image.memory_mb) * 1024 * 1024;
      config.CPUCount = image.cpu_count;
      VZLinuxBootLoader* boot = [[VZLinuxBootLoader alloc] initWithKernelURL:url(image.kernel)];
      boot.initialRamdiskURL = url(image.initrd);
      boot.commandLine = [NSString stringWithFormat:
          @"rdinit=/init panic=1 quiet vlike.session=%s vlike.scratch_mb=%u", session.c_str(), image.scratch_mb];
      config.bootLoader = boot;
      VZVirtioConsoleDeviceConfiguration* console = [VZVirtioConsoleDeviceConfiguration new];
      auto control = port(@"vlike-control", control_read, control_write);
      auto presentation = port(@"vlike-presentation", presentation_read, presentation_write);
      if (!control || !presentation) return;
      console.ports[0] = control;
      console.ports[1] = presentation;
      config.consoleDevices = @[console];
      VZSharedDirectory* directory = [[VZSharedDirectory alloc] initWithURL:url(inputs.string()) readOnly:YES];
      VZVirtioFileSystemDeviceConfiguration* share = [[VZVirtioFileSystemDeviceConfiguration alloc]
          initWithTag:@"vlike-input"];
      share.share = [[VZSingleDirectoryShare alloc] initWithDirectory:directory];
      config.directorySharingDevices = @[share];
      config.entropyDevices = @[[VZVirtioEntropyDeviceConfiguration new]];
      // Deliberately empty: there is no host-writable disk/share, networking,
      // graphics, audio, USB, Rosetta or input device available to guest code.
      config.storageDevices = @[];
      config.networkDevices = @[];
      config.graphicsDevices = @[];
      config.audioDevices = @[];
      config.keyboards = @[];
      config.pointingDevices = @[];
      NSError* error = nil;
      if (![config validateWithError:&error]) return;
      state->machine = [[VZVirtualMachine alloc] initWithConfiguration:config queue:state->queue];
      OPGuestDelegate* delegate = [OPGuestDelegate new];
      delegate.state = state;
      state->delegate = delegate;
      state->machine.delegate = delegate;
      state->starting = true;
      configured = true;
      [state->machine startWithCompletionHandler:^(NSError* start_error) {
        state->starting = false;
        if (start_error) { retired(state, true); return; }
        if (state->stopping) stop_on_queue(state);
      }];
    }
  });
  if (!configured) return {};
  return result;
}
}

@implementation OPGuestDelegate
- (void)guestDidStopVirtualMachine:(VZVirtualMachine*)machine {
  (void)machine;
  if (auto state = self.state.lock()) omniplay::macos::retired(state, false);
}
- (void)virtualMachine:(VZVirtualMachine*)machine didStopWithError:(NSError*)error {
  (void)machine; (void)error;
  if (auto state = self.state.lock()) omniplay::macos::retired(state, true);
}
@end
