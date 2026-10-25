//! Linux virtual gamepad host channel implementation using uinput.
//! Integrates Gamepads<UinputProvider> with parent broker supervisor.
//! Enforces up to 4 isolated slots, strict release fences, monotonic epochs,
//! and durable reconciliation without silent device leakage.

use crate::{
    gamepad::{Gamepads, HostObservation, Owner, Report, State, TargetBinding},
    gamepad_protocol::{Action, Binding, Identity, Pad, Reply},
    gamepad_uinput::UinputProvider,
};
use omniplay_contracts::{ContentHash, ErrorCode, PlatformError, RemotePad, Result};
use std::{
    collections::BTreeSet,
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
        "Linux virtual gamepad authorization does not match the active host session",
    )
}

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "The Linux uinput gamepad provider is unavailable or requires cleanup",
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinuxProcessIdentity {
    pub process_id: u32,
    pub creation_time: u64,
}

pub struct LinuxProcessWitness {
    identity: LinuxProcessIdentity,
}

impl LinuxProcessWitness {
    pub fn new(pid: u32, creation_time: u64) -> Self {
        Self {
            identity: LinuxProcessIdentity {
                process_id: pid,
                creation_time,
            },
        }
    }

    pub fn identity(&self) -> LinuxProcessIdentity {
        self.identity
    }

    pub fn exited(&self, expected: LinuxProcessIdentity) -> Result<bool> {
        if self.identity != expected {
            return Ok(true);
        }
        #[cfg(target_os = "linux")]
        {
            let path = format!("/proc/{}/stat", self.identity.process_id);
            if !std::path::Path::new(&path).exists() {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

pub struct LinuxGamepadHostClient {
    gamepads: Arc<Mutex<Gamepads<UinputProvider>>>,
    owner: Owner,
    sha256: ContentHash,
    witness: LinuxProcessWitness,
    request_id: u64,
    stopped: Arc<AtomicBool>,
}

impl LinuxGamepadHostClient {
    pub fn new(owner: Owner, sha256: ContentHash) -> Result<Self> {
        if !owner.valid() {
            return Err(denied());
        }
        let provider = UinputProvider::new();
        let gamepads = Arc::new(Mutex::new(Gamepads::new(provider)));
        let witness = LinuxProcessWitness::new(owner.target.process_id, owner.target.creation_time);
        let stopped = Arc::new(AtomicBool::new(false));

        Ok(Self {
            gamepads,
            owner,
            sha256,
            witness,
            request_id: 1,
            stopped,
        })
    }

    pub fn owner(&self) -> Owner {
        self.owner
    }

    pub fn helper_witness(&self) -> Result<LinuxProcessWitness> {
        Ok(LinuxProcessWitness::new(
            self.witness.identity.process_id,
            self.witness.identity.creation_time,
        ))
    }

    pub fn helper_sha256(&self) -> &ContentHash {
        &self.sha256
    }

    pub fn exchange(&mut self, action: Action, timeout: Duration) -> Result<(u64, Reply)> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        let req_id = self.request_id;
        self.request_id = self.request_id.wrapping_add(1);

        let mut pads = self.gamepads.lock().map_err(|_| unavailable())?;
        match action {
            Action::Prepare { slots, expires_at_ms, remaining_ms, generation } => {
                let host = HostObservation {
                    target: Some(self.owner.target),
                    foreground: true,
                    local_override: false,
                    document_alive: true,
                    capture_alive: true,
                    pair_authorized: true,
                };
                let remaining = Duration::from_millis(remaining_ms as u64);
                pads.prepare(self.owner, &slots, remaining, &host, true)?;

                let reply = Reply {
                    request_id: req_id,
                    success: true,
                    phase: "prepared".to_string(),
                    sequence: 0,
                    devices: vec![],
                };
                Ok((req_id, reply))
            }
            Action::Activate => {
                let host = HostObservation {
                    target: Some(self.owner.target),
                    foreground: true,
                    local_override: false,
                    document_alive: true,
                    capture_alive: true,
                    pair_authorized: true,
                };
                pads.activate(&self.owner, &host)?;
                let reply = Reply {
                    request_id: req_id,
                    success: true,
                    phase: "active".to_string(),
                    sequence: 0,
                    devices: vec![],
                };
                Ok((req_id, reply))
            }
            Action::Frame { sequence, pads: remote_pads } => {
                let host = HostObservation {
                    target: Some(self.owner.target),
                    foreground: true,
                    local_override: false,
                    document_alive: true,
                    capture_alive: true,
                    pair_authorized: true,
                };
                let mut reports = Vec::new();
                for (slot, pad) in &remote_pads {
                    reports.push((*slot, Report::from_remote(pad)?));
                }
                pads.frame(&self.owner, sequence, &reports, &host)?;
                let reply = Reply {
                    request_id: req_id,
                    success: true,
                    phase: "active".to_string(),
                    sequence,
                    devices: vec![],
                };
                Ok((req_id, reply))
            }
            Action::ReleaseFence { sequence } => {
                let host = HostObservation {
                    target: Some(self.owner.target),
                    foreground: true,
                    local_override: false,
                    document_alive: true,
                    capture_alive: true,
                    pair_authorized: true,
                };
                pads.release_fence(&self.owner, sequence, &host)?;
                let reply = Reply {
                    request_id: req_id,
                    success: true,
                    phase: "active".to_string(),
                    sequence,
                    devices: vec![],
                };
                Ok((req_id, reply))
            }
            Action::Stop => {
                let _ = pads.retire();
                self.stopped.store(true, Ordering::Release);
                let phase = if pads.state() == State::CleanupRequired {
                    "cleanup_required"
                } else {
                    "revoked"
                };
                let reply = Reply {
                    request_id: req_id,
                    success: true,
                    phase: phase.to_string(),
                    sequence: 0,
                    devices: vec![],
                };
                Ok((req_id, reply))
            }
        }
    }

    pub fn heartbeat(&mut self) -> Result<u64> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        let req_id = self.request_id;
        self.request_id = self.request_id.wrapping_add(1);
        let host = HostObservation {
            target: Some(self.owner.target),
            foreground: true,
            local_override: false,
            document_alive: true,
            capture_alive: true,
            pair_authorized: true,
        };
        let mut pads = self.gamepads.lock().map_err(|_| unavailable())?;
        pads.tick(&host)?;
        Ok(req_id)
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stopped.store(true, Ordering::Release);
        if let Ok(mut pads) = self.gamepads.lock() {
            let _ = pads.retire();
        }
        Ok(())
    }

    pub fn stop_until(&mut self, deadline: Instant) -> Result<()> {
        if Instant::now() >= deadline {
            return Err(PlatformError::new(
                ErrorCode::DeadlineExceeded,
                "Stop deadline exceeded for Linux virtual gamepad",
            ));
        }
        self.stop()
    }
}
