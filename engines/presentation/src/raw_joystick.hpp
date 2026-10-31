#pragma once
#include <SDL3/SDL.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace omni::presentation {

struct RawAxisSpec {
  int source = -1;
  std::int16_t minimum = -32767;
  std::int16_t center = 0;
  std::int16_t maximum = 32767;
};

enum class RawBindKind : std::uint8_t {
  None = 0,
  Button = 1,
  AxisPositive = 2,
  AxisNegative = 3,
  Hat = 4,
};

struct RawButtonSpec {
  RawBindKind kind = RawBindKind::None;
  std::uint8_t index = 0;
  std::uint8_t hat_mask = 0;
  std::uint8_t threshold_percent = 50;
};

struct SdlJoystickCalibration {
  std::string guid_hex;
  int axis_count = 0;
  int button_count = 0;
  // Logical axes: 0=LX, 1=LY, 2=RX, 3=RY, 4=LT, 5=RT
  std::array<RawAxisSpec, 6> axes{};
  // 16 logical buttons matching InputProfile::buttons indices:
  // 0=South(B), 1=West(Y), 2=Back(Select), 3=Start, 4=Up, 5=Down, 6=Left, 7=Right,
  // 8=East(A), 9=North(X), 10=L1, 11=R1, 12=L2, 13=R2, 14=L3, 15=R3
  std::array<RawButtonSpec, 16> buttons{};

  static SdlJoystickCalibration heuristic(const std::string& guid, int num_axes, int num_buttons, int num_hats) {
    SdlJoystickCalibration cal;
    cal.guid_hex = guid;
    cal.axis_count = std::clamp(num_axes, 0, 32);
    cal.button_count = std::clamp(num_buttons, 0, 64);
    for (int i = 0; i < 6; ++i) {
      cal.axes[static_cast<std::size_t>(i)].source = (i < cal.axis_count) ? i : -1;
      if (i >= 4) {
        cal.axes[static_cast<std::size_t>(i)].center = -32767;
      }
    }
    constexpr std::array<int, 12> default_btns{0, 2, 6, 7, -1, -1, -1, -1, 1, 3, 4, 5};
    for (std::size_t b = 0; b < default_btns.size(); ++b) {
      const int idx = default_btns[b];
      if (idx >= 0 && idx < cal.button_count) {
        cal.buttons[b] = {RawBindKind::Button, static_cast<std::uint8_t>(idx), 0, 50};
      }
    }
    if (num_hats > 0) {
      cal.buttons[4] = {RawBindKind::Hat, 0, SDL_HAT_UP, 50};
      cal.buttons[5] = {RawBindKind::Hat, 0, SDL_HAT_DOWN, 50};
      cal.buttons[6] = {RawBindKind::Hat, 0, SDL_HAT_LEFT, 50};
      cal.buttons[7] = {RawBindKind::Hat, 0, SDL_HAT_RIGHT, 50};
    }
    if (cal.button_count > 8) cal.buttons[14] = {RawBindKind::Button, 8, 0, 50};
    if (cal.button_count > 9) cal.buttons[15] = {RawBindKind::Button, 9, 0, 50};
    if (cal.axis_count > 4) cal.buttons[12] = {RawBindKind::AxisPositive, 4, 0, 50};
    if (cal.axis_count > 5) cal.buttons[13] = {RawBindKind::AxisPositive, 5, 0, 50};
    return cal;
  }
};

struct SampledPadState {
  bool connected = false;
  std::uint32_t sdl_button_mask = 0;
  std::array<std::int16_t, 6> axes{};
};

class RawJoystickManager {
  enum class UiPhase { Hidden, Overview, CaptureCenter, CaptureRange, BindButtons };

