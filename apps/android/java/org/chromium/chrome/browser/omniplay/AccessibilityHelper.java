package org.chromium.chrome.browser.omniplay;

import android.content.Context;
import android.os.Build;
import android.view.View;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;

/**
 * Accessibility helper for TalkBack screen reader support on Android.
 * Handles spoken announcements, live region updates, and content description binding.
 */
public final class AccessibilityHelper {

    private AccessibilityHelper() {}

    /**
     * Checks if a screen reader (such as TalkBack) is active.
     */
    public static boolean isScreenReaderActive(Context context) {
        if (context == null) return false;
        AccessibilityManager am = (AccessibilityManager) context.getSystemService(Context.ACCESSIBILITY_SERVICE);
        return am != null && am.isEnabled() && am.isTouchExplorationEnabled();
    }

    /**
     * Sends an accessibility announcement to be read aloud by TalkBack.
     */
    public static void announce(View view, CharSequence text) {
        if (view == null || text == null || text.length() == 0) return;
        view.announceForAccessibility(text);
    }

    /**
     * Dispatches a TYPE_ANNOUNCEMENT event directly through the view.
     */
    public static void dispatchAnnouncementEvent(View view, CharSequence message) {
        if (view == null || message == null) return;
        AccessibilityEvent event = AccessibilityEvent.obtain(AccessibilityEvent.TYPE_ANNOUNCEMENT);
        event.getText().add(message);
        event.setEnabled(true);
        view.sendAccessibilityEventUnchecked(event);
    }

    /**
     * Sets live region mode for dynamic status updates (e.g. sync status changes).
     */
    public static void setLiveRegion(View view, int mode) {
        if (view != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.KITKAT) {
            view.setAccessibilityLiveRegion(mode);
        }
    }
}
