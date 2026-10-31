#pragma once
#include <SDL3/SDL.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>

namespace omni::presentation {

struct TouchControlSpec {
  std::uint8_t id = 0;
  float nx = 0.5f;       // 0..1 normalized window/logical X
  float ny = 0.5f;       // 0..1 normalized window/logical Y
  float radius_px = 28.0f;
  bool visible = true;
  const char* label = "";
};

struct TouchOverlayOutput {
  bool active = false;
  std::uint32_t sdl_button_mask = 0;
  std::array<std::int16_t, 6> axes{};
};

class NativeTouchOverlay {
public:
  enum class Visibility { Auto, Always, Hidden };

private:
  struct Contact {
    bool active = false;
    SDL_FingerID finger = 0;
    std::uint8_t control_id = 0xff;
    float norm_x = 0.0f;
    float norm_y = 0.0f;
  };

  Visibility visibility_ = Visibility::Auto;
  bool touch_seen_ = false;
  bool left_handed_ = false;
  bool edit_mode_ = false;
  float size_scale_ = 1.0f;
  std::uint8_t opacity_alpha_ = 175;
  std::array<TouchControlSpec, 18> controls_{};
  std::array<Contact, 10> contacts_{};

  void reset_default_layout() {
    // IDs 0..15 match libretro/OmniPlay button indices; 16 = Left Stick, 17 = Right Stick.
    const auto lx = left_handed_ ? 0.84f : 0.16f;
    const auto rx = left_handed_ ? 0.16f : 0.84f;
    controls_ = {{
        {0, rx, 0.78f, 24.0f, true, "B"},
        {1, rx - 0.06f, 0.70f, 24.0f, true, "Y"},
        {2, 0.43f, 0.90f, 20.0f, true, "SEL"},
        {3, 0.57f, 0.90f, 20.0f, true, "STA"},
        {4, lx, 0.62f, 22.0f, true, "U"},
        {5, lx, 0.78f, 22.0f, true, "D"},
        {6, lx - 0.06f, 0.70f, 22.0f, true, "L"},
        {7, lx + 0.06f, 0.70f, 22.0f, true, "R"},
        {8, rx + 0.06f, 0.70f, 24.0f, true, "A"},
        {9, rx, 0.62f, 24.0f, true, "X"},
        {10, 0.12f, 0.18f, 26.0f, true, "L1"},
        {11, 0.88f, 0.18f, 26.0f, true, "R1"},
        {12, 0.12f, 0.08f, 24.0f, true, "L2"},
        {13, 0.88f, 0.08f, 24.0f, true, "R2"},
        {14, 0.30f, 0.90f, 18.0f, false, "L3"},
        {15, 0.70f, 0.90f, 18.0f, false, "R3"},
        {16, left_handed_ ? 0.72f : 0.28f, 0.82f, 42.0f, true, "LS"},
        {17, left_handed_ ? 0.28f : 0.72f, 0.82f, 42.0f, true, "RS"},
    }};
  }

  bool shown() const {
    if (visibility_ == Visibility::Hidden) return false;
    if (visibility_ == Visibility::Always || edit_mode_) return true;
    return touch_seen_;
  }

  int hit_test(float norm_x, float norm_y, float width, float height) const {
    if (width <= 0.0f || height <= 0.0f) return -1;
    const float px = norm_x * width;
    const float py = norm_y * height;
    int best = -1;
    float best_dist2 = 1e30f;
    for (int i = 0; i < 18; ++i) {
      const auto& c = controls_[static_cast<std::size_t>(i)];
      if (!c.visible && !edit_mode_) continue;
      const float cx = c.nx * width;
      const float cy = c.ny * height;
      const float r = c.radius_px * size_scale_ * 1.25f;
      const float dx = px - cx;
      const float dy = py - cy;
      const float d2 = dx * dx + dy * dy;
      if (d2 <= r * r && d2 < best_dist2) {
        best_dist2 = d2;
        best = i;
      }
    }
    return best;
  }

public:
  NativeTouchOverlay() {
    reset_default_layout();
  }

  void clear_contacts() {
    for (auto& c : contacts_) {
      c = Contact{};
    }
  }