  std::array<SDL_Joystick*, 4> joysticks_{};
  std::vector<SdlJoystickCalibration> calibrations_;
  UiPhase phase_ = UiPhase::Hidden;
  unsigned active_slot_ = 0;
  unsigned bind_index_ = 0;
  SdlJoystickCalibration working_{};
  std::array<std::int16_t, 32> resting_axes_{};
  std::array<std::int16_t, 32> min_axes_{};
  std::array<std::int16_t, 32> max_axes_{};
  std::array<bool, 64> prev_buttons_{};
  Uint64 bind_cooldown_until_ms_ = 0;

  static std::string joystick_guid(SDL_Joystick* joy) {
    if (!joy) return {};
    char buf[64]{};
    SDL_GUIDToString(SDL_GetJoystickGUID(joy), buf, sizeof(buf));
    return std::string(buf);
  }

  const SdlJoystickCalibration* find_calibration(SDL_Joystick* joy) const {
    if (!joy) return nullptr;
    const auto guid = joystick_guid(joy);
    const int axes = std::clamp(SDL_GetNumJoystickAxes(joy), 0, 32);
    const int buttons = std::clamp(SDL_GetNumJoystickButtons(joy), 0, 64);
    for (const auto& cal : calibrations_) {
      if (cal.guid_hex == guid && cal.axis_count == axes && cal.button_count == buttons) {
        return &cal;
      }
    }
    return nullptr;
  }

  void upsert_calibration(SdlJoystickCalibration cal) {
    for (auto& existing : calibrations_) {
      if (existing.guid_hex == cal.guid_hex && existing.axis_count == cal.axis_count && existing.button_count == cal.button_count) {
        existing = std::move(cal);
        return;
      }
    }
    if (calibrations_.size() < 16) {
      calibrations_.push_back(std::move(cal));
    }
  }

  static std::int16_t normalize_axis(std::int16_t raw, const RawAxisSpec& spec, bool trigger) {
    if (trigger) {
      const double span = static_cast<double>(spec.maximum) - static_cast<double>(spec.minimum);
      if (span < 256.0) return 0;
      const double t = std::clamp((static_cast<double>(raw) - static_cast<double>(spec.minimum)) / span, 0.0, 1.0);
      return static_cast<std::int16_t>(std::lround(t * 32767.0));
    }
    if (raw >= spec.center) {
      const double span = std::max(256.0, static_cast<double>(spec.maximum) - static_cast<double>(spec.center));
      const double t = std::clamp((static_cast<double>(raw) - static_cast<double>(spec.center)) / span, 0.0, 1.0);
      return static_cast<std::int16_t>(std::lround(t * 32767.0));
    }
    const double span = std::max(256.0, static_cast<double>(spec.center) - static_cast<double>(spec.minimum));
    const double t = std::clamp((static_cast<double>(spec.center) - static_cast<double>(raw)) / span, 0.0, 1.0);
    return static_cast<std::int16_t>(-std::lround(t * 32767.0));
  }

  static bool button_pressed(SDL_Joystick* joy, const RawButtonSpec& spec, const std::array<RawAxisSpec, 6>& axes) {
    if (!joy) return false;
    switch (spec.kind) {
      case RawBindKind::None:
        return false;
      case RawBindKind::Button:
        return spec.index < SDL_GetNumJoystickButtons(joy) && SDL_GetJoystickButton(joy, spec.index);
      case RawBindKind::Hat:
        return spec.index < SDL_GetNumJoystickHats(joy) && (SDL_GetJoystickHat(joy, spec.index) & spec.hat_mask) != 0;
      case RawBindKind::AxisPositive:
      case RawBindKind::AxisNegative: {
        if (spec.index >= SDL_GetNumJoystickAxes(joy)) return false;
        const std::int16_t raw = SDL_GetJoystickAxis(joy, spec.index);
        RawAxisSpec ref{-1, -32767, 0, 32767};
        for (const auto& ax : axes) {
          if (ax.source == static_cast<int>(spec.index)) {
            ref = ax;
            break;
          }
        }
        const double threshold = std::clamp(static_cast<double>(spec.threshold_percent) / 100.0, 0.15, 0.95);
        if (spec.kind == RawBindKind::AxisPositive) {
          const double span = std::max(512.0, static_cast<double>(ref.maximum) - static_cast<double>(ref.center));
          return (static_cast<double>(raw) - static_cast<double>(ref.center)) / span >= threshold;
        }
        const double span = std::max(512.0, static_cast<double>(ref.center) - static_cast<double>(ref.minimum));
        return (static_cast<double>(ref.center) - static_cast<double>(raw)) / span >= threshold;
      }
    }
    return false;
  }

