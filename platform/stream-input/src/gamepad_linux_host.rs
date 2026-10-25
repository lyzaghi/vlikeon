//! Linux virtual gamepad host channel implementation using uinput.
//! Integrates Gamepads<UinputProvider> with parent broker supervisor.
//! Enforces up to 4 isolated slots, strict release fences, monotonic epochs,
//! and durable reconciliation without silent device leakage.

use crate::{
    gamepad::{Gamepads, HostObservation, Owner, State},
    gamepad_protocol::{self as wire, Action, Identity, Reply},
    gamepad_uinput::UinputProvider,
};
use omniplay_contracts::{ContentHash, ErrorCode, PlatformError, RemotePad, Result};
use std::{
    fs::File,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

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

fn read_starttime(pid: u32) -> Option<u64> {
    let path = format!("/proc/{}/stat", pid);
    let mut file = File::open(&path).ok()?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).ok()?;
    let rparen = contents.rfind(')')?;
    let fields: Vec<&str> = contents[rparen + 1..].split_whitespace().collect();
    if fields.len() < 20 {
        return None;
    }
    fields[19].parse::<u64>().ok()
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
            match read_starttime(self.identity.process_id) {
                Some(actual_start) => Ok(actual_start != self.identity.creation_time),
                None => Ok(true),
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(false)
        }
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

    fn current_identities(pads: &Gamepads<UinputProvider>) -> Result<Vec<Identity>> {
        let pairs = pads.cleanup_identities()?;
        Ok(pairs
            .into_iter()
            .map(|(slot, container)| Identity { slot, container })
            .collect())
    }

    fn current_host_observation(&self, document_alive: bool, capture_alive: bool, pair_authorized: bool) -> HostObservation {
        let alive = !self.witness.exited(self.witness.identity).unwrap_or(true);
        HostObservation {
            target: if alive { Some(self.owner.target) } else { None },
            foreground: alive,
            local_override: false,
            document_alive,
            capture_alive,
            pair_authorized,
        }
    }

    pub fn exchange(&mut self, action: Action, timeout: Duration) -> Result<(u64, Reply)> {
        if self.stopped.load(Ordering::Acquire) || timeout.is_zero() {
            return Err(unavailable());
        }
        if self.request_id >= wire::MAX_ID {
            return Err(denied());
        }
        let req_id = self.request_id;
        self.request_id += 1;

        let mut pads = self.gamepads.lock().map_err(|_| unavailable())?;
        match action {
            Action::Prepare {
                slots,
                remaining_ms,
                expires_at_ms: _,
                system_visible_consent,
                vendor: _,
                product: _,
            } => {
                let host = self.current_host_observation(true, true, true);
                let remaining = Duration::from_millis(u64::from(remaining_ms));
                pads.prepare(self.owner, &slots, remaining, &host, system_visible_consent)?;
                let identities = Self::current_identities(&pads)?;
                let reply = Reply {
                    protocol: wire::VERSION,
                    id: req_id,
                    binding: self.owner.into(),
                    status: "ok".to_string(),
                    complete: true,
                    phase: "prepared".to_string(),
                    reason: None,
                    identities: Some(identities),
                };
                Ok((req_id, reply))
            }
            Action::Heartbeat {
                document_alive,
                capture_alive,
                pair_authorized,
            } => {
                let host = self.current_host_observation(document_alive, capture_alive, pair_authorized);
                pads.tick(&host)?;
                let identities = Self::current_identities(&pads).ok();
                let phase = match pads.state() {
                    State::Preparing => "prepared",
                    State::Active => "active",
                    State::CleanupRequired => "cleanup_required",
                    State::Idle => "revoked",
                };
                let reply = Reply {
                    protocol: wire::VERSION,
                    id: req_id,
                    binding: self.owner.into(),
                    status: "ok".to_string(),
                    complete: true,
                    phase: phase.to_string(),
                    reason: None,
                    identities,
                };
                Ok((req_id, reply))
            }
            Action::Activate { identities: expected } => {
                let host = self.current_host_observation(true, true, true);
                let current = Self::current_identities(&pads)?;
                if current != expected {
                    let _ = pads.revoke(&self.owner);
                    return Err(denied());
                }
                pads.activate(&self.owner, &host)?;
                let reply = Reply {
                    protocol: wire::VERSION,
                    id: req_id,
                    binding: self.owner.into(),
                    status: "ok".to_string(),
                    complete: true,
                    phase: "active".to_string(),
                    reason: None,
                    identities: Some(current),
                };
                Ok((req_id, reply))
            }
            Action::Input { sequence, pads: remote_pads } => {
                let host = self.current_host_observation(true, true, true);
                let frames: Vec<(u8, RemotePad)> = remote_pads
                    .into_iter()
                    .map(|p| (p.slot, p.state))
                    .collect();
                pads.apply(&self.owner, sequence, &frames, &host)?;
                let identities = Self::current_identities(&pads)?;
                let reply = Reply {
                    protocol: wire::VERSION,
                    id: req_id,
                    binding: self.owner.into(),
                    status: "ok".to_string(),
                    complete: true,
                    phase: "active".to_string(),
                    reason: None,
                    identities: Some(identities),
                };
                Ok((req_id, reply))
            }
            Action::Release { sequence } => {
                let host = self.current_host_observation(true, true, true);
                pads.release(&self.owner, sequence, &host)?;
                let identities = Self::current_identities(&pads)?;
                let reply = Reply {
                    protocol: wire::VERSION,
                    id: req_id,
                    binding: self.owner.into(),
                    status: "ok".to_string(),
                    complete: true,
                    phase: "active".to_string(),
                    reason: None,
                    identities: Some(identities),
                };
                Ok((req_id, reply))
            }
            Action::Status => {
                let host = self.current_host_observation(true, true, true);
                let _ = pads.tick(&host);
                let identities = Self::current_identities(&pads).ok();
                let phase = match pads.state() {
                    State::Preparing => "prepared",
                    State::Active => "active",
                    State::CleanupRequired => "cleanup_required",
                    State::Idle => "revoked",
                };
                let reply = Reply {
                    protocol: wire::VERSION,
                    id: req_id,
                    binding: self.owner.into(),
                    status: "ok".to_string(),
                    complete: true,
                    phase: phase.to_string(),
                    reason: None,
                    identities,
                };
                Ok((req_id, reply))
            }
            Action::Stop => {
                let _ = pads.revoke(&self.owner);
                self.stopped.store(true, Ordering::Release);
                let (phase, identities) = if pads.state() == State::CleanupRequired {
                    ("cleanup_required", Self::current_identities(&pads).ok())
                } else {
                    ("revoked", Some(Vec::new()))
                };
                let reply = Reply {
                    protocol: wire::VERSION,
                    id: req_id,
                    binding: self.owner.into(),
                    status: "ok".to_string(),
                    complete: true,
                    phase: phase.to_string(),
                    reason: None,
                    identities,
                };
                Ok((req_id, reply))
            }
        }
    }

    pub fn heartbeat(&mut self) -> Result<u64> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        if self.request_id >= wire::MAX_ID {
            return Err(denied());
        }
        let req_id = self.request_id;
        self.request_id += 1;
        let host = self.current_host_observation(true, true, true);
        let mut pads = self.gamepads.lock().map_err(|_| unavailable())?;
        pads.tick(&host)?;
        Ok(req_id)
    }

    pub fn stop(&mut self) -> Result<()> {
        self.stopped.store(true, Ordering::Release);
        if let Ok(mut pads) = self.gamepads.lock() {
            pads.revoke(&self.owner)?;
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
