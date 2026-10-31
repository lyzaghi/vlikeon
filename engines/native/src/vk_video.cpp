#include "engine.hpp"
#include "vk_video.hpp"
#include <algorithm>
#include <cmath>
#include <cstring>

namespace omni {
namespace {
constexpr std::uint32_t kVkApiVersion11 = (1u << 22) | (1u << 12);
constexpr int kStructureTypeApplicationInfo = 0;
constexpr int kStructureTypeInstanceCreateInfo = 1;
constexpr int kStructureTypeDeviceQueueCreateInfo = 2;
constexpr int kStructureTypeDeviceCreateInfo = 3;
constexpr int kStructureTypeSubmitInfo = 4;
constexpr int kStructureTypeFenceCreateInfo = 8;
constexpr int kStructureTypeSemaphoreCreateInfo = 9;
constexpr int kStructureTypeCommandPoolCreateInfo = 39;
constexpr int kStructureTypeCommandBufferAllocateInfo = 40;
constexpr int kStructureTypeCommandBufferBeginInfo = 42;
constexpr int kStructureTypeImageMemoryBarrier = 45;
constexpr int kStructureTypeSwapchainCreateInfoKHR = 1000001000;
constexpr int kStructureTypePresentInfoKHR = 1000001001;

struct RawVkApplicationInfo {
  int sType; const void* pNext; const char* pApplicationName; std::uint32_t applicationVersion;
  const char* pEngineName; std::uint32_t engineVersion; std::uint32_t apiVersion;
};
struct RawVkInstanceCreateInfo {
  int sType; const void* pNext; VkFlags flags; const RawVkApplicationInfo* pApplicationInfo;
  std::uint32_t enabledLayerCount; const char* const* ppEnabledLayerNames;
  std::uint32_t enabledExtensionCount; const char* const* ppEnabledExtensionNames;
};
struct RawVkQueueFamilyProperties {
  VkFlags queueFlags; std::uint32_t queueCount; std::uint32_t timestampValidBits;
  std::uint32_t minImageTransferGranularity[3];
};
struct RawVkDeviceQueueCreateInfo {
  int sType; const void* pNext; VkFlags flags; std::uint32_t queueFamilyIndex;
  std::uint32_t queueCount; const float* pQueuePriorities;
};
struct RawVkDeviceCreateInfo {
  int sType; const void* pNext; VkFlags flags; std::uint32_t queueCreateInfoCount;
  const RawVkDeviceQueueCreateInfo* pQueueCreateInfos; std::uint32_t enabledLayerCount;
  const char* const* ppEnabledLayerNames; std::uint32_t enabledExtensionCount;
  const char* const* ppEnabledExtensionNames; const void* pEnabledFeatures;
};
struct RawVkSurfaceCapabilitiesKHR {
  std::uint32_t minImageCount, maxImageCount;
  std::uint32_t currentWidth, currentHeight, minWidth, minHeight, maxWidth, maxHeight;
  std::uint32_t maxImageArrayLayers; VkFlags supportedTransforms, currentTransform;
  VkFlags supportedCompositeAlpha, supportedUsageFlags;
};
struct RawVkSurfaceFormatKHR { int format; int colorSpace; };
struct RawVkSwapchainCreateInfoKHR {
  int sType; const void* pNext; VkFlags flags; VkSurfaceKHR surface;
  std::uint32_t minImageCount; int imageFormat; int imageColorSpace;
  std::uint32_t imageWidth, imageHeight, imageArrayLayers; VkFlags imageUsage;
  int imageSharingMode; std::uint32_t queueFamilyIndexCount; const std::uint32_t* pQueueFamilyIndices;
  VkFlags preTransform, compositeAlpha; int presentMode; VkBool32 clipped; VkSwapchainKHR oldSwapchain;
};
struct RawVkCommandPoolCreateInfo { int sType; const void* pNext; VkFlags flags; std::uint32_t queueFamilyIndex; };
struct RawVkCommandBufferAllocateInfo { int sType; const void* pNext; VkCommandPool commandPool; int level; std::uint32_t commandBufferCount; };
struct RawVkFenceCreateInfo { int sType; const void* pNext; VkFlags flags; };
struct RawVkSemaphoreCreateInfo { int sType; const void* pNext; VkFlags flags; };
struct RawVkCommandBufferBeginInfo { int sType; const void* pNext; VkFlags flags; const void* pInheritanceInfo; };
struct RawVkImageMemoryBarrier {
  int sType; const void* pNext; VkFlags srcAccessMask, dstAccessMask;
  int oldLayout, newLayout; std::uint32_t srcQueueFamilyIndex, dstQueueFamilyIndex;
  VkImage image; VkImageSubresourceRange subresourceRange;
};
struct RawVkImageBlit {
  struct { VkFlags aspectMask; std::uint32_t mipLevel, baseArrayLayer, layerCount; } srcSubresource;
  struct { std::int32_t x, y, z; } srcOffsets[2];
  struct { VkFlags aspectMask; std::uint32_t mipLevel, baseArrayLayer, layerCount; } dstSubresource;
  struct { std::int32_t x, y, z; } dstOffsets[2];
};
struct RawVkClearColorValue { float float32[4]; };
struct RawVkSubmitInfo {
  int sType; const void* pNext; std::uint32_t waitSemaphoreCount; const VkSemaphore* pWaitSemaphores;
  const VkFlags* pWaitDstStageMask; std::uint32_t commandBufferCount; const VkCommandBuffer* pCommandBuffers;
  std::uint32_t signalSemaphoreCount; const VkSemaphore* pSignalSemaphores;
};
struct RawVkPresentInfoKHR {
  int sType; const void* pNext; std::uint32_t waitSemaphoreCount; const VkSemaphore* pWaitSemaphores;
  std::uint32_t swapchainCount; const VkSwapchainKHR* pSwapchains;
  const std::uint32_t* pImageIndices; VkResult* pResults;
};

using FnCreateInstance = VkResult (VKAPI_PTR*)(const RawVkInstanceCreateInfo*, const void*, VkInstance*);
using FnDestroyInstance = void (VKAPI_PTR*)(VkInstance, const void*);
using FnEnumeratePhysicalDevices = VkResult (VKAPI_PTR*)(VkInstance, std::uint32_t*, VkPhysicalDevice*);
using FnGetQueueFamilyProps = void (VKAPI_PTR*)(VkPhysicalDevice, std::uint32_t*, RawVkQueueFamilyProperties*);
using FnGetSurfaceSupportKHR = VkResult (VKAPI_PTR*)(VkPhysicalDevice, std::uint32_t, VkSurfaceKHR, VkBool32*);
using FnGetSurfaceCapsKHR = VkResult (VKAPI_PTR*)(VkPhysicalDevice, VkSurfaceKHR, RawVkSurfaceCapabilitiesKHR*);
using FnGetSurfaceFormatsKHR = VkResult (VKAPI_PTR*)(VkPhysicalDevice, VkSurfaceKHR, std::uint32_t*, RawVkSurfaceFormatKHR*);
using FnCreateDevice = VkResult (VKAPI_PTR*)(VkPhysicalDevice, const RawVkDeviceCreateInfo*, const void*, VkDevice*);
using FnDestroyDevice = void (VKAPI_PTR*)(VkDevice, const void*);
using FnDeviceWaitIdle = VkResult (VKAPI_PTR*)(VkDevice);
using FnGetDeviceQueue = void (VKAPI_PTR*)(VkDevice, std::uint32_t, std::uint32_t, VkQueue*);
using FnCreateSwapchainKHR = VkResult (VKAPI_PTR*)(VkDevice, const RawVkSwapchainCreateInfoKHR*, const void*, VkSwapchainKHR*);
using FnDestroySwapchainKHR = void (VKAPI_PTR*)(VkDevice, VkSwapchainKHR, const void*);
using FnGetSwapchainImagesKHR = VkResult (VKAPI_PTR*)(VkDevice, VkSwapchainKHR, std::uint32_t*, VkImage*);
using FnAcquireNextImageKHR = VkResult (VKAPI_PTR*)(VkDevice, VkSwapchainKHR, std::uint64_t, VkSemaphore, VkFence, std::uint32_t*);
using FnQueuePresentKHR = VkResult (VKAPI_PTR*)(VkQueue, const RawVkPresentInfoKHR*);
using FnQueueSubmit = VkResult (VKAPI_PTR*)(VkQueue, std::uint32_t, const RawVkSubmitInfo*, VkFence);
using FnCreateCommandPool = VkResult (VKAPI_PTR*)(VkDevice, const RawVkCommandPoolCreateInfo*, const void*, VkCommandPool*);
using FnDestroyCommandPool = void (VKAPI_PTR*)(VkDevice, VkCommandPool, const void*);
using FnAllocateCommandBuffers = VkResult (VKAPI_PTR*)(VkDevice, const RawVkCommandBufferAllocateInfo*, VkCommandBuffer*);
using FnBeginCommandBuffer = VkResult (VKAPI_PTR*)(VkCommandBuffer, const RawVkCommandBufferBeginInfo*);
using FnEndCommandBuffer = VkResult (VKAPI_PTR*)(VkCommandBuffer);
using FnResetCommandBuffer = VkResult (VKAPI_PTR*)(VkCommandBuffer, VkFlags);
using FnCmdPipelineBarrier = void (VKAPI_PTR*)(VkCommandBuffer, VkFlags, VkFlags, VkFlags, std::uint32_t, const void*, std::uint32_t, const void*, std::uint32_t, const RawVkImageMemoryBarrier*);
using FnCmdClearColorImage = void (VKAPI_PTR*)(VkCommandBuffer, VkImage, int, const RawVkClearColorValue*, std::uint32_t, const VkImageSubresourceRange*);
using FnCmdBlitImage = void (VKAPI_PTR*)(VkCommandBuffer, VkImage, int, VkImage, int, std::uint32_t, const RawVkImageBlit*, int);
using FnCreateFence = VkResult (VKAPI_PTR*)(VkDevice, const RawVkFenceCreateInfo*, const void*, VkFence*);
using FnDestroyFence = void (VKAPI_PTR*)(VkDevice, VkFence, const void*);
using FnWaitForFences = VkResult (VKAPI_PTR*)(VkDevice, std::uint32_t, const VkFence*, VkBool32, std::uint64_t);
using FnResetFences = VkResult (VKAPI_PTR*)(VkDevice, std::uint32_t, const VkFence*);
using FnCreateSemaphore = VkResult (VKAPI_PTR*)(VkDevice, const RawVkSemaphoreCreateInfo*, const void*, VkSemaphore*);
using FnDestroySemaphore = void (VKAPI_PTR*)(VkDevice, VkSemaphore, const void*);

template<class T> T proc_i(PFN_vkGetInstanceProcAddr gip, VkInstance inst, const char* name) {
  auto fn = reinterpret_cast<T>(gip(inst, name));
  if (!fn) throw Failure("runtime_unavailable");
  return fn;
}
template<class T> T proc_d(PFN_vkGetDeviceProcAddr gdp, VkDevice dev, const char* name) {
  auto fn = reinterpret_cast<T>(gdp(dev, name));
  if (!fn) throw Failure("runtime_unavailable");
  return fn;
}
} // namespace

void VkVideo::configure(SDL_Window* window, unsigned width, unsigned height) {
  if (window_ || !window || width < 16 || height < 16 || width > 4096 || height > 4096 ||
      std::uint64_t(width) * height > 16u * 1024u * 1024u) {
    throw Failure("runtime_crashed");
  }
  window_ = window;
  width_ = width;
  height_ = height;
}

void VkVideo::initialize_device() {
  if (device_) return;
  if (!SDL_Vulkan_LoadLibrary(nullptr)) throw Failure("runtime_unavailable");
  library_loaded_ = true;
  get_instance_proc_ = reinterpret_cast<PFN_vkGetInstanceProcAddr>(SDL_Vulkan_GetVkGetInstanceProcAddr());
  if (!get_instance_proc_) throw Failure("runtime_unavailable");

  Uint32 ext_count = 0;
  const char* const* sdl_exts = SDL_Vulkan_GetInstanceExtensions(&ext_count);
  if (!sdl_exts || ext_count == 0 || ext_count > 32) throw Failure("runtime_unavailable");

  RawVkApplicationInfo app{kStructureTypeApplicationInfo, nullptr, "OmniPlayWorker", 1, "OmniPlay", 1, kVkApiVersion11};
  RawVkInstanceCreateInfo ici{kStructureTypeInstanceCreateInfo, nullptr, 0, &app, 0, nullptr, ext_count, sdl_exts};
  const auto create_instance = proc_i<FnCreateInstance>(get_instance_proc_, nullptr, "vkCreateInstance");
  if (create_instance(&ici, nullptr, &instance_) != VK_SUCCESS || !instance_) throw Failure("runtime_unavailable");

  if (!SDL_Vulkan_CreateSurface(window_, instance_, nullptr, &surface_) || !surface_) throw Failure("runtime_unavailable");

  const auto enum_gpus = proc_i<FnEnumeratePhysicalDevices>(get_instance_proc_, instance_, "vkEnumeratePhysicalDevices");
  const auto get_qprops = proc_i<FnGetQueueFamilyProps>(get_instance_proc_, instance_, "vkGetPhysicalDeviceQueueFamilyProperties");
  const auto get_surf_sup = proc_i<FnGetSurfaceSupportKHR>(get_instance_proc_, instance_, "vkGetPhysicalDeviceSurfaceSupportKHR");
  const auto create_device = proc_i<FnCreateDevice>(get_instance_proc_, instance_, "vkCreateDevice");

  std::uint32_t gpu_count = 0;
  if (enum_gpus(instance_, &gpu_count, nullptr) != VK_SUCCESS || gpu_count == 0) throw Failure("runtime_unavailable");
  std::vector<VkPhysicalDevice> gpus(std::min<std::uint32_t>(gpu_count, 16));
  gpu_count = static_cast<std::uint32_t>(gpus.size());
  enum_gpus(instance_, &gpu_count, gpus.data());

  for (auto candidate : gpus) {
    std::uint32_t q_count = 0;
    get_qprops(candidate, &q_count, nullptr);
    if (q_count == 0) continue;
    std::vector<RawVkQueueFamilyProperties> qprops(std::min<std::uint32_t>(q_count, 32));
    q_count = static_cast<std::uint32_t>(qprops.size());
    get_qprops(candidate, &q_count, qprops.data());
    for (std::uint32_t q = 0; q < q_count; ++q) {
      VkBool32 present_ok = 0;
      if ((qprops[q].queueFlags & 0x3u) == 0x3u && get_surf_sup(candidate, q, surface_, &present_ok) == VK_SUCCESS && present_ok) {
        gpu_ = candidate;
        queue_family_ = q;
        break;
      }
    }
    if (gpu_) break;
  }
  if (!gpu_) throw Failure("runtime_unavailable");

  const float priority = 1.0f;
  RawVkDeviceQueueCreateInfo qci{kStructureTypeDeviceQueueCreateInfo, nullptr, 0, queue_family_, 1, &priority};
  const char* dev_exts[] = {"VK_KHR_swapchain"};
  RawVkDeviceCreateInfo dci{kStructureTypeDeviceCreateInfo, nullptr, 0, 1, &qci, 0, nullptr, 1, dev_exts, nullptr};
  if (create_device(gpu_, &dci, nullptr, &device_) != VK_SUCCESS || !device_) throw Failure("runtime_unavailable");

  get_device_proc_ = proc_i<PFN_vkGetDeviceProcAddr>(get_instance_proc_, instance_, "vkGetDeviceProcAddr");
  const auto get_queue = proc_d<FnGetDeviceQueue>(get_device_proc_, device_, "vkGetDeviceQueue");
  get_queue(device_, queue_family_, 0, &queue_);
  if (!queue_) throw Failure("runtime_unavailable");

  const auto create_pool = proc_d<FnCreateCommandPool>(get_device_proc_, device_, "vkCreateCommandPool");
  const auto alloc_cmds = proc_d<FnAllocateCommandBuffers>(get_device_proc_, device_, "vkAllocateCommandBuffers");
  const auto create_fence = proc_d<FnCreateFence>(get_device_proc_, device_, "vkCreateFence");
  const auto create_sem = proc_d<FnCreateSemaphore>(get_device_proc_, device_, "vkCreateSemaphore");

  RawVkCommandPoolCreateInfo cpci{kStructureTypeCommandPoolCreateInfo, nullptr, 0x2u /*RESET_COMMAND_BUFFER*/, queue_family_};
  if (create_pool(device_, &cpci, nullptr, &cmd_pool_) != VK_SUCCESS) throw Failure("runtime_unavailable");
  RawVkCommandBufferAllocateInfo cbai{kStructureTypeCommandBufferAllocateInfo, nullptr, cmd_pool_, 0, kSyncSlots};
  if (alloc_cmds(device_, &cbai, cmd_buffers_.data()) != VK_SUCCESS) throw Failure("runtime_unavailable");

  RawVkFenceCreateInfo fci{kStructureTypeFenceCreateInfo, nullptr, 0x1u /*SIGNALED*/};
  RawVkSemaphoreCreateInfo sci{kStructureTypeSemaphoreCreateInfo, nullptr, 0};
  for (unsigned i = 0; i < kSyncSlots; ++i) {
    if (create_fence(device_, &fci, nullptr, &fences_[i]) != VK_SUCCESS ||
        create_sem(device_, &sci, nullptr, &acquire_semaphores_[i]) != VK_SUCCESS ||
        create_sem(device_, &sci, nullptr, &render_semaphores_[i]) != VK_SUCCESS) {
      throw Failure("runtime_unavailable");
    }
  }
  recreate_swapchain();
}

void VkVideo::destroy_swapchain() noexcept {
  if (!device_ || !swapchain_ || !get_device_proc_) return;
  auto wait_idle = reinterpret_cast<FnDeviceWaitIdle>(get_device_proc_(device_, "vkDeviceWaitIdle"));
  auto destroy_sc = reinterpret_cast<FnDestroySwapchainKHR>(get_device_proc_(device_, "vkDestroySwapchainKHR"));
  if (wait_idle) {
    std::lock_guard lock(queue_mutex_);
    wait_idle(device_);
  }
  if (destroy_sc) destroy_sc(device_, swapchain_, nullptr);
  swapchain_ = 0;
  swapchain_images_.clear();
}

void VkVideo::recreate_swapchain() {
  int pw = 0, ph = 0;
  if (!SDL_GetWindowSizeInPixels(window_, &pw, &ph) || pw <= 0 || ph <= 0) return;
  const auto get_caps = proc_i<FnGetSurfaceCapsKHR>(get_instance_proc_, instance_, "vkGetPhysicalDeviceSurfaceCapabilitiesKHR");
  const auto get_fmts = proc_i<FnGetSurfaceFormatsKHR>(get_instance_proc_, instance_, "vkGetPhysicalDeviceSurfaceFormatsKHR");
  const auto create_sc = proc_d<FnCreateSwapchainKHR>(get_device_proc_, device_, "vkCreateSwapchainKHR");
  const auto get_imgs = proc_d<FnGetSwapchainImagesKHR>(get_device_proc_, device_, "vkGetSwapchainImagesKHR");

  RawVkSurfaceCapabilitiesKHR caps{};
  if (get_caps(gpu_, surface_, &caps) != VK_SUCCESS) throw Failure("runtime_unavailable");
  std::uint32_t fmt_count = 0;
  if (get_fmts(gpu_, surface_, &fmt_count, nullptr) != VK_SUCCESS || fmt_count == 0) throw Failure("runtime_unavailable");
  std::vector<RawVkSurfaceFormatKHR> fmts(std::min<std::uint32_t>(fmt_count, 32));
  fmt_count = static_cast<std::uint32_t>(fmts.size());
  get_fmts(gpu_, surface_, &fmt_count, fmts.data());

  RawVkSurfaceFormatKHR chosen = fmts[0];
  for (const auto& f : fmts) {
    if (f.format == 44 /*B8G8R8A8_UNORM*/ || f.format == 37 /*R8G8B8A8_UNORM*/) {
      chosen = f;
      break;
    }
  }
  destroy_swapchain();
  swapchain_w_ = std::clamp<std::uint32_t>(static_cast<std::uint32_t>(pw), caps.minWidth, std::max(caps.minWidth, caps.maxWidth));
  swapchain_h_ = std::clamp<std::uint32_t>(static_cast<std::uint32_t>(ph), caps.minHeight, std::max(caps.minHeight, caps.maxHeight));
  std::uint32_t img_count = std::max<std::uint32_t>(kSyncSlots, caps.minImageCount);
  if (caps.maxImageCount > 0) img_count = std::min(img_count, caps.maxImageCount);

  RawVkSwapchainCreateInfoKHR sci{
      kStructureTypeSwapchainCreateInfoKHR, nullptr, 0, surface_,
      img_count, chosen.format, chosen.colorSpace,
      swapchain_w_, swapchain_h_, 1,
      0x12u /*COLOR_ATTACHMENT | TRANSFER_DST*/, 0 /*EXCLUSIVE*/, 0, nullptr,
      caps.currentTransform, 0x1u /*OPAQUE*/, 2 /*FIFO_KHR*/, 1, 0};
  if (create_sc(device_, &sci, nullptr, &swapchain_) != VK_SUCCESS || !swapchain_) throw Failure("runtime_unavailable");

  std::uint32_t actual = 0;
  get_imgs(device_, swapchain_, &actual, nullptr);
  if (actual == 0 || actual > 8) throw Failure("runtime_unavailable");
  swapchain_images_.resize(actual);
  get_imgs(device_, swapchain_, &actual, swapchain_images_.data());
}

bool VkVideo::negotiate(retro_hw_render_callback& callback) {
  if (!window_ || negotiated_ || core_callback_active() || core_callback_failed() ||
      callback.context_type != RETRO_HW_CONTEXT_VULKAN || !callback.context_reset) {
    return false;
  }
  initialize_device();
  callback_ = callback;
  interface_ = {
      RETRO_HW_RENDER_INTERFACE_VULKAN,
      5u,
      this,
      instance_,
      gpu_,
      device_,
      get_device_proc_,
      get_instance_proc_,
      queue_,
      queue_family_,
      cb_set_image,
      cb_get_sync_index,
      cb_get_sync_index_mask,
      cb_set_command_buffers,
      cb_wait_sync_index,
      cb_lock_queue,
      cb_unlock_queue,
      cb_set_signal_semaphore,
  };
  negotiated_ = true;
  return true;
}

void VkVideo::check_healthy() const {
  if (!window_ || !device_ || device_lost_) throw Failure("runtime_crashed");
}

void VkVideo::reset_core() {
  if (!negotiated_ || reset_called_ || core_callback_active() || core_callback_failed()) throw Failure("runtime_crashed");
  check_healthy();
  reset_in_progress_ = true;
  try { callback_.context_reset(); }
  catch (...) { reset_in_progress_ = false; reset_failed_ = true; throw; }
  reset_in_progress_ = false;
  reset_called_ = true;
}

void VkVideo::release_core() {
  if (core_callback_active() || core_callback_failed()) throw Failure("runtime_crashed");
  const auto destroy_fn = reset_called_ ? callback_.context_destroy : nullptr;
  if (destroy_fn && device_ && get_device_proc_) {
    auto wait_idle = reinterpret_cast<FnDeviceWaitIdle>(get_device_proc_(device_, "vkDeviceWaitIdle"));
    if (wait_idle) {
      std::lock_guard lock(queue_mutex_);
      wait_idle(device_);
    }
  }
  release_in_progress_ = true;
  try { if (destroy_fn) destroy_fn(); }
  catch (...) { release_in_progress_ = false; release_failed_ = true; throw; }
  release_in_progress_ = false;
  reset_called_ = negotiated_ = false;
  callback_ = {};
  interface_ = {};
  staged_image_ = nullptr;
  staged_wait_semaphores_.clear();
  staged_cmd_buffers_.clear();
  staged_signal_semaphore_ = 0;
}

void VkVideo::destroy() noexcept {
  try { release_core(); } catch (...) { omni_owned_actor_exit(1); }
  destroy_swapchain();
  if (device_ && get_device_proc_) {
    auto destroy_fence = reinterpret_cast<FnDestroyFence>(get_device_proc_(device_, "vkDestroyFence"));
    auto destroy_sem = reinterpret_cast<FnDestroySemaphore>(get_device_proc_(device_, "vkDestroySemaphore"));
    auto destroy_pool = reinterpret_cast<FnDestroyCommandPool>(get_device_proc_(device_, "vkDestroyCommandPool"));
    auto destroy_dev = reinterpret_cast<FnDestroyDevice>(get_device_proc_(device_, "vkDestroyDevice"));
    for (unsigned i = 0; i < kSyncSlots; ++i) {
      if (fences_[i] && destroy_fence) destroy_fence(device_, fences_[i], nullptr);
      if (acquire_semaphores_[i] && destroy_sem) destroy_sem(device_, acquire_semaphores_[i], nullptr);
      if (render_semaphores_[i] && destroy_sem) destroy_sem(device_, render_semaphores_[i], nullptr);
      fences_[i] = 0;
      acquire_semaphores_[i] = 0;
      render_semaphores_[i] = 0;
    }
    if (cmd_pool_ && destroy_pool) destroy_pool(device_, cmd_pool_, nullptr);
    cmd_pool_ = 0;
    if (destroy_dev) destroy_dev(device_, nullptr);
    device_ = nullptr;
  }
  if (instance_ && surface_) {
    SDL_Vulkan_DestroySurface(instance_, surface_, nullptr);
    surface_ = 0;
  }
  if (instance_ && get_instance_proc_) {
    auto destroy_inst = reinterpret_cast<FnDestroyInstance>(get_instance_proc_(instance_, "vkDestroyInstance"));
    if (destroy_inst) destroy_inst(instance_, nullptr);
    instance_ = nullptr;
  }
  if (library_loaded_) {
    SDL_Vulkan_UnloadLibrary();
    library_loaded_ = false;
  }
  window_ = nullptr;
  gpu_ = nullptr;
  queue_ = nullptr;
}

void VkVideo::cb_set_image(void* handle, const RetroVulkanImage* image, std::uint32_t num_semaphores, const VkSemaphore* semaphores, std::uint32_t src_queue_family) {
  auto* self = static_cast<VkVideo*>(handle);
  if (!self) return;
  self->staged_image_ = image;
  self->staged_src_queue_family_ = src_queue_family;
  self->staged_wait_semaphores_.clear();
  if (semaphores && num_semaphores > 0 && num_semaphores <= 16) {
    self->staged_wait_semaphores_.assign(semaphores, semaphores + num_semaphores);
  }
}
std::uint32_t VkVideo::cb_get_sync_index(void* handle) {
  auto* self = static_cast<VkVideo*>(handle);
  return self ? self->current_sync_ : 0u;
}
std::uint32_t VkVideo::cb_get_sync_index_mask(void*) {
  return (1u << kSyncSlots) - 1u;
}
void VkVideo::cb_set_command_buffers(void* handle, std::uint32_t num_cmd, const VkCommandBuffer* cmd) {
  auto* self = static_cast<VkVideo*>(handle);
  if (!self) return;
  self->staged_cmd_buffers_.clear();
  if (cmd && num_cmd > 0 && num_cmd <= 16) {
    self->staged_cmd_buffers_.assign(cmd, cmd + num_cmd);
  }
}
void VkVideo::cb_wait_sync_index(void* handle) {
  auto* self = static_cast<VkVideo*>(handle);
  if (!self || !self->device_ || !self->get_device_proc_) return;
  auto wait_fences = reinterpret_cast<FnWaitForFences>(self->get_device_proc_(self->device_, "vkWaitForFences"));
  if (wait_fences && wait_fences(self->device_, 1, &self->fences_[self->current_sync_], 1, 2'000'000'000ull) == VK_ERROR_DEVICE_LOST) {
    self->device_lost_ = true;
  }
}
void VkVideo::cb_lock_queue(void* handle) {
  if (auto* self = static_cast<VkVideo*>(handle)) self->queue_mutex_.lock();
}
void VkVideo::cb_unlock_queue(void* handle) {
  if (auto* self = static_cast<VkVideo*>(handle)) self->queue_mutex_.unlock();
}
void VkVideo::cb_set_signal_semaphore(void* handle, VkSemaphore semaphore) {
  if (auto* self = static_cast<VkVideo*>(handle)) self->staged_signal_semaphore_ = semaphore;
}

bool VkVideo::present(unsigned width, unsigned height, double aspect) {
  if (!negotiated_ || !reset_called_ || core_callback_active() || core_callback_failed() ||
      !width || !height || width > width_ || height > height_) {
    throw Failure("runtime_crashed");
  }
  check_healthy();
  int pw = 0, ph = 0;
  if (!SDL_GetWindowSizeInPixels(window_, &pw, &ph)) throw Failure("runtime_crashed");
  if (pw <= 0 || ph <= 0 || pw > 16384 || ph > 16384) return false;
  if (!swapchain_ || static_cast<std::uint32_t>(pw) != swapchain_w_ || static_cast<std::uint32_t>(ph) != swapchain_h_) {
    recreate_swapchain();
    if (!swapchain_) return false;
  }

  const auto wait_fences = proc_d<FnWaitForFences>(get_device_proc_, device_, "vkWaitForFences");
  const auto reset_fences = proc_d<FnResetFences>(get_device_proc_, device_, "vkResetFences");
  const auto acquire_img = proc_d<FnAcquireNextImageKHR>(get_device_proc_, device_, "vkAcquireNextImageKHR");
  const auto reset_cmd = proc_d<FnResetCommandBuffer>(get_device_proc_, device_, "vkResetCommandBuffer");
  const auto begin_cmd = proc_d<FnBeginCommandBuffer>(get_device_proc_, device_, "vkBeginCommandBuffer");
  const auto end_cmd = proc_d<FnEndCommandBuffer>(get_device_proc_, device_, "vkEndCommandBuffer");
  const auto barrier = proc_d<FnCmdPipelineBarrier>(get_device_proc_, device_, "vkCmdPipelineBarrier");
  const auto clear_img = proc_d<FnCmdClearColorImage>(get_device_proc_, device_, "vkCmdClearColorImage");
  const auto blit_img = proc_d<FnCmdBlitImage>(get_device_proc_, device_, "vkCmdBlitImage");
  const auto submit_q = proc_d<FnQueueSubmit>(get_device_proc_, device_, "vkQueueSubmit");
  const auto present_q = proc_d<FnQueuePresentKHR>(get_device_proc_, device_, "vkQueuePresentKHR");

  const unsigned slot = current_sync_;
  if (wait_fences(device_, 1, &fences_[slot], 1, 2'000'000'000ull) != VK_SUCCESS) {
    device_lost_ = true;
    throw Failure("runtime_crashed");
  }

  std::uint32_t img_index = 0;
  const VkResult acq = acquire_img(device_, swapchain_, 1'000'000'000ull, acquire_semaphores_[slot], 0, &img_index);
  if (acq == VK_ERROR_OUT_OF_DATE_KHR) {
    recreate_swapchain();
    return false;
  }
  if (acq == VK_ERROR_DEVICE_LOST || acq == VK_ERROR_SURFACE_LOST_KHR || (acq != VK_SUCCESS && acq != VK_SUBOPTIMAL_KHR)) {
    device_lost_ = true;
    throw Failure("runtime_crashed");
  }

  reset_fences(device_, 1, &fences_[slot]);
  VkCommandBuffer cmd = cmd_buffers_[slot];
  reset_cmd(cmd, 0);
  RawVkCommandBufferBeginInfo binfo{kStructureTypeCommandBufferBeginInfo, nullptr, 0x1u /*ONE_TIME_SUBMIT*/, nullptr};
  begin_cmd(cmd, &binfo);

  VkImage dst = swapchain_images_[img_index];
  VkImageSubresourceRange color_range{0x1u /*COLOR*/, 0, 1, 0, 1};
  RawVkImageMemoryBarrier to_dst{kStructureTypeImageMemoryBarrier, nullptr, 0, 0x1000u /*TRANSFER_WRITE*/, 0 /*UNDEFINED*/, 7 /*TRANSFER_DST*/, ~0u, ~0u, dst, color_range};
  barrier(cmd, 0x1u /*TOP_OF_PIPE*/, 0x1000u /*TRANSFER*/, 0, 0, nullptr, 0, nullptr, 1, &to_dst);

  RawVkClearColorValue black{{0.0f, 0.0f, 0.0f, 1.0f}};
  clear_img(cmd, dst, 7 /*TRANSFER_DST*/, &black, 1, &color_range);

  if (staged_image_ && staged_image_->create_info.image) {
    const double ratio = std::isfinite(aspect) && aspect > 0.1 && aspect < 10.0 ? aspect : double(width) / height;
    int draw_w = static_cast<int>(swapchain_w_);
    int draw_h = static_cast<int>(std::lround(draw_w / ratio));
    if (draw_h > static_cast<int>(swapchain_h_)) {
      draw_h = static_cast<int>(swapchain_h_);
      draw_w = static_cast<int>(std::lround(draw_h * ratio));
    }
    const int left = (static_cast<int>(swapchain_w_) - draw_w) / 2;
    const int top = (static_cast<int>(swapchain_h_) - draw_h) / 2;

    RawVkImageBlit region{
        {0x1u, 0, 0, 1},
        {{0, 0, 0}, {static_cast<std::int32_t>(width), static_cast<std::int32_t>(height), 1}},
        {0x1u, 0, 0, 1},
        {{left, top, 0}, {left + draw_w, top + draw_h, 1}}};
    blit_img(cmd, staged_image_->create_info.image, staged_image_->image_layout, dst, 7 /*TRANSFER_DST*/, 1, &region, 1 /*LINEAR*/);
  }

  RawVkImageMemoryBarrier to_present{kStructureTypeImageMemoryBarrier, nullptr, 0x1000u /*TRANSFER_WRITE*/, 0, 7 /*TRANSFER_DST*/, 1000001002 /*PRESENT_SRC_KHR*/, ~0u, ~0u, dst, color_range};
  barrier(cmd, 0x1000u /*TRANSFER*/, 0x2000u /*BOTTOM_OF_PIPE*/, 0, 0, nullptr, 0, nullptr, 1, &to_present);
  end_cmd(cmd);

  std::vector<VkCommandBuffer> all_cmds = staged_cmd_buffers_;
  all_cmds.push_back(cmd);
  std::vector<VkSemaphore> waits;
  std::vector<VkFlags> stages;
  waits.push_back(acquire_semaphores_[slot]);
  stages.push_back(0x1000u /*TRANSFER*/);
  if (staged_cmd_buffers_.empty()) {
    for (auto s : staged_wait_semaphores_) {
      waits.push_back(s);
      stages.push_back(0x1000u);
    }
  }
  std::vector<VkSemaphore> signals{render_semaphores_[slot]};
  if (staged_signal_semaphore_) signals.push_back(staged_signal_semaphore_);

  RawVkSubmitInfo sinfo{
      kStructureTypeSubmitInfo, nullptr,
      static_cast<std::uint32_t>(waits.size()), waits.data(), stages.data(),
      static_cast<std::uint32_t>(all_cmds.size()), all_cmds.data(),
      static_cast<std::uint32_t>(signals.size()), signals.data()};

  VkResult sub_res = VK_SUCCESS;
  VkResult pres_res = VK_SUCCESS;
  {
    std::lock_guard lock(queue_mutex_);
    sub_res = submit_q(queue_, 1, &sinfo, fences_[slot]);
    if (sub_res == VK_SUCCESS) {
      RawVkPresentInfoKHR pinfo{kStructureTypePresentInfoKHR, nullptr, 1, &render_semaphores_[slot], 1, &swapchain_, &img_index, nullptr};
      pres_res = present_q(queue_, &pinfo);
    }
  }
  staged_wait_semaphores_.clear();
  staged_cmd_buffers_.clear();
  staged_signal_semaphore_ = 0;
  current_sync_ = (current_sync_ + 1) % kSyncSlots;

  if (sub_res != VK_SUCCESS || pres_res == VK_ERROR_DEVICE_LOST || pres_res == VK_ERROR_SURFACE_LOST_KHR) {
    device_lost_ = true;
    throw Failure("runtime_crashed");
  }
  if (pres_res == VK_ERROR_OUT_OF_DATE_KHR || pres_res == VK_SUBOPTIMAL_KHR) {
    recreate_swapchain();
  }
  return true;
}

} // namespace omni
