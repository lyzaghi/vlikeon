"""Author the pinned Chromium extension picker/application-audio contract.

No browser, generator, compiler, platform audio API or test is executed here.
"""
import hashlib

PINS={
    'chrome/browser/extensions/api/desktop_capture/desktop_capture_api.cc':'fe672eb9a07c499fa81fa909d1de4b562e43e56e',
    'chrome/browser/extensions/api/desktop_capture/desktop_capture_base.cc':'bb268f83a895a17a93a074470ce53a7c619a9156',
    'chrome/browser/extensions/api/desktop_capture/desktop_capture_base.h':'4ac6604b073dfc869e0ea7c0111c9efacb5c0b49',
    'chrome/browser/media/webrtc/desktop_media_picker.h':'75d83fb788daefad989626423514d817ad5ad832',
    'chrome/browser/ui/views/desktop_capture/desktop_media_picker_views.cc':'fb98e1125a03a47eab126be7f385e4d755130714',
    'chrome/common/extensions/api/desktop_capture.json':'975b51070f28e8b28a2aec1d75e3c3830b1365a5',
}


def author(snapshots):
    changes={}
    for name,expected in PINS.items():
        data=(snapshots/name).read_bytes()
        if hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()!=expected:
            raise ValueError('Pinned native picker source changed: '+name)
        changes[name]=data.decode('utf-8')
    def edit(name,before,after):
        value=changes[name]
        if value.count(before)!=1:raise ValueError('Pinned native picker anchor changed: '+name)
        changes[name]=value.replace(before,after,1)

    schema='chrome/common/extensions/api/desktop_capture.json'
    edit(schema,'              "suppressLocalAudioPlaybackIntended": {','''              "omniplayStrictWindowAudio": {
                "type": "boolean",
                "optional": true,
                "description": "OmniPlay: only window sources and application audio are admitted; no system-audio fallback. Requires windowAudio=window and systemAudio=exclude."
              },
              "suppressLocalAudioPlaybackIntended": {''')
    edit(schema,'                 "canRequestAudioTrack": {','''                 "omniplayCaptureProtocol": {
                   "type": "integer",
                   "optional": true,
                   "description": "OmniPlay native picker result protocol, currently 1."
                 },
                 "audioScope": {
                   "type": "string",
                   "optional": true,
                   "description": "Browser-selected audio classification: none, application, system, tab, or unspecified. It is not independent device qualification."
                 },
                 "canRequestAudioTrack": {''')

    api='chrome/browser/extensions/api/desktop_capture/desktop_capture_api.cc'
    edit(api,'  return Execute(params->sources, exclude_system_audio,','''  std::optional<blink::mojom::WindowAudioPreference> window_audio_preference;
  if (params->options) {
    using ApiAudio = api::desktop_capture::WindowAudioPreferenceEnum;
    if (params->options->window_audio == ApiAudio::kWindow) {
      window_audio_preference = blink::mojom::WindowAudioPreference::kWindow;
    } else if (params->options->window_audio == ApiAudio::kSystem) {
      window_audio_preference = blink::mojom::WindowAudioPreference::kSystem;
    } else if (params->options->window_audio == ApiAudio::kExclude) {
      window_audio_preference = blink::mojom::WindowAudioPreference::kExclude;
    }
  }
  const bool strict_window_audio = params->options &&
      params->options->omniplay_strict_window_audio == true;
  if (strict_window_audio) {
    bool has_window = false;
    for (auto source : params->sources) {
      if (source == api::desktop_capture::DesktopCaptureSourceType::kWindow) {
        has_window = true;
      } else if (source != api::desktop_capture::DesktopCaptureSourceType::kAudio) {
        return RespondNow(Error("Application audio requires window-only capture."));
      }
    }
    if (!has_window || !exclude_system_audio ||
        window_audio_preference != blink::mojom::WindowAudioPreference::kWindow) {
      return RespondNow(Error("Application audio requires the strict window policy."));
    }
  }

  return Execute(params->sources, exclude_system_audio,''')
    edit(api,'                 target_render_frame_host, origin, target_name);',
        '                 target_render_frame_host, origin, target_name,\n                 window_audio_preference, strict_window_audio);')

    header='chrome/browser/extensions/api/desktop_capture/desktop_capture_base.h'
    edit(header,'#include <memory>','#include <memory>\n#include <optional>')
    edit(header,'      const std::u16string& target_name);','''      const std::u16string& target_name,
      std::optional<blink::mojom::WindowAudioPreference> window_audio_preference =
          std::nullopt,
      bool omniplay_strict_window_audio = false);''')
    edit(header,'  std::unique_ptr<DesktopMediaPickerController> picker_controller_;',
        '  std::unique_ptr<DesktopMediaPickerController> picker_controller_;\n  bool omniplay_strict_window_audio_ = false;')

    base='chrome/browser/extensions/api/desktop_capture/desktop_capture_base.cc'
    edit(base,'    const std::u16string& target_name) {','''    const std::u16string& target_name,
    std::optional<blink::mojom::WindowAudioPreference> window_audio_preference,
    bool omniplay_strict_window_audio) {''')
    edit(base,'  DCHECK(!picker_controller_);','  DCHECK(!picker_controller_);\n  omniplay_strict_window_audio_ = omniplay_strict_window_audio;')
    edit(base,'  picker_params.exclude_system_audio = exclude_system_audio;','''  picker_params.exclude_system_audio = exclude_system_audio;
  if (window_audio_preference) {
    picker_params.window_audio_preference = *window_audio_preference;
  }
  picker_params.omniplay_strict_window_audio = omniplay_strict_window_audio_;''')
    edit(base,'  std::string result;\n  if (source.type != DesktopMediaID::TYPE_NONE) {','''  // Validate before registering a consumable media token. A picker/backend
  // result must not widen the application's explicit audio authorization.
  if (omniplay_strict_window_audio_ &&
      (source.type != DesktopMediaID::TYPE_WINDOW ||
       (source.audio_share && source.window_audio_type !=
                                  DesktopMediaID::AudioType::kApplication))) {
    Respond(Error("The selected source cannot provide application-only audio."));
    return;
  }

  std::string result;
  if (source.type != DesktopMediaID::TYPE_NONE) {''')
    edit(base,'  options.can_request_audio_track = source.audio_share;','''  options.can_request_audio_track = source.audio_share;
  options.omniplay_capture_protocol = 1;
  options.audio_scope = "none";
  if (source.audio_share) {
    if (source.type == DesktopMediaID::TYPE_WINDOW) {
      if (source.window_audio_type == DesktopMediaID::AudioType::kApplication) {
        options.audio_scope = "application";
      } else if (source.window_audio_type == DesktopMediaID::AudioType::kSystem) {
        options.audio_scope = "system";
      } else {
        options.audio_scope = "unspecified";
      }
    } else if (source.type == DesktopMediaID::TYPE_SCREEN) {
      options.audio_scope = "system";
    } else if (source.type == DesktopMediaID::TYPE_WEB_CONTENTS) {
      options.audio_scope = "tab";
    } else {
      options.audio_scope = "unspecified";
    }
  }''')

    picker='chrome/browser/media/webrtc/desktop_media_picker.h'
    edit(picker,'    // Normally, the media-picker sets the default states for the audio',
        '    // OmniPlay extension requests can prohibit broader system-audio fallback.\n    // Default false preserves existing getDisplayMedia and other picker callers.\n    bool omniplay_strict_window_audio = false;\n    // Normally, the media-picker sets the default states for the audio')
    views='chrome/browser/ui/views/desktop_capture/desktop_media_picker_views.cc'
    edit(views,'    } else if (DesktopMediaPickerController::IsSystemAudioCaptureSupported(\n                   params.request_source)) {',
        '    } else if (!params.omniplay_strict_window_audio &&\n               DesktopMediaPickerController::IsSystemAudioCaptureSupported(\n                   params.request_source)) {')
    edit(views,'  if (params.window_audio_preference ==\n          blink::mojom::WindowAudioPreference::kSystem &&',
        '  if (!params.omniplay_strict_window_audio &&\n      params.window_audio_preference ==\n          blink::mojom::WindowAudioPreference::kSystem &&')
    return changes
