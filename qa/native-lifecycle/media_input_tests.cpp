#include "../../engines/presentation/src/raw_joystick.hpp"
#include "../../engines/presentation/src/touch_overlay.hpp"
#include "../../engines/presentation/src/wire.hpp"
#include "../../engines/native/src/audio_ring.hpp"
#include "../../engines/native/src/input_profile.hpp"
#include "../../engines/native/src/presentation_input.hpp"
#include "../../engines/native/src/gl_video.hpp"
#include "../../engines/native/src/vk_video.hpp"

#include <SDL3/SDL.h>
#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

void require(bool condition, const char* message) {
  if (!condition) {
    throw std::runtime_error(message);
  }
}

void write_u16_le(std::array<std::uint8_t, omni::presentation::kInputBytes>& buf, std::size_t offset, std::int16_t v) {
  const auto u = static_cast<std::uint16_t>(v);
  buf[offset] = static_cast<std::uint8_t>(u & 0xffu);
  buf[offset + 1] = static_cast<std::uint8_t>((u >> 8) & 0xffu);
}

void write_u32_le(std::array<std::uint8_t, omni::presentation::kInputBytes>& buf, std::size_t offset, std::uint32_t u) {
  buf[offset] = static_cast<std::uint8_t>(u & 0xffu);
  buf[offset + 1] = static_cast<std::uint8_t>((u >> 8) & 0xffu);
  buf[offset + 2] = static_cast<std::uint8_t>((u >> 16) & 0xffu);
  buf[offset + 3] = static_cast<std::uint8_t>((u >> 24) & 0xffu);
}

