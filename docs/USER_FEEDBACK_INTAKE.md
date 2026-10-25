# OmniPlay User Feedback Intake & Issue Triage System (Item 108)

This document specifies the user trial feedback collection architecture, PII sanitization standards, triage categories, and issue resolution workflows.

---

## 1. Privacy & Consent First

User feedback is collected strictly on an opt-in basis:
1. **Explicit Consent**: `user_consent: true` is mandatory; submissions without user consent are rejected.
2. **Automated PII Scrubbing**: Diagnostic logs attached to feedback are automatically filtered through `tools/feedback_intake.py` to remove:
   - Filesystem paths containing user profile names (`C:\Users\<user>`, `/home/<user>`, `/Users/<user>`).
   - IPv4 and IPv6 network addresses.
   - Access tokens, authorization headers, and private key markers.
3. **No Automated Telemetry Upload**: Diagnostics are never uploaded in the background; they are submitted only when the user clicks "Submit Feedback".

---

## 2. Feedback Categories & Triage SLAs

| Category | Description | Target Triage SLA |
|:---|:---|:---:|
| **Bug** | Functional regression or unexpected program termination | 2 business days |
| **Performance** | Frame drops, stutter, excessive memory, or battery drain | 3 business days |
| **Compatibility** | Specific ROM title graphic/audio anomaly | 5 business days |
| **AudioVideo** | Presentation timing, VSync tear, DAC underrun | 3 business days |
| **Input** | Controller mapping, deadzone, touch overlay responsiveness | 3 business days |
| **Accessibility** | Screen reader, contrast, localization text truncation | 2 business days |
| **Feature** | Enhancement proposal or new platform request | Backlog review |

---

## 3. Tooling and Intake Verification

Test the feedback intake validator with:
```bash
py tools/feedback_intake.py
```
Outputs scrubbed, structured JSON submissions to `dist/feedback/`.