  void snapshot_resting(SDL_Joystick* joy) {
    resting_axes_.fill(0);
    min_axes_.fill(0);
    max_axes_.fill(0);
    prev_buttons_.fill(false);
    if (!joy) return;
    const int n_axes = std::clamp(SDL_GetNumJoystickAxes(joy), 0, 32);
    for (int i = 0; i < n_axes; ++i) {
      const auto v = SDL_GetJoystickAxis(joy, i);
      resting_axes_[static_cast<std::size_t>(i)] = v;
      min_axes_[static_cast<std::size_t>(i)] = v;
      max_axes_[static_cast<std::size_t>(i)] = v;
    }
    const int n_btns = std::clamp(SDL_GetNumJoystickButtons(joy), 0, 64);
    for (int i = 0; i < n_btns; ++i) {
      prev_buttons_[static_cast<std::size_t>(i)] = SDL_GetJoystickButton(joy, i);
    }
  }

public:
  void load_profiles(const std::vector<SdlJoystickCalibration>& profiles) {
    calibrations_ = profiles;
  }

  bool calibration_active() const {
    return phase_ != UiPhase::Hidden;
  }

  void close_ui() {
    phase_ = UiPhase::Hidden;
  }

  void refresh_devices(const std::array<SDL_Gamepad*, 4>& gamepads) {
    for (std::size_t i = 0; i < 4; ++i) {
      if (gamepads[i] && joysticks_[i]) {
        SDL_CloseJoystick(joysticks_[i]);
        joysticks_[i] = nullptr;
      }
      if (joysticks_[i] && !SDL_JoystickConnected(joysticks_[i])) {
        SDL_CloseJoystick(joysticks_[i]);
        joysticks_[i] = nullptr;
      }
    }
    int count = 0;
    SDL_JoystickID* ids = SDL_GetJoysticks(&count);
    if (!ids) return;
    const int bound = std::clamp(count, 0, 32);
    for (int i = 0; i < bound; ++i) {
      const SDL_JoystickID id = ids[i];
      if (SDL_IsGamepad(id)) continue;
      bool already_open = false;
      for (auto* joy : joysticks_) {
        if (joy && SDL_GetJoystickID(joy) == id) {
          already_open = true;
          break;
        }
      }
      if (already_open) continue;
      for (std::size_t slot = 0; slot < 4; ++slot) {
        if (!gamepads[slot] && !joysticks_[slot]) {
          SDL_Joystick* opened = SDL_OpenJoystick(id);
          if (opened) {
            joysticks_[slot] = opened;
            if (!find_calibration(opened)) {
              upsert_calibration(SdlJoystickCalibration::heuristic(
                  joystick_guid(opened),
                  SDL_GetNumJoystickAxes(opened),
                  SDL_GetNumJoystickButtons(opened),
                  SDL_GetNumJoystickHats(opened)));
            }
          }
          break;
        }
      }
    }
    SDL_free(ids);
  }

  bool remove_device(SDL_JoystickID id) {
    bool owned = false;
    for (std::size_t slot = 0; slot < 4; ++slot) {
      if (joysticks_[slot] && SDL_GetJoystickID(joysticks_[slot]) == id) {
        SDL_CloseJoystick(joysticks_[slot]);
        joysticks_[slot] = nullptr;
        owned = true;
        if (phase_ != UiPhase::Hidden && active_slot_ == slot) {
          phase_ = UiPhase::Overview;
        }
      }
    }
    return owned;
  }