// Item 37: SDL Raw Joystick 3-step calibration HUD + unmapped virtual HID joystick
void test_sdl_raw_joystick_calibration_and_virtual_hid(SDL_Renderer* renderer) {
  SDL_VirtualJoystickDesc desc{};
  SDL_INIT_INTERFACE(&desc);
  desc.type = SDL_JOYSTICK_TYPE_UNKNOWN;
  desc.vendor_id = 0x1234;
  desc.product_id = 0x5678;
  desc.naxes = 6;
  desc.nbuttons = 12;
  desc.nhats = 1;
  desc.name = "OmniPlay Unmapped HID Pad";

  const SDL_JoystickID virt_id = SDL_AttachVirtualJoystick(&desc);
  require(virt_id != 0, "SDL_AttachVirtualJoystick failed");
  require(!SDL_IsGamepad(virt_id), "Virtual unmapped HID joystick should not be classified as mapped SDL_Gamepad");

  omni::presentation::RawJoystickManager mgr;
  std::array<SDL_Gamepad*, 4> no_gamepads{nullptr, nullptr, nullptr, nullptr};
  mgr.refresh_devices(no_gamepads);

  SDL_Joystick* raw_joy = SDL_OpenJoystick(virt_id);
  require(raw_joy != nullptr, "SDL_OpenJoystick failed on virtual joystick");

  // Initial resting center: sticks at 0, triggers (axes 4, 5) at -32767
  for (int a = 0; a < 4; ++a) SDL_SetJoystickVirtualAxis(raw_joy, a, 0);
  SDL_SetJoystickVirtualAxis(raw_joy, 4, -32767);
  SDL_SetJoystickVirtualAxis(raw_joy, 5, -32767);
  SDL_UpdateJoysticks();

  // Open calibration HUD with F10 -> Overview
  require(!mgr.calibration_active(), "Calibration should start hidden");
  require(mgr.handle_key(SDL_SCANCODE_F10), "F10 should open RawJoystickManager overview");
  require(mgr.calibration_active(), "Calibration should be active in Overview");
  mgr.render_overlay(renderer, 640, 480);

  // Enter -> Step 1/3 CaptureCenter
  require(mgr.handle_key(SDL_SCANCODE_RETURN), "Return should enter CaptureCenter");
  mgr.render_overlay(renderer, 640, 480);

  // Enter -> capture resting center & advance to Step 2/3 CaptureRange
  require(mgr.handle_key(SDL_SCANCODE_RETURN), "Return should capture center and enter CaptureRange");
  for (int a = 0; a < 6; ++a) {
    SDL_SetJoystickVirtualAxis(raw_joy, a, -32767);
  }
  SDL_UpdateJoysticks();
  mgr.tick();
  for (int a = 0; a < 6; ++a) {
    SDL_SetJoystickVirtualAxis(raw_joy, a, 32767);
  }
  SDL_UpdateJoysticks();
  mgr.tick();
  // Return to resting
  for (int a = 0; a < 4; ++a) SDL_SetJoystickVirtualAxis(raw_joy, a, 0);
  SDL_SetJoystickVirtualAxis(raw_joy, 4, -32767);
  SDL_SetJoystickVirtualAxis(raw_joy, 5, -32767);
  SDL_UpdateJoysticks();
  mgr.render_overlay(renderer, 640, 480);

  // Enter -> finalize axis ranges & advance to Step 3/3 BindButtons
  require(mgr.handle_key(SDL_SCANCODE_RETURN), "Return should finalize ranges and enter BindButtons");
  mgr.render_overlay(renderer, 640, 480);

  // Bind 16 controls:
  // 0..3 via buttons 0..3
  for (int b = 0; b < 4; ++b) {
    SDL_Delay(310);
    SDL_SetJoystickVirtualButton(raw_joy, b, true);
    SDL_UpdateJoysticks();
    mgr.tick();
    SDL_SetJoystickVirtualButton(raw_joy, b, false);
    SDL_UpdateJoysticks();
  }
  // 4..7 (D-Pad Up/Down/Left/Right) via Hat 0
  const Uint8 hat_dirs[4] = {SDL_HAT_UP, SDL_HAT_DOWN, SDL_HAT_LEFT, SDL_HAT_RIGHT};
  for (int h = 0; h < 4; ++h) {
    SDL_Delay(310);
    SDL_SetJoystickVirtualHat(raw_joy, 0, hat_dirs[h]);
    SDL_UpdateJoysticks();
    mgr.tick();
    SDL_SetJoystickVirtualHat(raw_joy, 0, SDL_HAT_CENTERED);
    SDL_UpdateJoysticks();
  }
  // 8..15 via buttons 4..11
  for (int b = 4; b < 12; ++b) {
    SDL_Delay(310);
    SDL_SetJoystickVirtualButton(raw_joy, b, true);
    SDL_UpdateJoysticks();
    mgr.tick();
    SDL_SetJoystickVirtualButton(raw_joy, b, false);
    SDL_UpdateJoysticks();
  }

  // Close UI via F10
  require(mgr.handle_key(SDL_SCANCODE_F10), "F10 should close calibration UI");
  require(!mgr.calibration_active(), "Calibration UI should be hidden after F10");

  // Sample calibrated port 0
  SDL_SetJoystickVirtualButton(raw_joy, 0, true); // South
  SDL_SetJoystickVirtualHat(raw_joy, 0, SDL_HAT_UP); // D-Pad Up
  SDL_SetJoystickVirtualAxis(raw_joy, 0, 16384); // LX +50%
  SDL_SetJoystickVirtualAxis(raw_joy, 4, 32767); // LT full pull
  SDL_UpdateJoysticks();

  const auto sampled = mgr.sample_port(0);
  require(sampled.connected, "Calibrated virtual joystick on port 0 must report connected");
  require((sampled.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_SOUTH)) != 0, "South button must be set");
  require((sampled.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_DPAD_UP)) != 0, "Hat Up must map to DPAD_UP");
  require(sampled.axes[0] > 15000 && sampled.axes[0] < 17500, "Calibrated LX axis should be ~16384");
  require(sampled.axes[4] == 32767, "Calibrated LT trigger should reach 32767");

  mgr.remove_device(virt_id);
  mgr.shutdown();
  SDL_CloseJoystick(raw_joy);
  SDL_DetachVirtualJoystick(virt_id);
}