  void configure(Visibility vis, bool left_handed, unsigned size_percent, unsigned opacity_percent) {
    visibility_ = vis;
    if (left_handed_ != left_handed) {
      left_handed_ = left_handed;
      reset_default_layout();
    }
    size_scale_ = std::clamp(static_cast<float>(size_percent) / 100.0f, 0.8f, 1.4f);
    opacity_alpha_ = static_cast<std::uint8_t>(std::clamp<unsigned>(opacity_percent * 255u / 100u, 80u, 240u));
    clear_contacts();
  }

  bool handle_key(SDL_Scancode scancode, SDL_Keymod mod) {
    if (scancode != SDL_SCANCODE_F9) return false;
    if ((mod & SDL_KMOD_SHIFT) != 0) {
      edit_mode_ = !edit_mode_;
      clear_contacts();
      return true;
    }
    if (visibility_ == Visibility::Auto) visibility_ = Visibility::Always;
    else if (visibility_ == Visibility::Always) visibility_ = Visibility::Hidden;
    else visibility_ = Visibility::Auto;
    clear_contacts();
    return true;
  }

  // Returns true if the touch contact is owned by the virtual controller overlay
  // (so caller should NOT forward that finger as a raw RETRO_DEVICE_POINTER tap).
  bool handle_finger(Uint32 event_type, SDL_FingerID finger, float norm_x, float norm_y, int win_w, int win_h) {
    if (!std::isfinite(norm_x) || !std::isfinite(norm_y) || win_w <= 0 || win_h <= 0) return false;
    norm_x = std::clamp(norm_x, 0.0f, 1.0f);
    norm_y = std::clamp(norm_y, 0.0f, 1.0f);
    if (event_type == SDL_EVENT_FINGER_DOWN && visibility_ != Visibility::Hidden) {
      touch_seen_ = true;
    }
    if (!shown()) return false;

    for (auto& c : contacts_) {
      if (c.active && c.finger == finger) {
        if (event_type == SDL_EVENT_FINGER_UP || event_type == SDL_EVENT_FINGER_CANCELED) {
          c = Contact{};
          return true;
        }
        if (event_type == SDL_EVENT_FINGER_MOTION) {
          c.norm_x = norm_x;
          c.norm_y = norm_y;
          if (edit_mode_ && c.control_id < 18) {
            controls_[c.control_id].nx = std::clamp(norm_x, 0.06f, 0.94f);
            controls_[c.control_id].ny = std::clamp(norm_y, 0.06f, 0.94f);
          } else if (c.control_id < 16) {
            const int hit = hit_test(norm_x, norm_y, static_cast<float>(win_w), static_cast<float>(win_h));
            if (hit >= 0 && hit < 16) {
              c.control_id = static_cast<std::uint8_t>(hit);
            }
          }
          return true;
        }
      }
    }

    if (event_type != SDL_EVENT_FINGER_DOWN) return false;
    const int hit = hit_test(norm_x, norm_y, static_cast<float>(win_w), static_cast<float>(win_h));
    if (hit < 0) return false;
    for (auto& c : contacts_) {
      if (!c.active) {
        c.active = true;
        c.finger = finger;
        c.control_id = static_cast<std::uint8_t>(hit);
        c.norm_x = norm_x;
        c.norm_y = norm_y;
        return true;
      }
    }
    return true;
  }