  void shutdown() {
    phase_ = UiPhase::Hidden;
    for (auto*& joy : joysticks_) {
      if (joy) {
        SDL_CloseJoystick(joy);
        joy = nullptr;
      }
    }
  }

  bool handle_key(SDL_Scancode scancode) {
    if (scancode == SDL_SCANCODE_F10) {
      if (phase_ == UiPhase::Hidden) {
        phase_ = UiPhase::Overview;
        for (unsigned s = 0; s < 4; ++s) {
          if (joysticks_[s]) {
            active_slot_ = s;
            break;
          }
        }
      } else {
        phase_ = UiPhase::Hidden;
      }
      return true;
    }
    if (phase_ == UiPhase::Hidden) return false;

    if (scancode == SDL_SCANCODE_ESCAPE) {
      phase_ = UiPhase::Hidden;
      return true;
    }
    if (phase_ == UiPhase::Overview) {
      if (scancode == SDL_SCANCODE_TAB || scancode == SDL_SCANCODE_RIGHT) {
        active_slot_ = (active_slot_ + 1) % 4;
        return true;
      }
      if (scancode == SDL_SCANCODE_LEFT) {
        active_slot_ = (active_slot_ + 3) % 4;
        return true;
      }
      if ((scancode == SDL_SCANCODE_RETURN || scancode == SDL_SCANCODE_SPACE) && joysticks_[active_slot_]) {
        SDL_Joystick* joy = joysticks_[active_slot_];
        const auto* current = find_calibration(joy);
        working_ = current ? *current : SdlJoystickCalibration::heuristic(
            joystick_guid(joy),
            SDL_GetNumJoystickAxes(joy),
            SDL_GetNumJoystickButtons(joy),
            SDL_GetNumJoystickHats(joy));
        snapshot_resting(joy);
        phase_ = UiPhase::CaptureCenter;
        return true;
      }
      return true;
    }
    SDL_Joystick* joy = joysticks_[active_slot_];
    if (!joy) {
      phase_ = UiPhase::Overview;
      return true;
    }
    if (phase_ == UiPhase::CaptureCenter) {
      if (scancode == SDL_SCANCODE_RETURN || scancode == SDL_SCANCODE_SPACE) {
        snapshot_resting(joy);
        for (std::size_t i = 0; i < 6; ++i) {
          const int src = working_.axes[i].source;
          if (src >= 0 && src < 32) {
            working_.axes[i].center = resting_axes_[static_cast<std::size_t>(src)];
          }
        }
        phase_ = UiPhase::CaptureRange;
      }
      return true;
    }
    if (phase_ == UiPhase::CaptureRange) {
      if (scancode == SDL_SCANCODE_RETURN || scancode == SDL_SCANCODE_SPACE) {
        int assigned_stick_axis = 0;
        const int n_axes = std::clamp(SDL_GetNumJoystickAxes(joy), 0, 32);
        for (auto& ax : working_.axes) ax.source = -1;
        for (int a = 0; a < n_axes && assigned_stick_axis < 6; ++a) {
          const int span = static_cast<int>(max_axes_[static_cast<std::size_t>(a)]) - static_cast<int>(min_axes_[static_cast<std::size_t>(a)]);
          if (span >= 8192) {
            working_.axes[static_cast<std::size_t>(assigned_stick_axis)] = {
                a,
                min_axes_[static_cast<std::size_t>(a)],
                resting_axes_[static_cast<std::size_t>(a)],
                max_axes_[static_cast<std::size_t>(a)]};
            ++assigned_stick_axis;
          }
        }
        bind_index_ = 0;
        snapshot_resting(joy);
        bind_cooldown_until_ms_ = SDL_GetTicks() + 250;
        phase_ = UiPhase::BindButtons;
      }
      return true;
    }
    if (phase_ == UiPhase::BindButtons) {
      if (scancode == SDL_SCANCODE_BACKSPACE) {
        working_.buttons[bind_index_] = {RawBindKind::None, 0, 0, 50};
        if (++bind_index_ >= 16) {
          upsert_calibration(working_);
          phase_ = UiPhase::Overview;
        } else {
          bind_cooldown_until_ms_ = SDL_GetTicks() + 200;
        }
      }
      return true;
    }
    return true;
  }

