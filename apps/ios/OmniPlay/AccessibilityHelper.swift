import Foundation
import UIKit

/// VoiceOver accessibility helper for iOS / iPadOS.
/// Provides spoken announcements, accessibility trait management, and element layout notifications.
public final class AccessibilityHelper {

    private init() {}

    /// Checks if VoiceOver is currently running.
    public static var isVoiceOverRunning: Bool {
        return UIAccessibility.isVoiceOverRunning
    }

    /// Announces a localized message through VoiceOver.
    public static func postAnnouncement(_ message: String) {
        guard !message.isEmpty else { return }
        UIAccessibility.post(notification: .announcement, argument: message)
    }

    /// Notifies VoiceOver that a layout change occurred and moves focus to the specified element.
    public static func postLayoutChanged(focusElement: Any? = nil) {
        UIAccessibility.post(notification: .layoutChanged, argument: focusElement)
    }

    /// Notifies VoiceOver that a screen change occurred.
    public static func postScreenChanged(focusElement: Any? = nil) {
        UIAccessibility.post(notification: .screenChanged, argument: focusElement)
    }

    /// Configures standard accessibility properties on an interactive UIKit view.
    public static func configureButton(
        _ view: UIView,
        labelKey: String,
        hintKey: String? = nil,
        traits: UIAccessibilityTraits = .button
    ) {
        view.isAccessibilityElement = true
        view.accessibilityLabel = NSLocalizedString(labelKey, comment: "")
        if let hintKey = hintKey {
            view.accessibilityHint = NSLocalizedString(hintKey, comment: "")
        }
        view.accessibilityTraits = traits
    }
}
