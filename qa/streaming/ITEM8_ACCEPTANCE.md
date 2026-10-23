# Item 8 source changes: later execution assignment

Authored only. No browser, native process, network or test is executed by this
document. Codex owns expected behavior and fixes; Antigravity may execute only
an exact frozen batch after the owner resumes verification.

1. Hold the ICE-credential reply, call connect twice and then resolve the first
   reply. The duplicate must refuse before allocating a second peer. Stop before
   credential completion; no late peer, timer, transceiver or channel may appear.
2. Delay createOffer/setLocalDescription/setRemoteDescription/createAnswer and
   candidate draining separately. Stop at each boundary. Late completions must
   not send a new description, queue candidates or attach media to the closed
   peer. Check host and client roles, including a second offer during host setup.
3. Trigger timer-driven and signal-driven recovery together. One recovery and
   one negotiation may be active. Generation 16 must be the last admitted offer;
   exceeding the bound requires a new session, not a relaxed remote check.
4. Invoke Stop repeatedly and reentrantly through capture-ended callbacks.
   Every caller must receive the same completion promise. Capture and native
   input retire locally before optional remote notification. Hold notification
   and receiver close independently to observe their bounded app waits.
5. Deliver late track/channel/connection events after Stop. New tracks and
   channels must retire; no closed UI may return to connected or enabled input.
   Observe actual capture indicators and OS input independently of JS events.
6. Register the same selected PC executable through concurrent stores. Keep one
   library identity and its matching private launch metadata/audit. Change bytes,
   byte length, working-directory binding or library system before launch;
   require renewed local selection without starting an unapproved process.
7. Fail the audit before launch, then separately after successful process spawn.
   The first failure must prevent spawn. A retained real child in the latter
   case must still produce a successful launch result, avoiding duplicate retry.
8. Fail or make exit-accounting commit uncertain. Keep the original exit time and
   observation. Retry must consult the session receipt and credit exactly once,
   including a commit that succeeded durably but returned an error to the caller.
   Kill/reopen and launcher-child ownership remain separate required scenarios.
9. Reenter host accept from its capture event using the same source and a second
   source; only the first remains owned. Stop in that callback and during initial
   sender.setParameters; no successful acceptance or late quality event follows.
   Delay adaptive bitrate setParameters, stop and resolve it; no new policy work
   may be admitted. Browser work admitted before Stop may still finish.
10. Delay the signaling receiver lease. A duplicate start must refuse; Close
    before its completion must release only after that admitted lease settles.
    Repeated/reentrant Close shares one promise. Delay pair lookup, packet
    decryption and store.accept separately; after Close, no signal or connected
    event may publish. Hold sends to observe the 64-task queue bound and confirm
    the next attempt refuses without adding retained work.

Mocks can exercise ordering but cannot establish device, codec, foreground,
SendInput, TURN, mobile-lifecycle or actual process ownership. Preserve all
original acceptance scope in the streaming/native-input documents and item 8.

Owned native Windows follow-up, authored and not dispatched:

11. Prepare a broker-owned game for the pending paired session. Confirm PID,
    creation FILETIME and HWND never appear in Game Hub responses or remote
    packets. Attempt a second document, expired/closed page and abandoned late
    prepare result; retain no usable binding or picker after retirement.
12. Select a different game's window, screen/tab, exited process and reused PID.
    Each must fail before media token registration. Test exact process creation
    matching and actual NativeDesktopMediaList HWND representation on x64/ARM64.
13. Request application audio; reject system/unspecified fallback. Uncheck audio
    for video-only selection. Observe the actual source/process audio routing,
    not only returned classification, across supported OS/backend versions.
14. Try native input with generic capture, a forged/windowless capture UUID,
    another game/pair/session and expired capture. Require refusal. With a real
    owned binding, only the exact captured HWND may activate input; another
    window of the same process must not substitute for it.
15. Cancel the picker, close the page, stop during getUserMedia/configuration,
    expire the certificate and destroy/recreate the source window. Verify real
    track/helper retirement, held-input release and all documented OS race limits.
