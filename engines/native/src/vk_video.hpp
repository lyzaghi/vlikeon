#pragma once
#include <SDL3/SDL.h>
#include <SDL3/SDL_vulkan.h>
#include <libretro.h>
#include <array>
#include <cstdint>
#include <mutex>
#include <vector>

#if __has_include(<vulkan/vulkan_core.h>)
#include <vulkan/vulkan_core.h>
#else
#if defined(_WIN32)
#define VKAPI_ATTR
#define VKAPI_CALL __stdcall
#define VKAPI_PTR VKAPI_CALL
#else
#define VKAPI_ATTR
#define VKAPI_CALL
#define VKAPI_PTR
#endif

using VkFlags = std::uint32_t;
using VkBool32 = std::uint32_t;
using VkDeviceSize = std::uint64_t;
#define VK_DEFINE_NON_DISPATCHABLE_HANDLE(object) using object = std::uint64_t;
using VkQueue = struct VkQueue_T*;
using VkCommandBuffer = struct VkCommandBuffer_T*;
VK_DEFINE_NON_DISPATCHABLE_HANDLE(VkSemaphore)
VK_DEFINE_NON_DISPATCHABLE_HANDLE(VkFence)
VK_DEFINE_NON_DISPATCHABLE_HANDLE(VkImage)
VK_DEFINE_NON_DISPATCHABLE_HANDLE(VkImageView)
VK_DEFINE_NON_DISPATCHABLE_HANDLE(VkCommandPool)
VK_DEFINE_NON_DISPATCHABLE_HANDLE(VkSwapchainKHR)

enum VkResult : int {
  VK_SUCCESS = 0,
  VK_SUBOPTIMAL_KHR = 1000001003,
  VK_ERROR_DEVICE_LOST = -4,
  VK_ERROR_SURFACE_LOST_KHR = -1000000000,
  VK_ERROR_OUT_OF_DATE_KHR = -1000001004,
};

using PFN_vkVoidFunction = void (VKAPI_PTR*)(void);
using PFN_vkGetInstanceProcAddr = PFN_vkVoidFunction (VKAPI_PTR*)(VkInstance, const char*);
using PFN_vkGetDeviceProcAddr = PFN_vkVoidFunction (VKAPI_PTR*)(VkDevice, const char*);

struct VkComponentMapping { int r, g, b, a; };
struct VkImageSubresourceRange { VkFlags aspectMask; std::uint32_t baseMipLevel, levelCount, baseArrayLayer, layerCount; };
struct VkImageViewCreateInfo {
  int sType;
  const void* pNext;
  VkFlags flags;
  VkImage image;
  int viewType;
  int format;
  VkComponentMapping components;
  VkImageSubresourceRange subresourceRange;
};
using VkImageLayout = int;
#endif

namespace omni {

struct RetroVulkanImage {
  VkImageView image_view;
  VkImageLayout image_layout;
  VkImageViewCreateInfo create_info;
};

struct RetroHwRenderInterfaceVulkan {
  retro_hw_render_interface_type interface_type;
  unsigned interface_version;
  void* handle;
  VkInstance instance;
  VkPhysicalDevice gpu;
  VkDevice device;
  PFN_vkGetDeviceProcAddr get_device_proc_addr;
  PFN_vkGetInstanceProcAddr get_instance_proc_addr;
  VkQueue queue;
  unsigned queue_index;
  void (*set_image)(void* handle, const RetroVulkanImage* image, std::uint32_t num_semaphores, const VkSemaphore* semaphores, std::uint32_t src_queue_family);
  std::uint32_t (*get_sync_index)(void* handle);
  std::uint32_t (*get_sync_index_mask)(void* handle);
  void (*set_command_buffers)(void* handle, std::uint32_t num_cmd, const VkCommandBuffer* cmd);
  void (*wait_sync_index)(void* handle);
  void (*lock_queue)(void* handle);
  void (*unlock_queue)(void* handle);
  void (*set_signal_semaphore)(void* handle, VkSemaphore semaphore);
};

class VkVideo {
  static constexpr unsigned kSyncSlots = 3;

  SDL_Window* window_ = nullptr;
  bool library_loaded_ = false;
  bool negotiated_ = false;
  bool reset_called_ = false;
  bool release_in_progress_ = false;
  bool release_failed_ = false;
  bool reset_in_progress_ = false;
  bool reset_failed_ = false;
  bool device_lost_ = false;
  unsigned width_ = 0;
  unsigned height_ = 0;

  retro_hw_render_callback callback_{};
  RetroHwRenderInterfaceVulkan interface_{};

  PFN_vkGetInstanceProcAddr get_instance_proc_ = nullptr;
  PFN_vkGetDeviceProcAddr get_device_proc_ = nullptr;
  VkInstance instance_ = nullptr;
  VkSurfaceKHR surface_ = 0;
  VkPhysicalDevice gpu_ = nullptr;
  VkDevice device_ = nullptr;
  VkQueue queue_ = nullptr;
  std::uint32_t queue_family_ = 0;

  VkSwapchainKHR swapchain_ = 0;
  std::uint32_t swapchain_w_ = 0;
  std::uint32_t swapchain_h_ = 0;
  std::vector<VkImage> swapchain_images_;
  VkCommandPool cmd_pool_ = 0;
  std::array<VkCommandBuffer, kSyncSlots> cmd_buffers_{};
  std::array<VkFence, kSyncSlots> fences_{};
  std::array<VkSemaphore, kSyncSlots> acquire_semaphores_{};
  std::array<VkSemaphore, kSyncSlots> render_semaphores_{};
  std::uint32_t current_sync_ = 0;

  std::mutex queue_mutex_;
  const RetroVulkanImage* staged_image_ = nullptr;
  std::vector<VkSemaphore> staged_wait_semaphores_;
  std::vector<VkCommandBuffer> staged_cmd_buffers_;
  VkSemaphore staged_signal_semaphore_ = 0;
  std::uint32_t staged_src_queue_family_ = ~0u;

  void initialize_device();
  void recreate_swapchain();
  void destroy_swapchain() noexcept;

  static void cb_set_image(void* handle, const RetroVulkanImage* image, std::uint32_t num_semaphores, const VkSemaphore* semaphores, std::uint32_t src_queue_family);
  static std::uint32_t cb_get_sync_index(void* handle);
  static std::uint32_t cb_get_sync_index_mask(void* handle);
  static void cb_set_command_buffers(void* handle, std::uint32_t num_cmd, const VkCommandBuffer* cmd);
  static void cb_wait_sync_index(void* handle);
  static void cb_lock_queue(void* handle);
  static void cb_unlock_queue(void* handle);
  static void cb_set_signal_semaphore(void* handle, VkSemaphore semaphore);

public:
  void configure(SDL_Window* window, unsigned width, unsigned height);
  bool negotiate(retro_hw_render_callback& callback);
  bool negotiated() const { return negotiated_; }
  bool core_callback_active() const { return release_in_progress_ || reset_in_progress_; }
  bool core_callback_failed() const { return release_failed_ || reset_failed_; }
  unsigned width() const { return width_; }
  unsigned height() const { return height_; }
  const void* render_interface() const { return negotiated_ ? &interface_ : nullptr; }
  void check_healthy() const;
  void reset_core();
  void release_core();
  void destroy() noexcept;
  bool present(unsigned width, unsigned height, double aspect_ratio);
};

} // namespace omni