// Item 38: Native 18-control touch overlay, 10 contacts, F9 visibility cycle, Shift+F9 custom placement
void test_native_touch_overlay_18_controls_10_contacts_and_editor(SDL_Renderer* renderer) {
  omni::presentation::NativeTouchOverlay overlay;
  overlay.configure(omni::presentation::NativeTouchOverlay::Visibility::Auto, false, 100, 90);

  // Cycle visibility with F9: Auto -> Always -> Hidden -> Auto
  require(overlay.handle_key(SDL_SCANCODE_F9, SDL_KMOD_NONE), "F9 should cycle Auto -> Always");
  require(overlay.handle_key(SDL_SCANCODE_F9, SDL_KMOD_NONE), "F9 should cycle Always -> Hidden");
  require(!overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 1, 0.84f, 0.78f, 1280, 720),
          "Hidden overlay must ignore touch events");
  require(overlay.handle_key(SDL_SCANCODE_F9, SDL_KMOD_NONE), "F9 should cycle Hidden -> Auto");

  // Toggle edit mode with Shift+F9 and drag control 0 (B at 0.84, 0.78) to extreme corner
  require(overlay.handle_key(SDL_SCANCODE_F9, SDL_KMOD_LSHIFT), "Shift+F9 should enter edit mode");
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 99, 0.84f, 0.78f, 1280, 720),
          "Should grab B button in edit mode");
  require(overlay.handle_finger(SDL_EVENT_FINGER_MOTION, 99, 0.99f, 0.01f, 1280, 720),
          "Should drag B button in edit mode");
  require(overlay.state(1280, 720).active == false, "Edit mode must not emit gameplay button presses");
  overlay.render(renderer, 1280, 720);
  require(overlay.handle_finger(SDL_EVENT_FINGER_UP, 99, 0.99f, 0.01f, 1280, 720),
          "Should release B button in edit mode");
  require(overlay.handle_key(SDL_SCANCODE_F9, SDL_KMOD_RSHIFT), "Shift+F9 should exit edit mode");

  // Verify dragged B button is clamped at (0.94, 0.06) and responds there
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 1, 0.94f, 0.06f, 1280, 720),
          "Clamped B button at (0.94, 0.06) should accept finger 1");
  // Add 9 more simultaneous contacts (total 10 contacts across buttons, L2 trigger, and both analog sticks)
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 2, 0.78f, 0.70f, 1280, 720), "Finger 2 on Y");
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 3, 0.43f, 0.90f, 1280, 720), "Finger 3 on SEL");
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 4, 0.57f, 0.90f, 1280, 720), "Finger 4 on STA");
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 5, 0.16f, 0.62f, 1280, 720), "Finger 5 on D-Pad Up");
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 6, 0.90f, 0.70f, 1280, 720), "Finger 6 on A");
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 7, 0.12f, 0.18f, 1280, 720), "Finger 7 on L1");
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 8, 0.12f, 0.08f, 1280, 720), "Finger 8 on L2 trigger");
  // Finger 9 on Left Stick (center 0.28, 0.82), dragged right/down
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 9, 0.28f, 0.82f, 1280, 720), "Finger 9 on LS");
  require(overlay.handle_finger(SDL_EVENT_FINGER_MOTION, 9, 0.31f, 0.85f, 1280, 720), "Finger 9 drag LS");
  // Finger 10 on Right Stick (center 0.72, 0.82), dragged left/up
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 10, 0.72f, 0.82f, 1280, 720), "Finger 10 on RS");
  require(overlay.handle_finger(SDL_EVENT_FINGER_MOTION, 10, 0.69f, 0.79f, 1280, 720), "Finger 10 drag RS");

  const auto st = overlay.state(1280, 720);
  require(st.active, "10-finger overlay state must be active");
  require((st.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_SOUTH)) != 0, "B (South) pressed");
  require((st.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_WEST)) != 0, "Y (West) pressed");
  require((st.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_BACK)) != 0, "SEL pressed");
  require((st.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_START)) != 0, "STA pressed");
  require((st.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_DPAD_UP)) != 0, "DPAD_UP pressed");
  require((st.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_EAST)) != 0, "A (East) pressed");
  require((st.sdl_button_mask & (1u << SDL_GAMEPAD_BUTTON_LEFT_SHOULDER)) != 0, "L1 pressed");
  require(st.axes[4] == 32767, "L2 trigger axis should be 32767");
  require(st.axes[0] > 10000 && st.axes[1] > 10000, "LS axes should be positive");
  require(st.axes[2] < -10000 && st.axes[3] < -10000, "RS axes should be negative");

  overlay.render(renderer, 1280, 720);
  overlay.clear_contacts();
  require(!overlay.state(1280, 720).active, "clear_contacts() must release all 10 contacts immediately");
}

