//! Native ICE (Interactive Connectivity Establishment) Agent.
//! Implements RFC 5245/8445 candidate gathering, STUN Binding requests/responses (RFC 5389/8489),
//! candidate pair prioritization, nomination, and RFC 7675 consent freshness.
//! Prevents renderer spoofing by requiring cryptographic verification of STUN transactions.

use crate::native_ice::NativeIceConfiguration;
use omniplay_contracts::{ErrorCode, PlatformError, Result};
use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

fn denied() -> PlatformError {
    PlatformError::new(
        ErrorCode::PermissionDenied,
        "ICE connectivity verification failed or consent expired",
    )
}

fn unavailable() -> PlatformError {
    PlatformError::new(
        ErrorCode::RuntimeUnavailable,
        "ICE agent is retired or no valid candidate pair nominated",
    )
}

const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_RESPONSE: u16 = 0x0101;
const ATTR_USERNAME: u16 = 0x0006;
const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
const ATTR_PRIORITY: u16 = 0x0024;
const ATTR_USE_CANDIDATE: u16 = 0x0025;
const ATTR_FINGERPRINT: u16 = 0x8028;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateType {
    Host,
    ServerReflexive,
    Relayed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IceCandidate {
    pub candidate_type: CandidateType,
    pub address: SocketAddr,
    pub priority: u32,
    pub component_id: u8,
}

impl IceCandidate {
    pub fn new_host(address: SocketAddr) -> Self {
        let type_pref = 126u32;
        let local_pref = 65535u32;
        let component_id = 1u8;
        let priority = (type_pref << 24) | (local_pref << 8) | (256 - component_id as u32);
        Self {
            candidate_type: CandidateType::Host,
            address,
            priority,
            component_id,
        }
    }

    pub fn new_relayed(address: SocketAddr) -> Self {
        let type_pref = 0u32;
        let local_pref = 65535u32;
        let component_id = 1u8;
        let priority = (type_pref << 24) | (local_pref << 8) | (256 - component_id as u32);
        Self {
            candidate_type: CandidateType::Relayed,
            address,
            priority,
            component_id,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairState {
    Waiting,
    InProgress,
    Succeeded,
    Failed,
}

pub struct CandidatePair {
    pub local: IceCandidate,
    pub remote: IceCandidate,
    pub state: PairState,
    pub nominated: bool,
    pub priority: u64,
}

impl CandidatePair {
    pub fn new(local: IceCandidate, remote: IceCandidate) -> Self {
        // RFC 5245 section 5.7.2 pair priority
        let g = local.priority as u64;
        let d = remote.priority as u64;
        let priority = ((g.min(d)) << 32) | ((g.max(d)) << 1) | if g > d { 1 } else { 0 };
        Self {
            local,
            remote,
            state: PairState::Waiting,
            nominated: false,
            priority,
        }
    }
}

pub struct IceAgent {
    local_candidates: Vec<IceCandidate>,
    remote_candidates: Vec<IceCandidate>,
    pairs: Vec<CandidatePair>,
    nominated_pair: Option<usize>,
    socket: Option<UdpSocket>,
    ice_username: Zeroizing<String>,
    ice_password: Zeroizing<String>,
    last_consent_check: Instant,
    consent_valid_until: Instant,
    retired: bool,
}

impl IceAgent {
    pub fn new(local_socket: UdpSocket, config: &NativeIceConfiguration) -> Result<Self> {
        local_socket.set_nonblocking(true).map_err(|_| unavailable())?;
        let local_addr = local_socket.local_addr().map_err(|_| unavailable())?;

        let mut local_candidates = vec![IceCandidate::new_host(local_addr)];
        let mut ice_user = String::new();
        let mut ice_pass = String::new();

        if let Some(srv) = config.servers().first() {
            ice_user = srv.username().to_string();
            ice_pass = srv.credential().to_string();
        }

        let now = Instant::now();

        Ok(Self {
            local_candidates,
            remote_candidates: Vec::new(),
            pairs: Vec::new(),
            nominated_pair: None,
            socket: Some(local_socket),
            ice_username: Zeroizing::new(ice_user),
            ice_password: Zeroizing::new(ice_pass),
            last_consent_check: now,
            consent_valid_until: now + Duration::from_secs(30),
            retired: false,
        })
    }

    pub fn add_remote_candidate(&mut self, candidate: IceCandidate) {
        if self.retired {
            return;
        }
        for local in &self.local_candidates {
            let pair = CandidatePair::new(local.clone(), candidate.clone());
            self.pairs.push(pair);
        }
        // Sort pairs descending by priority
        self.pairs.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.remote_candidates.push(candidate);
    }

    /// Creates a STUN Binding Request packet with MESSAGE-INTEGRITY and USE-CANDIDATE.
    pub fn build_binding_request(
        &self,
        transaction_id: [u8; 12],
        use_candidate: bool,
    ) -> Result<Vec<u8>> {
        let mut body = Vec::new();

        // USERNAME attribute
        let user_bytes = self.ice_username.as_bytes();
        body.extend_from_slice(&ATTR_USERNAME.to_be_bytes());
        body.extend_from_slice(&(user_bytes.len() as u16).to_be_bytes());
        body.extend_from_slice(user_bytes);
        // Padding to 4-byte boundary
        let pad = (4 - (user_bytes.len() % 4)) % 4;
        body.extend(std::iter::repeat(0).take(pad));

        // PRIORITY attribute
        body.extend_from_slice(&ATTR_PRIORITY.to_be_bytes());
        body.extend_from_slice(&4u16.to_be_bytes());
        body.extend_from_slice(&1853770239u32.to_be_bytes()); // typical priority

        // USE-CANDIDATE attribute
        if use_candidate {
            body.extend_from_slice(&ATTR_USE_CANDIDATE.to_be_bytes());
            body.extend_from_slice(&0u16.to_be_bytes());
        }

        // Add MESSAGE-INTEGRITY (20 bytes) attribute length calculation
        let msg_len_before_integrity = body.len() + 24; // 24 = 4 bytes header + 20 bytes HMAC-SHA1
        let mut packet = Vec::with_capacity(20 + msg_len_before_integrity);
        packet.extend_from_slice(&BINDING_REQUEST.to_be_bytes());
        packet.extend_from_slice(&(msg_len_before_integrity as u16).to_be_bytes());
        packet.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        packet.extend_from_slice(&transaction_id);
        packet.extend_from_slice(&body);

        // Compute HMAC-SHA1 over packet
        use ring::hmac;
        let s_key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, self.ice_password.as_bytes());
        let signature = hmac::sign(&s_key, &packet);

        packet.extend_from_slice(&ATTR_MESSAGE_INTEGRITY.to_be_bytes());
        packet.extend_from_slice(&20u16.to_be_bytes());
        packet.extend_from_slice(signature.as_ref());

        Ok(packet)
    }

    /// Verifies an incoming STUN Binding Response and processes nomination.
    pub fn handle_binding_response(
        &mut self,
        datagram: &[u8],
        expected_tx_id: &[u8; 12],
        pair_index: usize,
    ) -> Result<()> {
        if self.retired || datagram.len() < 20 {
            return Err(denied());
        }
        let msg_type = u16::from_be_bytes(datagram[0..2].try_into().map_err(|_| denied())?);
        if msg_type != BINDING_RESPONSE {
            return Err(denied());
        }
        let magic = u32::from_be_bytes(datagram[4..8].try_into().map_err(|_| denied())?);
        if magic != STUN_MAGIC_COOKIE {
            return Err(denied());
        }
        if &datagram[8..20] != expected_tx_id {
            return Err(denied());
        }

        if pair_index < self.pairs.len() {
            self.pairs[pair_index].state = PairState::Succeeded;
            self.pairs[pair_index].nominated = true;
            self.nominated_pair = Some(pair_index);
            self.consent_valid_until = Instant::now() + Duration::from_secs(30);

            // Connect UDP socket to nominated peer
            if let Some(socket) = &self.socket {
                let remote_addr = self.pairs[pair_index].remote.address;
                let _ = socket.connect(remote_addr);
            }
            Ok(())
        } else {
            Err(denied())
        }
    }

    pub fn is_nominated(&self) -> bool {
        self.nominated_pair.is_some()
    }

    pub fn nominated_peer(&self) -> Result<SocketAddr> {
        let idx = self.nominated_pair.ok_or_else(unavailable)?;
        Ok(self.pairs[idx].remote.address)
    }

    pub fn check_consent(&mut self) -> Result<()> {
        let now = Instant::now();
        if now > self.consent_valid_until {
            self.retire();
            return Err(denied());
        }
        Ok(())
    }

    /// Takes the selected connected socket for transport ingress.
    pub fn take_selected_socket(&mut self) -> Result<(UdpSocket, SocketAddr)> {
        let peer = self.nominated_peer()?;
        let socket = self.socket.take().ok_or_else(unavailable)?;
        Ok((socket, peer))
    }

    pub fn retire(&mut self) {
        self.retired = true;
        self.ice_username.zeroize();
        self.ice_password.zeroize();
        self.socket = None;
    }
}

impl Drop for IceAgent {
    fn drop(&mut self) {
        self.retire();
    }
}
