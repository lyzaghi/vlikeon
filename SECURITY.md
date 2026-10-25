# Security policy

## Support status

OmniPlay (Vlikeon) actively maintains security updates and vulnerability patches for the following versions:

| Version | Status | Notes |
|:---|:---:|:---|
| `1.0.x` | Supported | Current stable release series; active security maintenance |
| `< 1.0.0` | Unsupported | Pre-release and development branches; upgrade to 1.0.x |

Runtime, sandbox containment, updater verification, and synchronization protocol are maintained under continuous security auditing.

## Report a vulnerability

To report a suspected vulnerability, sandbox escape, or security flaw, please use our private intake channel:

1. **Private Security Email**: Contact `security@vlikeon.vn`. Encrypt sensitive details using our published project PGP security key.
2. **GitHub Private Vulnerability Reporting**: Use the **Report a vulnerability** feature on this repository if enabled.

Do **NOT** open public issues or public pull requests for unpatched security vulnerabilities.

### Report Contents:
In your report, please provide:
- The affected version or git commit SHA.
- The specific subsystem: browser/native capabilities, containment, file grants, saves, updater, authentication, DTLS/SRTP streaming, or encrypted cloud sync.
- Proof of Concept (PoC) or reproduction steps.
- Security impact assessment.

## Response SLA & Coordinated Disclosure

- **Initial Acknowledgment**: Within **48 hours** of receipt.
- **Triage & Status Confirmation**: Within **5 business days**.
- **Remediation SLA**: Critical severity flaws remediated within **14 calendar days**; high/medium within standard release cycle.
- **Coordinated Disclosure Policy**: Standard 90-day coordinated disclosure timeline to allow users to apply signed updates prior to public details.