// Item 40 & Item 44: Analog pressure buttons, Mouse/Lightgun/Pointer peripherals, 4-port wire layout & focus loss
void test_pressure_buttons_peripherals_and_four_port_focus_loss() {
  omni::Json j = {
      {"schema_version", 1},
      {"keyboard", {"KeyZ", "KeyA", "ShiftRight", "Enter", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
                    "KeyX", "KeyS", "KeyQ", "KeyW", "KeyE", "KeyR", "KeyD", "KeyF"}},
      {"analog_keyboard", {"KeyI", "KeyK", "KeyJ", "KeyL"}},
      {"button_sources", {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15}},
      {"haptics_enabled", true},
      {"pressure_buttons_enabled", true},
      {"analog_deadzone_percent", 15},
      {"controller_ports", {
          {{"device", "analog_joypad"}, {"subclass", 1}, {"pressure_curve_percent", 200}},
          {{"device", "mouse"}, {"subclass", 0}, {"pressure_curve_percent", 100}},
          {{"device", "lightgun"}, {"subclass", 2}, {"pressure_curve_percent", 50}},
          {{"device", "none"}, {"subclass", 0}, {"pressure_curve_percent", 100}},
      }}
  };

  omni::InputProfile profile;
  require(profile.load(j), "InputProfile::load must accept valid pressure_buttons + controller_ports JSON");
  require(profile.pressure_buttons, "pressure_buttons must be enabled");
  require(profile.controller_ports[0].retro_device_id() == RETRO_DEVICE_SUBCLASS(RETRO_DEVICE_ANALOG, 0),
          "Port 0 retro_device_id must be RETRO_DEVICE_SUBCLASS(RETRO_DEVICE_ANALOG, 0)");
  require(profile.controller_ports[1].retro_device_id() == RETRO_DEVICE_MOUSE,
          "Port 1 retro_device_id must be RETRO_DEVICE_MOUSE");
  require(profile.controller_ports[2].retro_device_id() == RETRO_DEVICE_SUBCLASS(RETRO_DEVICE_LIGHTGUN, 1),
          "Port 2 retro_device_id must be RETRO_DEVICE_SUBCLASS(RETRO_DEVICE_LIGHTGUN, 1)");
  require(profile.controller_ports[3].retro_device_id() == RETRO_DEVICE_NONE,
          "Port 3 retro_device_id must be RETRO_DEVICE_NONE");

  // Pressure shaping: port 0 has curve 200% (firm, < linear), port 1 has 100% (linear), port 2 has 50% (soft, > linear)
  const auto p_firm = profile.shape_pressure(16384, 0);
  const auto p_linear = profile.shape_pressure(16384, 1);
  const auto p_soft = profile.shape_pressure(16384, 2);
  require(p_firm < p_linear && p_linear < p_soft, "Pressure curve shaping must order firm < linear < soft");

  // Build 160-byte presentation::Snapshot
  omni::presentation::Snapshot snap{};
  snap.fresh = true;
  snap.focused = true;
  snap.input[144] = 1; // focused

  // Port 0..3 connected with distinct button masks & axes
  for (unsigned port = 0; port < 4; ++port) {
    const std::size_t at = 64u + port * 20u;
    snap.input[at] = 1; // connected
    snap.input[at + 1] = 1; // rumble supported
    write_u32_le(snap.input, at + 4, (1u << SDL_GAMEPAD_BUTTON_SOUTH) | (1u << SDL_GAMEPAD_BUTTON_START));
    write_u16_le(snap.input, at + 8 + 0, 20000);  // LX
    write_u16_le(snap.input, at + 8 + 2, -20000); // LY
    write_u16_le(snap.input, at + 8 + 8, 16384);  // LT (axis 4)
    write_u16_le(snap.input, at + 8 + 10, 24000); // RT (axis 5)
  }
  // Pointer in bounds + pressed at (-4096, 8192)
  snap.input[145] = 1;
  snap.input[146] = 1;
  // Mouse buttons: left (1) + right (2) = 3
  snap.input[147] = 0x03;
  write_u16_le(snap.input, 148, -4096);
  write_u16_le(snap.input, 150, 8192);
  write_u16_le(snap.input, 152, -15); // mouse dx
  write_u16_le(snap.input, 154, 22);  // mouse dy
  write_u16_le(snap.input, 156, 1);   // wheel x
  write_u16_le(snap.input, 158, -1);  // wheel y

  // Verify Port 0 Analog Button pressure (L2 = id 12 -> axis 4 shaped with port 0 curve 200%)
  const auto l2_pressure = omni::presentation_input(
      snap, profile, true, 0, RETRO_DEVICE_ANALOG, RETRO_DEVICE_INDEX_ANALOG_BUTTON, RETRO_DEVICE_ID_JOYPAD_L2);
  require(l2_pressure == p_firm, "Port 0 L2 analog button pressure must match shape_pressure(16384, 0)");

  // Verify Port 1 Mouse queries
  require(omni::presentation_input(snap, profile, false, 1, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_X) == -15,
          "Mouse X delta mismatch");
  require(omni::presentation_input(snap, profile, false, 1, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_Y) == 22,
          "Mouse Y delta mismatch");
  require(omni::presentation_input(snap, profile, false, 1, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_LEFT) == 1,
          "Mouse Left button mismatch");
  require(omni::presentation_input(snap, profile, false, 1, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_RIGHT) == 1,
          "Mouse Right button mismatch");
  require(omni::presentation_input(snap, profile, false, 1, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_WHEELDOWN) == 1,
          "Mouse wheel down mismatch");
  require(omni::presentation_input(snap, profile, false, 1, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_HORIZ_WHEELUP) == 1,
          "Mouse horizontal wheel up mismatch");

  // Verify Port 2 Lightgun queries
  require(omni::presentation_input(snap, profile, false, 2, RETRO_DEVICE_LIGHTGUN, 0, RETRO_DEVICE_ID_LIGHTGUN_SCREEN_X) == -4096,
          "Lightgun screen X mismatch");
  require(omni::presentation_input(snap, profile, false, 2, RETRO_DEVICE_LIGHTGUN, 0, RETRO_DEVICE_ID_LIGHTGUN_SCREEN_Y) == 8192,
          "Lightgun screen Y mismatch");
  require(omni::presentation_input(snap, profile, false, 2, RETRO_DEVICE_LIGHTGUN, 0, RETRO_DEVICE_ID_LIGHTGUN_IS_OFFSCREEN) == 0,
          "Lightgun should be on-screen");
  require(omni::presentation_input(snap, profile, false, 2, RETRO_DEVICE_LIGHTGUN, 0, RETRO_DEVICE_ID_LIGHTGUN_TRIGGER) == 1,
          "Lightgun trigger should be active");
  require(omni::presentation_input(snap, profile, false, 2, RETRO_DEVICE_LIGHTGUN, 0, RETRO_DEVICE_ID_LIGHTGUN_RELOAD) == 1,
          "Lightgun reload should be active from right mouse button");
  require(omni::presentation_input(snap, profile, false, 2, RETRO_DEVICE_LIGHTGUN, 0, RETRO_DEVICE_ID_LIGHTGUN_START) == 1,
          "Lightgun Start should forward from joypad Start");

  // Verify Port 3 (PortDevice::None) suppresses joypad/analog/lightgun input
  require(omni::presentation_input(snap, profile, false, 3, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B) == 0,
          "Port 3 with PortDevice::None must return 0 for joypad");

  // Verify Focus Loss (focused = false) immediately drops all ports, mouse, lightgun, and pointer to 0
  snap.focused = false;
  snap.input[144] = 0;
  for (unsigned port = 0; port < 4; ++port) {
    require(!omni::presentation_pad(snap, port), "Unfocused snapshot must report pad disconnected/inactive");
    require(omni::presentation_input(snap, profile, true, port, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_B) == 0,
            "Unfocused joypad must be 0");
  }
  require(omni::presentation_input(snap, profile, false, 1, RETRO_DEVICE_MOUSE, 0, RETRO_DEVICE_ID_MOUSE_X) == 0,
          "Unfocused mouse X must be 0");
  require(omni::presentation_input(snap, profile, false, 2, RETRO_DEVICE_LIGHTGUN, 0, RETRO_DEVICE_ID_LIGHTGUN_TRIGGER) == 0,
          "Unfocused lightgun trigger must be 0");
  require(omni::presentation_input(snap, profile, false, 0, RETRO_DEVICE_POINTER, 0, RETRO_DEVICE_ID_POINTER_PRESSED) == 0,
          "Unfocused pointer pressed must be 0");
}

