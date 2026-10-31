import CoreHaptics
import Foundation
import GameController
import UIKit

/// Dedicated iOS / iPadOS native controller & haptics integration for Game Hub (Item 39).
///
/// - Tracks up to 4 physical controllers via `GCController` (`GCExtendedGamepad` / `GCMicroGamepad`)
///   with foreground-only monitoring (`GCController.shouldMonitorBackgroundEvents = false`).
/// - Samples canonical 16-button masks, signed 16-bit dual sticks (Y-down convention matching SDL),
///   and 16-channel analog button/trigger pressure (`0..1000`).
/// - Drives opt-in controller rumble via `CoreHaptics` (`GCDeviceHaptics` -> `CHHapticEngine`)
///   bounded by a 250ms auto-stop watchdog.
/// - Immediately zeroes all ports and stops haptics on suspend, resign-active, backgrounding,
///   document reset, or controller disconnect.
@MainActor
final class NativeInputController: NSObject {
    static let maxPorts = 4
    private static let maxRumbleMilliseconds = 250

    private final class PortSlot {
        weak var controller: GCController?
        var rumbleEngine: CHHapticEngine?
        var rumbleStopWork: DispatchWorkItem?

        func stopRumble() {
            rumbleStopWork?.cancel()
            rumbleStopWork = nil
            rumbleEngine?.stop(completionHandler: nil)
            rumbleEngine = nil
        }
    }

    private var slots: [PortSlot] = (0..<NativeInputController.maxPorts).map { _ in PortSlot() }
    private var active = false
    private var observersInstalled = false
    private var remotePlayBridge: RemotePlayBridge?

    override init() {
        super.init()
        GCController.shouldMonitorBackgroundEvents = false
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }

    func activate() {
        GCController.shouldMonitorBackgroundEvents = false
        active = true
        installObserversIfNeeded()
        reconcileControllers()
    }

    func deactivate() {
        active = false
        for slot in slots {
            slot.stopRumble()
        }
        remotePlayBridge?.stop()
    }

    func attachRemotePlay(handler: @escaping RemotePlayBridge.ReportHandler) {
        remotePlayBridge?.stop()
        let bridge = RemotePlayBridge(reportHandler: handler)
        remotePlayBridge = bridge
        if active {
            bridge.start()
        }
    }

    func detachRemotePlay() {
        remotePlayBridge?.stop()
        remotePlayBridge = nil
    }

