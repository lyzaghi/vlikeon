# OmniPlay Platform Code Signing & Packaging Configuration (Item 101)

This document specifies the exact code signing, notarization, and release configurations required for each target platform.

---

## 1. Microsoft Windows (Authenticode)

- **Certificate Type**: Standard or EV Code Signing Certificate from a Microsoft-trusted Certificate Authority (DigiCert, Sectigo).
- **Hashing & Timestamping**:
  - Digest Algorithm: `SHA-256`
  - RFC 3161 Timestamp Server: `http://timestamp.digicert.com`
- **Signing Command**:
  ```powershell
  signtool.exe sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /sha1 <CertThumbprint> omniplay-launcher.exe omniplay-update.exe
  ```

---

## 2. Apple macOS (Developer ID & Notarization)

- **Certificate Type**: `Developer ID Application: Vlikeon (...)`
- **Hardened Runtime**: Enabled with `--options runtime`.
- **Entitlements**: `packaging/macos/Entitlements.plist` (strictly enforces `com.apple.security.cs.allow-jit: false`).
- **Notarization Workflow**:
  ```bash
  # 1. Sign app bundle
  codesign --deep --force --options runtime --entitlements packaging/macos/Entitlements.plist --sign "Developer ID Application: Vlikeon" dist/macos/OmniPlay.app
  # 2. Archive
  ditto -c -k --keepParent dist/macos/OmniPlay.app dist/macos/OmniPlay.zip
  # 3. Submit
  xcrun notarytool submit dist/macos/OmniPlay.zip --keychain-profile "vlikeon-notary" --wait
  # 4. Staple
  xcrun stapler staple dist/macos/OmniPlay.app
  ```

---

## 3. Google Android (Play App Signing)

- **Keystore**: PKCS12 upload keystore (`vlikeon-upload-keystore.jks`).
- **Algorithm**: RSA 4096-bit or EC P-256 with SHA-256 digest.
- **Alignment**: 16 KiB page alignment verified via `tools/verify_android_16k_alignment.py`.
- **Packaging**: Android App Bundle (`.aab`) with Play App Signing.

---

## 4. Apple iOS / iPadOS (App Store Distribution)

- **Provisioning**: App Store Distribution Provisioning Profile matching `com.vlikeon.omniplay`.
- **App Store Guidelines**: Strictly compliant with Guideline 2.5.2 (No dynamic code generation / JIT). All emulator cores run in pure interpreter mode.

---

## 5. Linux Distributions (GPG Repository Signing)

- **Key**: 4096-bit RSA or Ed25519 GPG key.
- **APT / DEB**: `Release` and `InRelease` signed via `gpg --clearsign`.
- **RPM / DNF**: RPM headers signed via `rpmsign --addsign`.
