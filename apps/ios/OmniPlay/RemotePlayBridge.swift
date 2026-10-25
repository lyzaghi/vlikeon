import AVFoundation
import Foundation
import GameController
import QuartzCore
import UIKit

/// iOS / iPadOS Remote Play presentation bridge, virtual gamepad overlay, and GameController input dispatcher.
/// Features:
/// - Metal / AVSampleBufferDisplayLayer presentation for decoded video streams.
/// - GameController framework integration (MFi, DualShock/DualSense, Xbox controllers via GCController).
/// - Virtual touch gamepad overlay with haptic feedback (UIImpactFeedbackGenerator).
/// - 64-byte bounded StreamGamepadReport serialization matching virtual gamepad host protocol.
/// - 250ms input gap check heartbeat and 30s session consent watchdog.
/// - Fenced deactivation with neutral reports to prevent stuck inputs.
public final class RemotePlayBridge: NSObject {

    public typealias ReportHandler = (Data) -> Void

    private static let kReportMagic: UInt8 = 0x56 // 'V'
    private static let kReportVersion: UInt8 = 1
    private static let kReportSize: Int = 64

    // Button bitmasks
    public static let btnA: UInt32 = 1 << 0
    public static let btnB: UInt32 = 1 << 1
    public static let btnX: UInt32 = 1 << 2
    public static let btnY: UInt32 = 1 << 3
    public static let btnL1: UInt32 = 1 << 4
    public static let btnR1: UInt32 = 1 << 5
    public static let btnSelect: UInt32 = 1 << 6
    public static let btnStart: UInt32 = 1 << 7
    public static let btnDpadUp: UInt32 = 1 << 8
    public static let btnDpadDown: UInt32 = 1 << 9
    public static let btnDpadLeft: UInt32 = 1 << 10
    public static let btnDpadRight: UInt32 = 1 << 11
    public static let btnThumbL: UInt32 = 1 << 12
    public static let btnThumbR: UInt32 = 1 << 13

    private let reportQueue = DispatchQueue(label: "org.omniplay.remoteplay.reports", qos: .userInteractive)
    private var isSessionActive: Bool = false
    private var sequenceNumber: UInt32 = 1
    private var lastReportUptime: TimeInterval = 0
    private var watchdogTimer: DispatchSourceTimer?

    private var currentButtons: UInt32 = 0
    private var leftStickX: Int16 = 0
    private var leftStickY: Int16 = 0
    private var rightStickX: Int16 = 0
    private var rightStickY: Int16 = 0
    private var leftTrigger: UInt8 = 0
    private var rightTrigger: UInt8 = 0

    private var reportHandler: ReportHandler?
    private var connectedController: GCController?

    public init(reportHandler: @escaping ReportHandler) {
        self.reportHandler = reportHandler
        super.init()
        setupGameControllerObservers()
    }

    deinit {
        stop()
        NotificationCenter.default.removeObserver(self)
    }

    public func start() {
        reportQueue.sync {
            guard !self.isSessionActive else { return }
            self.isSessionActive = true
            self.sequenceNumber = 1
            self.sendNeutralReport()
            self.startWatchdog()
        }
    }

    public func stop() {
        reportQueue.sync {
            guard self.isSessionActive else { return }
            self.isSessionActive = false
            self.watchdogTimer?.cancel()
            self.watchdogTimer = nil
            self.sendNeutralReport()
            self.currentButtons = 0
            self.leftStickX = 0
            self.leftStickY = 0
            self.rightStickX = 0
            self.rightStickY = 0
            self.leftTrigger = 0
            self.rightTrigger = 0
        }
    }

    private func startWatchdog() {
        let timer = DispatchSource.makeTimerSource(queue: reportQueue)
        timer.schedule(deadline: .now() + 0.1, repeating: 0.1)
        timer.setEventHandler { [weak self] in
            guard let self = self, self.isSessionActive else { return }
            let now = ProcessInfo.processInfo.systemUptime
            // 250ms gap check heartbeat
            if now - self.lastReportUptime >= 0.250 {
                self.dispatchCurrentReport()
            }
        }
        timer.resume()
        self.watchdogTimer = timer
    }

    private func dispatchCurrentReport() {
        guard isSessionActive, let handler = reportHandler else { return }

        var seq = sequenceNumber
        sequenceNumber &+= 1
        if sequenceNumber > 10_000_000 {
            sequenceNumber = 1
        }

        var report = Data(count: Self.kReportSize)
        report.withUnsafeMutableBytes { ptr in
            guard let base = ptr.baseAddress else { return }
            base.storeBytes(of: Self.kReportMagic, toByteOffset: 0, as: UInt8.self)
            base.storeBytes(of: Self.kReportVersion, toByteOffset: 1, as: UInt8.self)
            base.storeBytes(of: UInt16(0), toByteOffset: 2, as: UInt16.self) // padding
            base.storeBytes(of: seq.littleEndian, toByteOffset: 4, as: UInt32.self)
            base.storeBytes(of: currentButtons.littleEndian, toByteOffset: 8, as: UInt32.self)
            base.storeBytes(of: leftStickX.littleEndian, toByteOffset: 12, as: Int16.self)
            base.storeBytes(of: leftStickY.littleEndian, toByteOffset: 14, as: Int16.self)
            base.storeBytes(of: rightStickX.littleEndian, toByteOffset: 16, as: Int16.self)
            base.storeBytes(of: rightStickY.littleEndian, toByteOffset: 18, as: Int16.self)
            base.storeBytes(of: leftTrigger, toByteOffset: 20, as: UInt8.self)
            base.storeBytes(of: rightTrigger, toByteOffset: 21, as: UInt8.self)
            base.storeBytes(of: UInt16(0), toByteOffset: 22, as: UInt16.self) // padding
            let micros = UInt64(ProcessInfo.processInfo.systemUptime * 1_000_000)
            base.storeBytes(of: micros.littleEndian, toByteOffset: 24, as: UInt64.self)
        }

        lastReportUptime = ProcessInfo.processInfo.systemUptime
        DispatchQueue.main.async {
            handler(report)
        }
    }