  void tick() {
    if (phase_ == UiPhase::Hidden) return;
    SDL_Joystick* joy = joysticks_[active_slot_];
    if (!joy || !SDL_JoystickConnected(joy)) return;
    const int n_axes = std::clamp(SDL_GetNumJoystickAxes(joy), 0, 32);
    if (phase_ == UiPhase::CaptureRange) {
      for (int i = 0; i < n_axes; ++i) {
        const auto v = SDL_GetJoystickAxis(joy, i);
        min_axes_[static_cast<std::size_t>(i)] = std::min(min_axes_[static_cast<std::size_t>(i)], v);
        max_axes_[static_cast<std::size_t>(i)] = std::max(max_axes_[static_cast<std::size_t>(i)], v);
      }
      return;
    }
    if (phase_ == UiPhase::BindButtons && SDL_GetTicks() >= bind_cooldown_until_ms_) {
      const int n_btns = std::clamp(SDL_GetNumJoystickButtons(joy), 0, 64);
      bool bound = false;
      for (int b = 0; b < n_btns; ++b) {
        const bool down = SDL_GetJoystickButton(joy, b);
        if (down && !prev_buttons_[static_cast<std::size_t>(b)]) {
          working_.buttons[bind_index_] = {RawBindKind::Button, static_cast<std::uint8_t>(b), 0, 50};
          bound = true;
          break;
        }
        prev_buttons_[static_cast<std::size_t>(b)] = down;
      }
      const int n_hats = std::clamp(SDL_GetNumJoystickHats(joy), 0, 8);
      for (int h = 0; !bound && h < n_hats; ++h) {
        const Uint8 hv = SDL_GetJoystickHat(joy, h);
        for (Uint8 mask : {SDL_HAT_UP, SDL_HAT_DOWN, SDL_HAT_LEFT, SDL_HAT_RIGHT}) {
          if ((hv & mask) != 0) {
            working_.buttons[bind_index_] = {RawBindKind::Hat, static_cast<std::uint8_t>(h), mask, 50};
            bound = true;
            break;
          }
        }
      }
      for (int a = 0; !bound && a < n_axes; ++a) {
        const int delta = static_cast<int>(SDL_GetJoystickAxis(joy, a)) - static_cast<int>(resting_axes_[static_cast<std::size_t>(a)]);
        if (std::abs(delta) > 18000) {
          working_.buttons[bind_index_] = {
              delta > 0 ? RawBindKind::AxisPositive : RawBindKind::AxisNegative,
              static_cast<std::uint8_t>(a),
              0,
              55};
          bound = true;
        }
      }
      if (bound) {
        snapshot_resting(joy);
        if (++bind_index_ >= 16) {
          upsert_calibration(working_);
          phase_ = UiPhase::Overview;
        } else {
          bind_cooldown_until_ms_ = SDL_GetTicks() + 300;
        }
      }
    }
  }

