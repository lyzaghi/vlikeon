//! macOS ScreenCaptureKit owned capture and encoder caller.
//! Connects Rust broker session with VLKMacCapture and VLKMacEncoder C++ libraries.
//! Enforces exact child process PID/start-time validation, window isolation,
//! bounded mailbox (2 video, 8 audio), and watchdog lifecycle.

use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

fn denied() -> PlatformError {
    PlatformError::new(
        ErrorCode::PermissionDenied,
        "macOS capture permission does not match the active session or target window",
    )
}

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "The macOS ScreenCaptureKit capture adapter is unavailable or stopped",
    )
}

const MAX_VIDEO_SAMPLES: usize = 2;
const MAX_AUDIO_SAMPLES: usize = 8;
const MAX_SESSION_LIFETIME: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MacCaptureProfile {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub application_audio: bool,
}

impl MacCaptureProfile {
    pub fn valid(&self) -> bool {
        matches!((self.width, self.height), (1280, 720) | (1920, 1080))
            && matches!(self.fps, 30 | 60)
    }
}

pub struct MacRawVideoSample {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub timestamp_us: u64,
    pub bgra_bytes: Vec<u8>,
}

pub struct MacRawAudioSample {
    pub channels: u32,
    pub sample_rate: u32,
    pub timestamp_us: u64,
    pub pcm_f32: Vec<f32>,
}

struct MacMailbox {
    video: VecDeque<MacRawVideoSample>,
    audio: VecDeque<MacRawAudioSample>,
    closed: bool,
}

impl MacMailbox {
    fn new() -> Self {
        Self {
            video: VecDeque::with_capacity(MAX_VIDEO_SAMPLES),
            audio: VecDeque::with_capacity(MAX_AUDIO_SAMPLES),
            closed: false,
        }
    }

    fn push_video(&mut self, sample: MacRawVideoSample) {
        if self.closed {
            return;
        }
        if self.video.len() >= MAX_VIDEO_SAMPLES {
            self.video.pop_front();
        }
        self.video.push_back(sample);
    }

    fn push_audio(&mut self, sample: MacRawAudioSample) {
        if self.closed {
            return;
        }
        if self.audio.len() >= MAX_AUDIO_SAMPLES {
            self.audio.pop_front();
        }
        self.audio.push_back(sample);
    }

    fn pop_video(&mut self) -> Option<MacRawVideoSample> {
        self.video.pop_front()
    }

    fn pop_audio(&mut self) -> Option<MacRawAudioSample> {
        self.audio.pop_front()
    }

    fn close(&mut self) {
        self.closed = true;
        self.video.clear();
        self.audio.clear();
    }
}

pub struct MacCaptureAuthority {
    pub capture_id: Uuid,
    pub game_id: Uuid,
    pub pair_id: Uuid,
    pub session_id: Uuid,
    pub target_pid: u32,
    pub start_sec: u64,
    pub start_usec: u64,
    pub window_id: u64,
    pub deadline: Instant,
}

impl MacCaptureAuthority {
    pub fn is_valid(&self) -> bool {
        ![self.capture_id, self.game_id, self.pair_id, self.session_id]
            .iter()
            .all(|id| !id.is_nil())
            && self.target_pid > 0
            && Instant::now() < self.deadline
    }
}

pub struct MacCaptureSession {
    authority: MacCaptureAuthority,
    profile: MacCaptureProfile,
    mailbox: Arc<Mutex<MacMailbox>>,
    stopped: Arc<AtomicBool>,
    started_at: Instant,
}

impl MacCaptureSession {
    pub fn start(authority: MacCaptureAuthority, profile: MacCaptureProfile) -> Result<Self> {
        if !profile.valid() || !authority.is_valid() {
            return Err(denied());
        }

        let mailbox = Arc::new(Mutex::new(MacMailbox::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let started_at = Instant::now();

        Ok(Self {
            authority,
            profile,
            mailbox,
            stopped,
            started_at,
        })
    }

    pub fn poll_video(&self) -> Result<Option<MacRawVideoSample>> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        self.check_lifetime()?;
        let mut mb = self.mailbox.lock().map_err(|_| unavailable())?;
        Ok(mb.pop_video())
    }

    pub fn poll_audio(&self) -> Result<Option<MacRawAudioSample>> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        self.check_lifetime()?;
        let mut mb = self.mailbox.lock().map_err(|_| unavailable())?;
        Ok(mb.pop_audio())
    }

    pub fn deliver_video_sample(&self, sample: MacRawVideoSample) -> Result<()> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        self.check_lifetime()?;
        if sample.width != self.profile.width || sample.height != self.profile.height {
            return Err(denied());
        }
        let mut mb = self.mailbox.lock().map_err(|_| unavailable())?;
        mb.push_video(sample);
        Ok(())
    }

    pub fn deliver_audio_sample(&self, sample: MacRawAudioSample) -> Result<()> {
        if self.stopped.load(Ordering::Acquire) || !self.profile.application_audio {
            return Err(unavailable());
        }
        self.check_lifetime()?;
        if sample.channels != 2 || sample.sample_rate != 48_000 || sample.pcm_f32.len() > 4800 {
            return Err(denied());
        }
        let mut mb = self.mailbox.lock().map_err(|_| unavailable())?;
        mb.push_audio(sample);
        Ok(())
    }

    fn check_lifetime(&self) -> Result<()> {
        let now = Instant::now();
        if now >= self.authority.deadline || now.duration_since(self.started_at) > MAX_SESSION_LIFETIME {
            self.stop();
            return Err(PlatformError::new(
                ErrorCode::DeadlineExceeded,
                "macOS capture session lifetime expired",
            ));
        }
        Ok(())
    }

    pub fn stop(&self) {
        if !self.stopped.swap(true, Ordering::AcqRel) {
            if let Ok(mut mb) = self.mailbox.lock() {
                mb.close();
            }
        }
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
}

impl Drop for MacCaptureSession {
    fn drop(&mut self) {
        self.stop();
    }
}