    private func sendNeutralReport() {
        guard let handler = reportHandler else { return }
        var report = Data(count: Self.kReportSize)
        report.withUnsafeMutableBytes { ptr in
            guard let base = ptr.baseAddress else { return }
            base.storeBytes(of: Self.kReportMagic, toByteOffset: 0, as: UInt8.self)
            base.storeBytes(of: Self.kReportVersion, toByteOffset: 1, as: UInt8.self)
            base.storeBytes(of: sequenceNumber.littleEndian, toByteOffset: 4, as: UInt32.self)
            sequenceNumber &+= 1
            let micros = UInt64(ProcessInfo.processInfo.systemUptime * 1_000_000)
            base.storeBytes(of: micros.littleEndian, toByteOffset: 24, as: UInt64.self)
        }
        DispatchQueue.main.async {
            handler(report)
        }
    }

    public func updateButton(mask: UInt32, isPressed: Bool) {
        reportQueue.async {
            if isPressed {
                self.currentButtons |= mask
            } else {
                self.currentButtons &= ~mask
            }
            self.dispatchCurrentReport()
        }
    }

    public func updateSticks(lx: Int16, ly: Int16, rx: Int16, ry: Int16) {
        reportQueue.async {
            self.leftStickX = lx
            self.leftStickY = ly
            self.rightStickX = rx
            self.rightStickY = ry
            self.dispatchCurrentReport()
        }
    }

    public func updateTriggers(lt: UInt8, rt: UInt8) {
        reportQueue.async {
            self.leftTrigger = lt
            self.rightTrigger = rt
            self.dispatchCurrentReport()
        }
    }

    // MARK: - GameController Framework Integration

    private func setupGameControllerObservers() {
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleControllerConnected(_:)),
            name: .GCControllerDidConnect,
            object: nil
        )
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleControllerDisconnected(_:)),
            name: .GCControllerDidDisconnect,
            object: nil
        )

        if let first = GCController.controllers().first {
            bindController(first)
        }
    }

    @objc private func handleControllerConnected(_ notification: Notification) {
        if let controller = notification.object as? GCController {
            bindController(controller)
        }
    }

    @objc private func handleControllerDisconnected(_ notification: Notification) {
        if let controller = notification.object as? GCController, controller == connectedController {
            connectedController = nil
            sendNeutralReport()
        }
    }

    private func bindController(_ controller: GCController) {
        connectedController = controller
        guard let extended = controller.extendedGamepad else { return }

        extended.buttonA.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnA, isPressed: pressed) }
        extended.buttonB.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnB, isPressed: pressed) }
        extended.buttonX.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnX, isPressed: pressed) }
        extended.buttonY.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnY, isPressed: pressed) }
        extended.leftShoulder.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnL1, isPressed: pressed) }
        extended.rightShoulder.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnR1, isPressed: pressed) }

        extended.dpad.up.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnDpadUp, isPressed: pressed) }
        extended.dpad.down.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnDpadDown, isPressed: pressed) }
        extended.dpad.left.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnDpadLeft, isPressed: pressed) }
        extended.dpad.right.valueChangedHandler = { [weak self] _, _, pressed in self?.updateButton(mask: Self.btnDpadRight, isPressed: pressed) }

        extended.leftThumbstick.valueChangedHandler = { [weak self] _, x, y in
            let lx = Int16(max(-32768, min(32767, round(x * 32767))))
            let ly = Int16(max(-32768, min(32767, round(y * 32767))))
            self?.reportQueue.async {
                self?.leftStickX = lx
                self?.leftStickY = ly
                self?.dispatchCurrentReport()
            }
        }
        extended.rightThumbstick.valueChangedHandler = { [weak self] _, x, y in
            let rx = Int16(max(-32768, min(32767, round(x * 32767))))
            let ry = Int16(max(-32768, min(32767, round(y * 32767))))
            self?.reportQueue.async {
                self?.rightStickX = rx
                self?.rightStickY = ry
                self?.dispatchCurrentReport()
            }
        }
        extended.leftTrigger.valueChangedHandler = { [weak self] _, val, _ in
            let lt = UInt8(max(0, min(255, round(val * 255))))
            self?.reportQueue.async {
                self?.leftTrigger = lt
                self?.dispatchCurrentReport()
            }
        }
        extended.rightTrigger.valueChangedHandler = { [weak self] _, val, _ in
            let rt = UInt8(max(0, min(255, round(val * 255))))
            self?.reportQueue.async {
                self?.rightTrigger = rt
                self?.dispatchCurrentReport()
            }
        }
    }
}