  SampledPadState sample_port(unsigned port) const {
    SampledPadState out{};
    if (port >= 4 || phase_ != UiPhase::Hidden) return out;
    SDL_Joystick* joy = joysticks_[port];
    if (!joy || !SDL_JoystickConnected(joy)) return out;
    const auto* cal = find_calibration(joy);
    if (!cal) return out;
    out.connected = true;

    constexpr std::array<SDL_GamepadButton, 16> kSdlMap{
        SDL_GAMEPAD_BUTTON_SOUTH, SDL_GAMEPAD_BUTTON_WEST, SDL_GAMEPAD_BUTTON_BACK, SDL_GAMEPAD_BUTTON_START,
        SDL_GAMEPAD_BUTTON_DPAD_UP, SDL_GAMEPAD_BUTTON_DPAD_DOWN, SDL_GAMEPAD_BUTTON_DPAD_LEFT, SDL_GAMEPAD_BUTTON_DPAD_RIGHT,
        SDL_GAMEPAD_BUTTON_EAST, SDL_GAMEPAD_BUTTON_NORTH, SDL_GAMEPAD_BUTTON_LEFT_SHOULDER, SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER,
        SDL_GAMEPAD_BUTTON_INVALID, SDL_GAMEPAD_BUTTON_INVALID, SDL_GAMEPAD_BUTTON_LEFT_STICK, SDL_GAMEPAD_BUTTON_RIGHT_STICK};

    for (std::size_t i = 0; i < 16; ++i) {
      if (button_pressed(joy, cal->buttons[i], cal->axes)) {
        if (kSdlMap[i] != SDL_GAMEPAD_BUTTON_INVALID) {
          out.sdl_button_mask |= (1u << static_cast<unsigned>(kSdlMap[i]));
        } else if (i == 12) {
          out.axes[4] = 32767;
        } else if (i == 13) {
          out.axes[5] = 32767;
        }
      }
    }
    for (std::size_t a = 0; a < 6; ++a) {
      const int src = cal->axes[a].source;
      if (src >= 0 && src < SDL_GetNumJoystickAxes(joy)) {
        const auto v = normalize_axis(SDL_GetJoystickAxis(joy, src), cal->axes[a], a >= 4);
        if (a >= 4) {
          out.axes[a] = std::max(out.axes[a], v);
        } else {
          out.axes[a] = v;
        }
      }
    }
    return out;
  }