  TouchOverlayOutput state(int win_w, int win_h) const {
    TouchOverlayOutput out{};
    if (!shown() || edit_mode_ || win_w <= 0 || win_h <= 0) return out;
    constexpr std::array<SDL_GamepadButton, 16> kSdlMap{
        SDL_GAMEPAD_BUTTON_SOUTH, SDL_GAMEPAD_BUTTON_WEST, SDL_GAMEPAD_BUTTON_BACK, SDL_GAMEPAD_BUTTON_START,
        SDL_GAMEPAD_BUTTON_DPAD_UP, SDL_GAMEPAD_BUTTON_DPAD_DOWN, SDL_GAMEPAD_BUTTON_DPAD_LEFT, SDL_GAMEPAD_BUTTON_DPAD_RIGHT,
        SDL_GAMEPAD_BUTTON_EAST, SDL_GAMEPAD_BUTTON_NORTH, SDL_GAMEPAD_BUTTON_LEFT_SHOULDER, SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER,
        SDL_GAMEPAD_BUTTON_INVALID, SDL_GAMEPAD_BUTTON_INVALID, SDL_GAMEPAD_BUTTON_LEFT_STICK, SDL_GAMEPAD_BUTTON_RIGHT_STICK};

    for (const auto& c : contacts_) {
      if (!c.active || c.control_id >= 18) continue;
      out.active = true;
      if (c.control_id < 16) {
        const auto btn = kSdlMap[c.control_id];
        if (btn != SDL_GAMEPAD_BUTTON_INVALID) {
          out.sdl_button_mask |= (1u << static_cast<unsigned>(btn));
        } else if (c.control_id == 12) {
          out.axes[4] = 32767;
        } else if (c.control_id == 13) {
          out.axes[5] = 32767;
        }
      } else {
        const auto& spec = controls_[c.control_id];
        const float radius = std::max(16.0f, spec.radius_px * size_scale_);
        const float dx = (c.norm_x - spec.nx) * static_cast<float>(win_w) / radius;
        const float dy = (c.norm_y - spec.ny) * static_cast<float>(win_h) / radius;
        const float len = std::hypot(dx, dy);
        const float scale = len > 1.0f ? (1.0f / len) : 1.0f;
        const auto ax = static_cast<std::int16_t>(std::lround(std::clamp(dx * scale, -1.0f, 1.0f) * 32767.0f));
        const auto ay = static_cast<std::int16_t>(std::lround(std::clamp(dy * scale, -1.0f, 1.0f) * 32767.0f));
        const std::size_t base = (c.control_id == 16) ? 0u : 2u;
        out.axes[base] = ax;
        out.axes[base + 1] = ay;
      }
    }
    return out;
  }

  void render(SDL_Renderer* renderer, unsigned logical_w, unsigned logical_h) const {
    if (!shown() || !renderer || logical_w < 120 || logical_h < 120) return;
    SDL_BlendMode old_blend = SDL_BLENDMODE_NONE;
    SDL_GetRenderDrawBlendMode(renderer, &old_blend);
    SDL_SetRenderDrawBlendMode(renderer, SDL_BLENDMODE_BLEND);

    const float fw = static_cast<float>(logical_w);
    const float fh = static_cast<float>(logical_h);
    for (std::size_t i = 0; i < 18; ++i) {
      const auto& spec = controls_[i];
      if (!spec.visible && !edit_mode_) continue;
      bool pressed = false;
      float knob_dx = 0.0f, knob_dy = 0.0f;
      const float r = spec.radius_px * size_scale_;
      for (const auto& c : contacts_) {
        if (c.active && c.control_id == i) {
          pressed = true;
          if (i >= 16) {
            knob_dx = std::clamp((c.norm_x - spec.nx) * fw, -r, r);
            knob_dy = std::clamp((c.norm_y - spec.ny) * fh, -r, r);
          }
          break;
        }
      }
      const SDL_FRect box{spec.nx * fw - r, spec.ny * fh - r, r * 2.0f, r * 2.0f};
      if (pressed) {
        SDL_SetRenderDrawColor(renderer, 78, 186, 132, opacity_alpha_);
      } else {
        SDL_SetRenderDrawColor(renderer, 24, 34, 30, static_cast<Uint8>(opacity_alpha_ * 2 / 3));
      }
      SDL_RenderFillRect(renderer, &box);
      SDL_SetRenderDrawColor(renderer, edit_mode_ ? 245 : 170, edit_mode_ ? 195 : 215, edit_mode_ ? 70 : 195, opacity_alpha_);
      SDL_RenderRect(renderer, &box);

      if (i >= 16) {
        const float kr = r * 0.42f;
        const SDL_FRect knob{spec.nx * fw + knob_dx - kr, spec.ny * fh + knob_dy - kr, kr * 2.0f, kr * 2.0f};
        SDL_SetRenderDrawColor(renderer, 220, 240, 230, opacity_alpha_);
        SDL_RenderFillRect(renderer, &knob);
      }
      SDL_SetRenderDrawColor(renderer, 240, 248, 244, opacity_alpha_);
      SDL_RenderDebugText(renderer, spec.nx * fw - 6.0f, spec.ny * fh - 4.0f, spec.label);
    }
    if (edit_mode_) {
      SDL_SetRenderDrawColor(renderer, 250, 210, 90, 255);
      SDL_RenderDebugText(renderer, 12.0f, 12.0f, "TOUCH OVERLAY EDIT MODE (Drag controls | Shift+F9: Done)");
    }
    SDL_SetRenderDrawBlendMode(renderer, old_blend);
  }
};

} // namespace omni::presentation