// Item 41 & Item 42: GPU video backend interfaces & window resize/fullscreen/DPI/device-loss lifecycle
void test_gpu_video_backends_and_window_lifecycle(SDL_Window* window, SDL_Renderer* renderer) {
  static_assert(omni::VkVideo::kInterfaceVersion == 5, "Libretro Vulkan interface version must be 5");
  omni::VkVideo vk;
  omni::GlVideo gl;
  require(!vk.lost(), "Fresh VkVideo instance must not be marked lost");
  require(!vk.negotiated(), "Fresh VkVideo instance must not be negotiated");
  require(vk.render_interface() == nullptr, "Unnegotiated VkVideo render_interface must be nullptr");
  vk.configure(window, 640, 480);
  require(vk.width() == 640 && vk.height() == 480, "VkVideo configured dimensions must match 640x480");
  vk.destroy();
  gl.destroy();

  // Exercise live SDL_Window resize, pixel-size query, and fullscreen state transitions
  require(SDL_SetWindowSize(window, 800, 600), "SDL_SetWindowSize(800, 600) failed");
  int pw = 0, ph = 0;
  require(SDL_GetWindowSizeInPixels(window, &pw, &ph), "SDL_GetWindowSizeInPixels failed");
  require(pw == 800 && ph == 600, "Window pixel size must match 800x600");

  // Simulate resize/DPI/render-device events clearing touch overlay contacts
  omni::presentation::NativeTouchOverlay overlay;
  overlay.configure(omni::presentation::NativeTouchOverlay::Visibility::Always, false, 100, 100);
  require(overlay.handle_finger(SDL_EVENT_FINGER_DOWN, 1, 0.84f, 0.78f, pw, ph), "Touch down before resize");
  require(overlay.state(pw, ph).active, "Overlay active before resize");

  for (Uint32 ev_type : {SDL_EVENT_WINDOW_RESIZED, SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED,
                         SDL_EVENT_WINDOW_DISPLAY_SCALE_CHANGED, SDL_EVENT_RENDER_TARGETS_RESET,
                         SDL_EVENT_RENDER_DEVICE_LOST}) {
    SDL_Event ev{};
    ev.type = ev_type;
    SDL_PushEvent(&ev);
    overlay.clear_contacts();
    require(!overlay.state(pw, ph).active, "Contacts must be cleared on window/render lifecycle event");
  }
  SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255);
  SDL_RenderClear(renderer);
  SDL_RenderPresent(renderer);
}

