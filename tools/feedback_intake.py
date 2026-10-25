#!/usr/bin/env python3
"""
OmniPlay User Feedback Intake & Diagnostics Sanitizer
Addresses Work Item 108.

Validates user trial feedback, sanitizes diagnostic logs (scrubbing PII, paths, IPs),
and manages the feedback triage intake queue.
"""

import os
import sys
import json
import uuid
import re
import time
from pathlib import Path
from typing import Dict, Any, Optional

ALLOWED_CATEGORIES = {
    "Bug",
    "Performance",
    "Compatibility",
    "AudioVideo",
    "Input",
    "Accessibility",
    "Feature",
}

ALLOWED_PLATFORMS = {
    "windows",
    "macos",
    "linux",
    "android",
    "ios",
    "web",
}

PII_PATTERNS = [
    re.compile(r"[a-zA-Z]:\\[Users|Documents|Desktop]+(?:\\[^\\/\r\n]+)+", re.IGNORECASE),
    re.compile(r"/home/[^/\r\n]+", re.IGNORECASE),
    re.compile(r"/Users/[^/\r\n]+", re.IGNORECASE),
    re.compile(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b"),
    re.compile(r"\bgh[pousr]_[A-Za-z0-9]{36,255}\b"),
]

def sanitize_diagnostics(raw_text: str) -> str:
    sanitized = raw_text
    for pat in PII_PATTERNS:
        sanitized = pat.sub("<REDACTED_PII>", sanitized)
    return sanitized

class FeedbackSubmission:
    def __init__(
        self,
        platform_id: str,
        category: str,
        summary: str,
        description: str,
        user_consent: bool,
        reproduction_steps: str = "",
        game_title: Optional[str] = None,
        diagnostics_log: Optional[str] = None,
    ):
        self.id = str(uuid.uuid4())
        self.timestamp_ms = int(time.time() * 1000)
        self.platform = platform_id.lower()
        self.category = category
        self.summary = summary.strip()
        self.description = description.strip()
        self.user_consent = user_consent
        self.reproduction_steps = reproduction_steps.strip()
        self.game_title = game_title.strip() if game_title else None
        self.diagnostics_log = sanitize_diagnostics(diagnostics_log) if diagnostics_log else None

    def validate(self) -> bool:
        if not self.user_consent:
            return False
        if self.platform not in ALLOWED_PLATFORMS:
            return False
        if self.category not in ALLOWED_CATEGORIES:
            return False
        if not self.summary or len(self.summary) > 240:
            return False
        if not self.description or len(self.description) > 4096:
            return False
        return True

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "timestamp_ms": self.timestamp_ms,
            "platform": self.platform,
            "category": self.category,
            "summary": self.summary,
            "description": self.description,
            "user_consent": self.user_consent,
            "reproduction_steps": self.reproduction_steps,
            "game_title": self.game_title,
            "diagnostics_log": self.diagnostics_log,
        }

def save_submission(sub: FeedbackSubmission, queue_dir: Path) -> Path:
    if not sub.validate():
        raise ValueError("Invalid feedback submission")
    queue_dir.mkdir(parents=True, exist_ok=True)
    out_path = queue_dir / f"feedback-{sub.id}.json"
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(sub.to_dict(), f, indent=2)
    print(f"[+] Saved feedback submission {sub.id} to {out_path}")
    return out_path

def main():
    # Sample intake test
    sample = FeedbackSubmission(
        platform_id="windows",
        category="AudioVideo",
        summary="Audio buffer underrun observed during fast-forward",
        description="When holding fast forward in mGBA core, brief audio crackle occurs.",
        user_consent=True,
        reproduction_steps="1. Open Game. 2. Hold R2 / Space for fast forward.",
        game_title="Sample GBA ROM",
        diagnostics_log="Running on C:\\Users\\johndoe\\AppData\\Local\\OmniPlay with IP 192.168.1.50",
    )
    assert sample.validate(), "Sample feedback validation failed"
    out = save_submission(sample, Path("dist/feedback"))
    print(f"[+] Feedback intake operational. Verified PII scrubbed: {'johndoe' not in out.read_text(encoding='utf-8')}")

if __name__ == "__main__":
    main()
