# Signaling, Relay and TURN Production Deployment Guide — Item 59

Status: authored template and requirements specification. Production deployment,
domain configuration, and TLS certificate generation remain pending owner infrastructure provision.
Do not enable live relay or commit secrets to repository.

## 1. Domain, DNS, and Firewall Specifications

### Required Hostnames
- `turn.omniplay.vlikeon.internal` (A/AAAA pointing to dedicated relay host)
- `signal.omniplay.vlikeon.internal` (A/AAAA pointing to HTTPS/WSS gateway)

### Inbound Firewall Ports
| Port | Protocol | Purpose | Destination |
|---|---|---|---|
| 3478 | UDP / TCP | STUN / TURN standard | Relay server |
| 5349 | UDP / TCP | TURNS (DTLS / TLS) secure | Relay server |
| 49152–65535 | UDP | TURN relayed media stream range | Relay server |
| 443 / 8443 | TCP | Authenticated Signaling (HTTPS / WSS) | Signaling server |

## 2. Secrets Management & Environment Injection

Secrets MUST NOT be committed to git or stored in public images.
Inject secrets through environment variables or secure vault (e.g., HashiCorp Vault / AWS Secrets Manager):
- `COTURN_STATIC_AUTH_SECRET`: High-entropy 256-bit hexadecimal key used by TURN REST API.
- `SIGNALING_PRIVATE_KEY`: TLS certificate private key.
- `SIGNALING_TOKEN_SECRET`: Key for validating session handshake tokens.

## 3. Operational Resource and Cost Projections

- **Bandwidth:** Video streaming at 720p 60fps (~4–6 Mbps) and 1080p 60fps (~8–12 Mbps). When direct P2P connectivity succeeds (via STUN host/reflexive candidates), zero relay bandwidth is consumed. Relayed sessions through coturn consume bidirectional relay traffic (approx. 2.7 to 5.4 GB per hour per active 1080p stream).
- **Compute:** Dual-core virtual machine with 2 GB RAM handles up to 500 concurrent STUN binding requests and 50 concurrent active TURN relay streams with negligible CPU load (< 15%).
- **Estimated Infrastructure Cost:** ~$20–$40/month for compute VM plus cloud egress bandwidth depending on P2P vs Relay ratio.