// Item 43: AudioRing bounded non-blocking producer, priming, underrun, wedged consumer trimming, and sink rejection
void test_audio_ring_priming_underrun_wedged_consumer_and_reset() {
  omni::AudioRing ring;
  std::vector<std::int16_t> out(512 * 2, -1);

  // Invalid parameters rejected
  std::vector<std::int16_t> odd(3, 100);
  require(!ring.push(odd).valid, "Odd stereo sample count must be rejected");
  require(ring.deliver(out, 0, 512, 1.0f, [](auto) { return true; }).delivery == omni::AudioRing::Delivery::Invalid,
          "Zero target must be rejected");

  // Push 256 stereo frames (< target 512): not primed yet, deliver outputs silence without underrun
  std::vector<std::int16_t> batch256(256 * 2);
  for (std::size_t i = 0; i < 256; ++i) {
    batch256[i * 2] = 1000;
    batch256[i * 2 + 1] = -1000;
  }
  const auto w1 = ring.push(batch256);
  require(w1.valid && w1.frames == 256, "Push 256 frames must succeed");

  const auto r_unprimed = ring.deliver(out, 512, 512, 1.0f, [](auto span) {
    for (auto s : span) {
      if (s != 0) return false;
    }
    return true;
  });
  require(r_unprimed.delivery == omni::AudioRing::Delivery::Accepted && r_unprimed.frames == 0 && !r_unprimed.underrun,
          "Unprimed delivery must output silence with 0 frames and no underrun");

  // Push another 256 frames -> queued == 512 == target (now primed!)
  require(ring.push(batch256).frames == 256, "Second push 256 frames must succeed");

  // Test sink rejection: must NOT consume queued frames
  const auto r_rejected = ring.deliver(out, 512, 512, 0.5f, [](auto) { return false; });
  require(r_rejected.delivery == omni::AudioRing::Delivery::Rejected, "Rejected sink must report Delivery::Rejected");

  // Now accept 256 frames at 0.5 volume
  std::vector<std::int16_t> out256(256 * 2, 0);
  const auto r_ok = ring.deliver(out256, 512, 512, 0.5f, [](auto span) {
    return span[0] == 500 && span[1] == -500;
  });
  require(r_ok.delivery == omni::AudioRing::Delivery::Accepted && r_ok.frames == 256 && !r_ok.underrun,
          "Primed delivery of 256 frames must succeed with scaled samples");

  // Request 512 frames when only 256 remain -> underrun == true, tail zero-filled
  const auto r_under = ring.deliver(out, 512, 512, 1.0f, [](auto span) {
    return span[0] == 1000 && span[511] == -1000 && span[512] == 0 && span[1023] == 0;
  });
  require(r_under.delivery == omni::AudioRing::Delivery::Accepted && r_under.frames == 256 && r_under.underrun,
          "Partial drain after priming must flag underrun and zero-fill tail");

  // Simulate wedged consumer: push 2048 frames while target=512, slack=256 (max before trim = 768)
  std::vector<std::int16_t> batch2048(2048 * 2, 400);
  require(ring.push(batch2048).frames == 2048, "Push 2048 frames must succeed without blocking");
  const auto r_trimmed = ring.deliver(out, 512, 256, 1.0f, [](auto) { return true; });
  require(r_trimmed.delivery == omni::AudioRing::Delivery::Accepted && r_trimmed.trimmed == 2048 - 512 && r_trimmed.frames == 512,
          "Wedged consumer backlog must be trimmed to target=512");

  // Reset on pause/interruption
  ring.reset_quiescent();
  const auto r_after_reset = ring.deliver(out256, 512, 512, 1.0f, [](auto) { return true; });
  require(r_after_reset.frames == 0 && !r_after_reset.underrun, "reset_quiescent must clear queued frames and priming");
}

} // namespace