  void render_overlay(SDL_Renderer* renderer, unsigned logical_w, unsigned logical_h) const {
    if (phase_ == UiPhase::Hidden || !renderer || logical_w < 160 || logical_h < 120) return;
    SDL_BlendMode old_blend = SDL_BLENDMODE_NONE;
    SDL_GetRenderDrawBlendMode(renderer, &old_blend);
    SDL_SetRenderDrawBlendMode(renderer, SDL_BLENDMODE_BLEND);

    const float panel_w = std::min(static_cast<float>(logical_w) - 24.0f, 520.0f);
    const float panel_h = std::min(static_cast<float>(logical_h) - 24.0f, 300.0f);
    const SDL_FRect panel{(static_cast<float>(logical_w) - panel_w) * 0.5f,
                          (static_cast<float>(logical_h) - panel_h) * 0.5f,
                          panel_w, panel_h};
    SDL_SetRenderDrawColor(renderer, 12, 18, 16, 232);
    SDL_RenderFillRect(renderer, &panel);
    SDL_SetRenderDrawColor(renderer, 78, 186, 132, 255);
    SDL_RenderRect(renderer, &panel);

    const float tx = panel.x + 14.0f;
    float ty = panel.y + 14.0f;
    SDL_SetRenderDrawColor(renderer, 235, 245, 240, 255);
    SDL_RenderDebugText(renderer, tx, ty, "SDL RAW JOYSTICK CALIBRATION (F10/Esc: Close)");
    ty += 18.0f;

    char line[160]{};
    SDL_Joystick* joy = joysticks_[active_slot_];
    const char* name = joy ? SDL_GetJoystickName(joy) : nullptr;
    std::snprintf(line, sizeof(line), "Slot %u: %s", active_slot_ + 1, name ? name : "[No Raw Joystick]");
    SDL_RenderDebugText(renderer, tx, ty, line);
    ty += 16.0f;

    if (!joy) {
      SDL_RenderDebugText(renderer, tx, ty, "Tab/Arrows: Switch slot | Connect unmapped USB/HID pad");
      SDL_SetRenderDrawBlendMode(renderer, old_blend);
      return;
    }

    if (phase_ == UiPhase::Overview) {
      std::snprintf(line, sizeof(line), "Axes: %d  Buttons: %d  Hats: %d",
                    SDL_GetNumJoystickAxes(joy), SDL_GetNumJoystickButtons(joy), SDL_GetNumJoystickHats(joy));
      SDL_RenderDebugText(renderer, tx, ty, line);
      ty += 16.0f;
      SDL_RenderDebugText(renderer, tx, ty, "Enter/Space: Calibrate | Tab: Next Slot");
      ty += 20.0f;
      const int show_axes = std::min(6, SDL_GetNumJoystickAxes(joy));
      for (int a = 0; a < show_axes && ty + 14.0f < panel.y + panel.h; ++a) {
        const int v = SDL_GetJoystickAxis(joy, a);
        std::snprintf(line, sizeof(line), "Axis %d: %6d", a, v);
        SDL_RenderDebugText(renderer, tx, ty, line);
        const SDL_FRect bar_bg{tx + 130.0f, ty + 1.0f, 160.0f, 8.0f};
        SDL_SetRenderDrawColor(renderer, 45, 58, 52, 255);
        SDL_RenderFillRect(renderer, &bar_bg);
        const float norm = std::clamp((static_cast<float>(v) + 32768.0f) / 65535.0f, 0.0f, 1.0f);
        const SDL_FRect bar_fg{tx + 130.0f, ty + 1.0f, 160.0f * norm, 8.0f};
        SDL_SetRenderDrawColor(renderer, 78, 186, 132, 255);
        SDL_RenderFillRect(renderer, &bar_fg);
        SDL_SetRenderDrawColor(renderer, 235, 245, 240, 255);
        ty += 14.0f;
      }
    } else if (phase_ == UiPhase::CaptureCenter) {
      SDL_RenderDebugText(renderer, tx, ty, "STEP 1/3: Release all sticks and triggers to neutral.");
      ty += 18.0f;
      SDL_RenderDebugText(renderer, tx, ty, "Press Enter or Space to capture resting center.");
    } else if (phase_ == UiPhase::CaptureRange) {
      SDL_RenderDebugText(renderer, tx, ty, "STEP 2/3: Rotate sticks & pull triggers full range.");
      ty += 18.0f;
      SDL_RenderDebugText(renderer, tx, ty, "Press Enter or Space when finished.");
      ty += 18.0f;
      const int show_axes = std::min(6, SDL_GetNumJoystickAxes(joy));
      for (int a = 0; a < show_axes && ty + 14.0f < panel.y + panel.h; ++a) {
        std::snprintf(line, sizeof(line), "A%d [%6d .. %6d]", a,
                      static_cast<int>(min_axes_[static_cast<std::size_t>(a)]),
                      static_cast<int>(max_axes_[static_cast<std::size_t>(a)]));
        SDL_RenderDebugText(renderer, tx, ty, line);
        ty += 14.0f;
      }
    } else if (phase_ == UiPhase::BindButtons) {
      constexpr std::array<const char*, 16> kNames{
          "South (B/Cross)", "West (Y/Square)", "Select / Back", "Start",
          "D-Pad Up", "D-Pad Down", "D-Pad Left", "D-Pad Right",
          "East (A/Circle)", "North (X/Triangle)", "L1 Shoulder", "R1 Shoulder",
          "L2 Trigger", "R2 Trigger", "L3 Thumb", "R3 Thumb"};
      std::snprintf(line, sizeof(line), "STEP 3/3: Press input for [%s] (%u/16)",
                    kNames[bind_index_ % 16], bind_index_ + 1);
      SDL_RenderDebugText(renderer, tx, ty, line);
      ty += 18.0f;
      SDL_RenderDebugText(renderer, tx, ty, "Backspace: Skip/Unbind | Esc: Cancel");
    }

    SDL_SetRenderDrawBlendMode(renderer, old_blend);
  }
};

} // namespace omni::presentation