    private func installObserversIfNeeded() {
        guard !observersInstalled else { return }
        observersInstalled = true
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(controllerDidConnect(_:)),
            name: .GCControllerDidConnect,
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(controllerDidDisconnect(_:)),
            name: .GCControllerDidDisconnect,
            object: nil
        )
    }

    @objc private func controllerDidConnect(_ notification: Notification) {
        guard active else { return }
        reconcileControllers()
    }

    @objc private func controllerDidDisconnect(_ notification: Notification) {
        let removed = notification.object as? GCController
        for i in 0..<Self.maxPorts {
            if slots[i].controller == nil || (removed != nil && slots[i].controller === removed) {
                slots[i].stopRumble()
                slots[i].controller = nil
            }
        }
        if active {
            reconcileControllers()
        }
    }

    private func reconcileControllers() {
        let connected = GCController.controllers()
        for slot in slots {
            if let existing = slot.controller, !connected.contains(where: { $0 === existing }) {
                slot.stopRumble()
                slot.controller = nil
            }
        }
        for controller in connected {
            if slots.contains(where: { $0.controller === controller }) {
                continue
            }
            if let emptyIndex = slots.firstIndex(where: { $0.controller == nil }) {
                slots[emptyIndex].controller = controller
                if let playerIdx = GCControllerPlayerIndex(rawValue: emptyIndex) {
                    controller.playerIndex = playerIdx
                }
            }
        }
    }

    private static func permille(_ value: Float) -> Int {
        guard value.isFinite else { return 0 }
        return max(0, min(1000, Int((value * 1000.0).rounded())))
    }

    private static func axisInt16(_ value: Float, invert: Bool = false) -> Int {
        guard value.isFinite else { return 0 }
        let v = invert ? -value : value
        let deadzone: Float = 0.08
        if abs(v) <= deadzone { return 0 }
        let norm = (abs(v) - deadzone) / (1.0 - deadzone)
        let signed = v < 0 ? -min(1.0, norm) : min(1.0, norm)
        return max(-32767, min(32767, Int((signed * 32767.0).rounded())))
    }

    func pollSnapshot() -> [String: Any] {
        let foreground = active
            && UIApplication.shared.applicationState == .active
            && UIApplication.shared.isProtectedDataAvailable
        if foreground {
            reconcileControllers()
        }

        var portsArray: [[String: Any]] = []
        for index in 0..<Self.maxPorts {
            guard let controller = slots[index].controller else { continue }
            var mask = 0
            var axes = [0, 0, 0, 0]
            var triggers = [0, 0]
            var pressure = Array(repeating: 0, count: 16)

            if foreground, let pad = controller.extendedGamepad {
                let buttons: [(Int, GCControllerButtonInput?, Float)] = [
                    (0, pad.buttonA, 0.5),
                    (1, pad.buttonX, 0.5),
                    (2, pad.buttonOptions, 0.5),
                    (3, pad.buttonMenu, 0.5),
                    (4, pad.dpad.up, 0.5),
                    (5, pad.dpad.down, 0.5),
                    (6, pad.dpad.left, 0.5),
                    (7, pad.dpad.right, 0.5),
                    (8, pad.buttonB, 0.5),
                    (9, pad.buttonY, 0.5),
                    (10, pad.leftShoulder, 0.5),
                    (11, pad.rightShoulder, 0.5),
                    (12, pad.leftTrigger, 0.35),
                    (13, pad.rightTrigger, 0.35),
                    (14, pad.leftThumbstickButton, 0.5),
                    (15, pad.rightThumbstickButton, 0.5),
                ]
                for (bit, input, threshold) in buttons {
                    guard let btn = input else { continue }
                    let p = Self.permille(btn.value)
                    pressure[bit] = p
                    if btn.isPressed || btn.value >= threshold {
                        mask |= (1 << bit)
                        if pressure[bit] == 0 {
                            pressure[bit] = 1000
                        }
                    }
                }
                // GCController Y-axis is +1 up; invert to match SDL/libretro +1 down convention.
                axes[0] = Self.axisInt16(pad.leftThumbstick.xAxis.value, invert: false)
                axes[1] = Self.axisInt16(pad.leftThumbstick.yAxis.value, invert: true)
                axes[2] = Self.axisInt16(pad.rightThumbstick.xAxis.value, invert: false)
                axes[3] = Self.axisInt16(pad.rightThumbstick.yAxis.value, invert: true)
                triggers[0] = pressure[12]
                triggers[1] = pressure[13]
            } else if foreground, let micro = controller.microGamepad {
                if micro.buttonA.isPressed { mask |= (1 << 0); pressure[0] = 1000 }
                if micro.buttonX.isPressed { mask |= (1 << 1); pressure[1] = 1000 }
                if micro.buttonMenu.isPressed { mask |= (1 << 3); pressure[3] = 1000 }
                if micro.dpad.up.isPressed { mask |= (1 << 4); pressure[4] = 1000 }
                if micro.dpad.down.isPressed { mask |= (1 << 5); pressure[5] = 1000 }
                if micro.dpad.left.isPressed { mask |= (1 << 6); pressure[6] = 1000 }
                if micro.dpad.right.isPressed { mask |= (1 << 7); pressure[7] = 1000 }
            }

            let rawVendor = controller.vendorName ?? "Controller"
            let safeLabel = String(rawVendor.prefix(64))
            portsArray.append([
                "port": index,
                "connected": true,
                "label": "Port \(index + 1) · \(safeLabel)",
                "buttons_mask": mask,
                "axes": axes,
                "triggers_permille": triggers,
                "pressure_permille": pressure,
                "rumble_supported": controller.haptics != nil,
            ])
        }

        return [
            "active": foreground,
            "ports": portsArray,
        ]
    }

    func applyHaptics(port: Int, enabled: Bool, weakStrength: Int, strongStrength: Int, durationMs: Int) -> Bool {
        guard (0..<Self.maxPorts).contains(port) else { return false }
        let slot = slots[port]
        slot.stopRumble()

        guard active, enabled,
              UIApplication.shared.applicationState == .active,
              UIApplication.shared.isProtectedDataAvailable else {
            return false
        }

        let clampedWeak = max(0, min(65535, weakStrength))
        let clampedStrong = max(0, min(65535, strongStrength))
        let clampedDuration = max(0, min(Self.maxRumbleMilliseconds, durationMs))
        if (clampedWeak == 0 && clampedStrong == 0) || clampedDuration == 0 {
            return true
        }

        guard let controller = slot.controller,
              let deviceHaptics = controller.haptics,
              let engine = deviceHaptics.createEngine(withLocality: .default) else {
            return false
        }

        let intensity = max(0.05, min(1.0, Float(max(clampedWeak, clampedStrong)) / 65535.0))
        let sharpness = max(0.1, min(1.0, Float(clampedWeak) / 65535.0))
        let durationSeconds = TimeInterval(clampedDuration) / 1000.0

        do {
            try engine.start()
            let event = CHHapticEvent(
                eventType: .hapticContinuous,
                parameters: [
                    CHHapticEventParameter(parameterID: .hapticIntensity, value: intensity),
                    CHHapticEventParameter(parameterID: .hapticSharpness, value: sharpness),
                ],
                relativeTime: 0,
                duration: durationSeconds
            )
            let pattern = try CHHapticPattern(events: [event], parameters: [])
            let player = try engine.makePlayer(with: pattern)
            try player.start(atTime: CHHapticTimeImmediate)
            slot.rumbleEngine = engine

            let stopWork = DispatchWorkItem { [weak self] in
                guard let self = self, (0..<Self.maxPorts).contains(port) else { return }
                self.slots[port].stopRumble()
            }
            slot.rumbleStopWork = stopWork
            DispatchQueue.main.asyncAfter(
                deadline: .now() + .milliseconds(clampedDuration + 16),
                execute: stopWork
            )
            return true
        } catch {
            slot.stopRumble()
            return false
        }
    }
}