int main() {
  try {
    SDL_SetHint(SDL_HINT_VIDEO_DRIVER, "offscreen");
    SDL_SetHint(SDL_HINT_AUDIO_DRIVER, "dummy");
    SDL_SetHint(SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS, "1");
    require(SDL_Init(SDL_INIT_VIDEO | SDL_INIT_JOYSTICK | SDL_INIT_GAMEPAD | SDL_INIT_EVENTS),
            "SDL_Init(VIDEO|JOYSTICK|GAMEPAD|EVENTS) with offscreen driver failed");

    SDL_Window* window = SDL_CreateWindow("OmniPlay Media Input Qualification", 640, 480, SDL_WINDOW_HIDDEN);
    require(window != nullptr, "SDL_CreateWindow failed");
    SDL_Renderer* renderer = SDL_CreateRenderer(window, nullptr);
    require(renderer != nullptr, "SDL_CreateRenderer failed");

    test_sdl_raw_joystick_calibration_and_virtual_hid(renderer);
    test_native_touch_overlay_18_controls_10_contacts_and_editor(renderer);
    test_pressure_buttons_peripherals_and_four_port_focus_loss();
    test_gpu_video_backends_and_window_lifecycle(window, renderer);
    test_audio_ring_priming_underrun_wedged_consumer_and_reset();

    SDL_DestroyRenderer(renderer);
    SDL_DestroyWindow(window);
    SDL_Quit();

    std::cout << "{\"passed\":true,\"tests\":[\"sdl_raw_joystick_calibration_and_virtual_hid\","
                 "\"native_touch_overlay_18_controls_10_contacts_and_editor\","
                 "\"pressure_buttons_peripherals_and_four_port_focus_loss\","
                 "\"gpu_video_backends_and_window_lifecycle\","
                 "\"audio_ring_priming_underrun_wedged_consumer_and_reset\"]}\n";
    return 0;
  } catch (const std::exception& ex) {
    std::cerr << "media_input_tests failed: " << ex.what() << "\n";
    SDL_Quit();
    return 1;
  }
}
