package org.chromium.chrome.browser.omniplay;

import android.content.Context;
import android.hardware.input.InputManager;
import android.os.Build;
import android.os.CombinedVibration;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.os.VibratorManager;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewTreeObserver;

import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Dedicated Android native input integration for Game Hub tabs (Item 39).
 *
 * Responsibilities:
 * - Tracks up to 4 physical controllers via InputManager.InputDeviceListener with deterministic
 *   port assignment and hotplug cleanup.
 * - Captures KeyEvent and MotionEvent (sticks, triggers, hat D-pad, analog pressure) directly on
 *   the owned Game Hub tab View without exposing raw hardware identifiers to untrusted web pages.
 * - Drives opt-in controller haptics (VibratorManager / Vibrator) with a 250ms auto-stop watchdog.
 * - Immediately zeroes all ports, releases stuck inputs, and cancels rumble on focus loss,
 *   tab hide, activity pause, view detach, or controller removal.
 */
public final class NativeInputController implements InputManager.InputDeviceListener,
        View.OnKeyListener, View.OnGenericMotionListener,
        ViewTreeObserver.OnWindowFocusChangeListener, View.OnAttachStateChangeListener {

    public static final int MAX_PORTS = 4;
    private static final int MAX_RUMBLE_MS = 250;

    // Canonical libretro joypad bit positions (0..15)
    public static final int RETRO_B = 1 << 0;
    public static final int RETRO_Y = 1 << 1;
    public static final int RETRO_SELECT = 1 << 2;
    public static final int RETRO_START = 1 << 3;
    public static final int RETRO_UP = 1 << 4;
    public static final int RETRO_DOWN = 1 << 5;
    public static final int RETRO_LEFT = 1 << 6;
    public static final int RETRO_RIGHT = 1 << 7;
    public static final int RETRO_A = 1 << 8;
    public static final int RETRO_X = 1 << 9;
    public static final int RETRO_L1 = 1 << 10;
    public static final int RETRO_R1 = 1 << 11;
    public static final int RETRO_L2 = 1 << 12;
    public static final int RETRO_R2 = 1 << 13;
    public static final int RETRO_L3 = 1 << 14;
    public static final int RETRO_R3 = 1 << 15;

    private static final class PortState {
        int deviceId = -1;
        String name = "";
        int keyMask = 0;
        int hatMask = 0;
        int triggerMask = 0;
        short lx = 0;
        short ly = 0;
        short rx = 0;
        short ry = 0;
        int leftTriggerPermille = 0;
        int rightTriggerPermille = 0;
        final int[] buttonPressure = new int[16];
        boolean rumbleSupported = false;
        Runnable rumbleStop = null;

        void resetInputs() {
            keyMask = 0;
            hatMask = 0;
            triggerMask = 0;
            lx = 0;
            ly = 0;
            rx = 0;
            ry = 0;
            leftTriggerPermille = 0;
            rightTriggerPermille = 0;
            for (int i = 0; i < buttonPressure.length; i++) {
                buttonPressure[i] = 0;
            }
        }

        void clearDevice() {
            resetInputs();
            deviceId = -1;
            name = "";
            rumbleSupported = false;
        }

        int combinedMask() {
            return (keyMask | hatMask | triggerMask) & 0xFFFF;
        }
    }

    private final Context mContext;
    private final InputManager mInputManager;
    private final Handler mMain = new Handler(Looper.getMainLooper());
    private final PortState[] mPorts = new PortState[MAX_PORTS];

    private static final long GAMEPLAY_POLL_LEASE_MS = 1500L;

    private View mAttachedView;
    private boolean mActive = false;
    private boolean mGameplayConsumerActive = false;
    private long mLastPollUptimeMs = 0L;
    private boolean mListenerRegistered = false;
    private RemotePlayClient mRemotePlayClient;

    public NativeInputController(Context context) {
        mContext = context.getApplicationContext();
        mInputManager = (InputManager) mContext.getSystemService(Context.INPUT_SERVICE);
        for (int i = 0; i < MAX_PORTS; i++) {
            mPorts[i] = new PortState();
        }
    }

    public synchronized void activate(View tabView) {
        mActive = true;
        if (mInputManager != null && !mListenerRegistered) {
            mInputManager.registerInputDeviceListener(this, mMain);
            mListenerRegistered = true;
        }
        scanConnectedDevicesLocked();
        bindViewLocked(tabView);
    }

    public synchronized void deactivate() {
        mActive = false;
        mGameplayConsumerActive = false;
        mLastPollUptimeMs = 0L;
        unbindViewLocked();
        if (mInputManager != null && mListenerRegistered) {
            mInputManager.unregisterInputDeviceListener(this);
            mListenerRegistered = false;
        }
        for (int i = 0; i < MAX_PORTS; i++) {
            stopPortRumbleLocked(i);
            mPorts[i].resetInputs();
        }
        if (mRemotePlayClient != null) {
            mRemotePlayClient.stop();
        }
    }

    public synchronized boolean setGameplayMode(boolean enabled) {
        if (enabled && mActive) {
            mGameplayConsumerActive = true;
            mLastPollUptimeMs = SystemClock.uptimeMillis();
            return true;
        }
        mGameplayConsumerActive = false;
        mLastPollUptimeMs = 0L;
        for (int i = 0; i < MAX_PORTS; i++) {
            stopPortRumbleLocked(i);
            mPorts[i].resetInputs();
        }
        return false;
    }

    private boolean shouldConsumeControllerEventsLocked() {
        if (!mActive || mAttachedView == null || !mAttachedView.hasWindowFocus()) {
            return false;
        }
        if (mRemotePlayClient != null) {
            return true;
        }
        if (!mGameplayConsumerActive) {
            return false;
        }
        return (SystemClock.uptimeMillis() - mLastPollUptimeMs) <= GAMEPLAY_POLL_LEASE_MS;
    }

    public synchronized void attachRemotePlay(RemotePlayClient.ReportSink sink) {
        if (mRemotePlayClient != null) {
            mRemotePlayClient.stop();
        }
        mRemotePlayClient = sink == null ? null : new RemotePlayClient(mContext, sink);
        if (mRemotePlayClient != null && mActive) {
            mRemotePlayClient.start();
        }
    }

    public synchronized void detachRemotePlay() {
        if (mRemotePlayClient != null) {
            mRemotePlayClient.stop();
            mRemotePlayClient = null;
        }
    }

    private void bindViewLocked(View view) {
        if (mAttachedView == view) return;
        unbindViewLocked();
        if (view == null) return;
        mAttachedView = view;
        view.setOnKeyListener(this);
        view.setOnGenericMotionListener(this);
        view.addOnAttachStateChangeListener(this);
        ViewTreeObserver observer = view.getViewTreeObserver();
        if (observer != null && observer.isAlive()) {
            observer.addOnWindowFocusChangeListener(this);
        }
    }

    private void unbindViewLocked() {
        if (mAttachedView == null) return;
        View view = mAttachedView;
        mAttachedView = null;
        view.setOnKeyListener(null);
        view.setOnGenericMotionListener(null);
        view.removeOnAttachStateChangeListener(this);
        ViewTreeObserver observer = view.getViewTreeObserver();
        if (observer != null && observer.isAlive()) {
            observer.removeOnWindowFocusChangeListener(this);
        }
    }

    private static boolean isSupportedController(InputDevice device) {
        if (device == null || device.isVirtual()) return false;
        int sources = device.getSources();
        return (sources & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                || (sources & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK;
    }

    private static boolean supportsRumble(InputDevice device) {
        if (device == null) return false;
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            VibratorManager vm = device.getVibratorManager();
            if (vm != null && vm.getVibratorIds().length > 0) return true;
        }
        @SuppressWarnings("deprecation")
        Vibrator vib = device.getVibrator();
        return vib != null && vib.hasVibrator();
    }

    private void scanConnectedDevicesLocked() {
        if (mInputManager == null) return;
        int[] ids = mInputManager.getInputDeviceIds();
        for (int id : ids) {
            ensurePortForDeviceLocked(id);
        }
    }

    private int ensurePortForDeviceLocked(int deviceId) {
        for (int i = 0; i < MAX_PORTS; i++) {
            if (mPorts[i].deviceId == deviceId) return i;
        }
        if (mInputManager == null) return -1;
        InputDevice device = mInputManager.getInputDevice(deviceId);
        if (!isSupportedController(device)) return -1;
        for (int i = 0; i < MAX_PORTS; i++) {
            if (mPorts[i].deviceId < 0) {
                mPorts[i].deviceId = deviceId;
                String rawName = device.getName();
                mPorts[i].name = rawName == null ? "Controller" : rawName.substring(0, Math.min(64, rawName.length()));
                mPorts[i].rumbleSupported = supportsRumble(device);
                mPorts[i].resetInputs();
                return i;
            }
        }
        return -1;
    }

    @Override
    public synchronized void onInputDeviceAdded(int deviceId) {
        if (!mActive) return;
        ensurePortForDeviceLocked(deviceId);
    }

    @Override
    public synchronized void onInputDeviceChanged(int deviceId) {
        if (!mActive || mInputManager == null) return;
        for (int i = 0; i < MAX_PORTS; i++) {
            if (mPorts[i].deviceId == deviceId) {
                InputDevice dev = mInputManager.getInputDevice(deviceId);
                if (!isSupportedController(dev)) {
                    stopPortRumbleLocked(i);
                    mPorts[i].clearDevice();
                } else {
                    mPorts[i].rumbleSupported = supportsRumble(dev);
                }
                return;
            }
        }
        ensurePortForDeviceLocked(deviceId);
    }

    @Override
    public synchronized void onInputDeviceRemoved(int deviceId) {
        for (int i = 0; i < MAX_PORTS; i++) {
            if (mPorts[i].deviceId == deviceId) {
                stopPortRumbleLocked(i);
                mPorts[i].clearDevice();
            }
        }
    }

    @Override
    public synchronized void onWindowFocusChanged(boolean hasFocus) {
        if (!hasFocus) {
            for (int i = 0; i < MAX_PORTS; i++) {
                stopPortRumbleLocked(i);
                mPorts[i].resetInputs();
            }
        }
    }

    @Override
    public void onViewAttachedToWindow(View v) {}

    @Override
    public synchronized void onViewDetachedFromWindow(View v) {
        if (v == mAttachedView) {
            for (int i = 0; i < MAX_PORTS; i++) {
                stopPortRumbleLocked(i);
                mPorts[i].resetInputs();
            }
        }
    }

    private static int keyCodeToRetroBit(int keyCode) {
        switch (keyCode) {
            case KeyEvent.KEYCODE_BUTTON_A: return 0; // South -> RETRO_B
            case KeyEvent.KEYCODE_BUTTON_X: return 1; // West -> RETRO_Y
            case KeyEvent.KEYCODE_BUTTON_SELECT:
            case KeyEvent.KEYCODE_BACK: return 2;
            case KeyEvent.KEYCODE_BUTTON_START: return 3;
            case KeyEvent.KEYCODE_DPAD_UP: return 4;
            case KeyEvent.KEYCODE_DPAD_DOWN: return 5;
            case KeyEvent.KEYCODE_DPAD_LEFT: return 6;
            case KeyEvent.KEYCODE_DPAD_RIGHT: return 7;
            case KeyEvent.KEYCODE_BUTTON_B: return 8; // East -> RETRO_A
            case KeyEvent.KEYCODE_BUTTON_Y: return 9; // North -> RETRO_X
            case KeyEvent.KEYCODE_BUTTON_L1: return 10;
            case KeyEvent.KEYCODE_BUTTON_R1: return 11;
            case KeyEvent.KEYCODE_BUTTON_L2: return 12;
            case KeyEvent.KEYCODE_BUTTON_R2: return 13;
            case KeyEvent.KEYCODE_BUTTON_THUMBL: return 14;
            case KeyEvent.KEYCODE_BUTTON_THUMBR: return 15;
            default: return -1;
        }
    }

    @Override
    public synchronized boolean onKey(View v, int keyCode, KeyEvent event) {
        if (event == null || !shouldConsumeControllerEventsLocked()) return false;
        int sources = event.getSource();
        boolean isPad = (sources & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD
                || (sources & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK
                || (sources & InputDevice.SOURCE_DPAD) == InputDevice.SOURCE_DPAD;
        if (!isPad) return false;

        int bit = keyCodeToRetroBit(keyCode);
        if (bit < 0) return false;
        // Allow ordinary browser Back navigation unless coming from an actual gamepad
        if (keyCode == KeyEvent.KEYCODE_BACK
                && (sources & InputDevice.SOURCE_GAMEPAD) != InputDevice.SOURCE_GAMEPAD) {
            return false;
        }

        int port = ensurePortForDeviceLocked(event.getDeviceId());
        if (port < 0) return false;

        int action = event.getAction();
        if (action == KeyEvent.ACTION_DOWN) {
            mPorts[port].keyMask |= (1 << bit);
            mPorts[port].buttonPressure[bit] = 1000;
        } else if (action == KeyEvent.ACTION_UP) {
            mPorts[port].keyMask &= ~(1 << bit);
            if (bit != 12 && bit != 13) {
                mPorts[port].buttonPressure[bit] = 0;
            }
        } else {
            return false;
        }

        if (mRemotePlayClient != null && port == 0) {
            mRemotePlayClient.onKeyEvent(event);
        }
        return true;
    }

    private static float filteredAxis(MotionEvent event, InputDevice device, int axis) {
        float value = event.getAxisValue(axis);
        if (!Float.isFinite(value)) return 0.0f;
        float flat = 0.08f;
        if (device != null) {
            InputDevice.MotionRange range = device.getMotionRange(axis, event.getSource());
            if (range != null && range.getFlat() > 0.0f && range.getFlat() < 0.45f) {
                flat = range.getFlat();
            }
        }
        if (Math.abs(value) <= flat) return 0.0f;
        float norm = (Math.abs(value) - flat) / (1.0f - flat);
        norm = Math.max(0.0f, Math.min(1.0f, norm));
        return value < 0.0f ? -norm : norm;
    }

    private static short toShortAxis(float normalized) {
        int scaled = Math.round(normalized * 32767.0f);
        return (short) Math.max(-32767, Math.min(32767, scaled));
    }

    @Override
    public synchronized boolean onGenericMotion(View v, MotionEvent event) {
        if (event == null || !shouldConsumeControllerEventsLocked()) return false;
        if ((event.getSource() & InputDevice.SOURCE_JOYSTICK) != InputDevice.SOURCE_JOYSTICK
                && (event.getSource() & InputDevice.SOURCE_GAMEPAD) != InputDevice.SOURCE_GAMEPAD) {
            return false;
        }
        if (event.getActionMasked() != MotionEvent.ACTION_MOVE) return false;

        int port = ensurePortForDeviceLocked(event.getDeviceId());
        if (port < 0) return false;

        InputDevice device = mInputManager == null ? null : mInputManager.getInputDevice(event.getDeviceId());
        PortState state = mPorts[port];

        state.lx = toShortAxis(filteredAxis(event, device, MotionEvent.AXIS_X));
        state.ly = toShortAxis(filteredAxis(event, device, MotionEvent.AXIS_Y));
        state.rx = toShortAxis(filteredAxis(event, device, MotionEvent.AXIS_Z));
        state.ry = toShortAxis(filteredAxis(event, device, MotionEvent.AXIS_RZ));

        float hatX = event.getAxisValue(MotionEvent.AXIS_HAT_X);
        float hatY = event.getAxisValue(MotionEvent.AXIS_HAT_Y);
        int hat = 0;
        if (hatX <= -0.5f) hat |= RETRO_LEFT;
        else if (hatX >= 0.5f) hat |= RETRO_RIGHT;
        if (hatY <= -0.5f) hat |= RETRO_UP;
        else if (hatY >= 0.5f) hat |= RETRO_DOWN;
        state.hatMask = hat;

        float lTrig = Math.max(event.getAxisValue(MotionEvent.AXIS_LTRIGGER),
                event.getAxisValue(MotionEvent.AXIS_BRAKE));
        float rTrig = Math.max(event.getAxisValue(MotionEvent.AXIS_RTRIGGER),
                event.getAxisValue(MotionEvent.AXIS_GAS));
        int ltPermille = Float.isFinite(lTrig) ? Math.max(0, Math.min(1000, Math.round(lTrig * 1000.0f))) : 0;
        int rtPermille = Float.isFinite(rTrig) ? Math.max(0, Math.min(1000, Math.round(rTrig * 1000.0f))) : 0;
        state.leftTriggerPermille = ltPermille;
        state.rightTriggerPermille = rtPermille;
        state.buttonPressure[12] = ltPermille;
        state.buttonPressure[13] = rtPermille;

        int trigMask = 0;
        if (ltPermille >= 350) trigMask |= RETRO_L2;
        if (rtPermille >= 350) trigMask |= RETRO_R2;
        state.triggerMask = trigMask;

        if (mRemotePlayClient != null && port == 0) {
            mRemotePlayClient.onGenericMotionEvent(event);
        }
        return true;
    }

    private void stopPortRumbleLocked(int port) {
        if (port < 0 || port >= MAX_PORTS) return;
        PortState state = mPorts[port];
        if (state.rumbleStop != null) {
            mMain.removeCallbacks(state.rumbleStop);
            state.rumbleStop = null;
        }
        if (state.deviceId < 0 || mInputManager == null) return;
        InputDevice device = mInputManager.getInputDevice(state.deviceId);
        if (device == null) return;
        try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                VibratorManager vm = device.getVibratorManager();
                if (vm != null) {
                    vm.cancel();
                    return;
                }
            }
            @SuppressWarnings("deprecation")
            Vibrator vib = device.getVibrator();
            if (vib != null) vib.cancel();
        } catch (Exception ignored) {
        }
    }

    public synchronized boolean applyHaptics(
            int port, boolean enabled, int weakStrength, int strongStrength, int durationMs) {
        if (port < 0 || port >= MAX_PORTS) return false;
        stopPortRumbleLocked(port);
        if (!mActive || !enabled) return false;
        if (mAttachedView == null || !mAttachedView.hasWindowFocus()) return false;

        int clampedWeak = Math.max(0, Math.min(65535, weakStrength));
        int clampedStrong = Math.max(0, Math.min(65535, strongStrength));
        int clampedDuration = Math.max(0, Math.min(MAX_RUMBLE_MS, durationMs));
        if ((clampedWeak == 0 && clampedStrong == 0) || clampedDuration == 0) {
            return true;
        }

        PortState state = mPorts[port];
        if (state.deviceId < 0 || !state.rumbleSupported || mInputManager == null) return false;
        InputDevice device = mInputManager.getInputDevice(state.deviceId);
        if (device == null) return false;

        int peak = Math.max(clampedWeak, clampedStrong);
        int amplitude = Math.max(1, Math.min(255, Math.round((peak / 65535.0f) * 255.0f)));
        try {
            VibrationEffect effect = VibrationEffect.createOneShot(clampedDuration, amplitude);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                VibratorManager vm = device.getVibratorManager();
                if (vm != null && vm.getVibratorIds().length > 0) {
                    vm.vibrate(CombinedVibration.createParallel(effect));
                    armRumbleWatchdogLocked(port, clampedDuration);
                    return true;
                }
            }
            @SuppressWarnings("deprecation")
            Vibrator vib = device.getVibrator();
            if (vib != null && vib.hasVibrator()) {
                vib.vibrate(effect);
                armRumbleWatchdogLocked(port, clampedDuration);
                return true;
            }
        } catch (Exception ignored) {
        }
        return false;
    }

    private void armRumbleWatchdogLocked(int port, int durationMs) {
        Runnable stop = () -> {
            synchronized (NativeInputController.this) {
                stopPortRumbleLocked(port);
            }
        };
        mPorts[port].rumbleStop = stop;
        mMain.postDelayed(stop, durationMs + 16L);
    }

    public synchronized JSONObject pollSnapshot() throws Exception {
        if (mGameplayConsumerActive && mActive) {
            mLastPollUptimeMs = SystemClock.uptimeMillis();
        }
        JSONObject result = new JSONObject();
        boolean focused = mActive && mAttachedView != null && mAttachedView.hasWindowFocus();
        result.put("active", focused);
        JSONArray ports = new JSONArray();
        for (int i = 0; i < MAX_PORTS; i++) {
            PortState state = mPorts[i];
            if (state.deviceId < 0) continue;
            JSONObject entry = new JSONObject();
            entry.put("port", i);
            entry.put("connected", true);
            entry.put("label", "Port " + (i + 1) + " · " + state.name);
            entry.put("buttons_mask", focused ? state.combinedMask() : 0);
            JSONArray axes = new JSONArray();
            axes.put(focused ? (int) state.lx : 0);
            axes.put(focused ? (int) state.ly : 0);
            axes.put(focused ? (int) state.rx : 0);
            axes.put(focused ? (int) state.ry : 0);
            entry.put("axes", axes);
            JSONArray triggers = new JSONArray();
            triggers.put(focused ? state.leftTriggerPermille : 0);
            triggers.put(focused ? state.rightTriggerPermille : 0);
            entry.put("triggers_permille", triggers);
            JSONArray pressure = new JSONArray();
            for (int b = 0; b < 16; b++) {
                int p = focused ? state.buttonPressure[b] : 0;
                if (p == 0 && focused && ((state.combinedMask() & (1 << b)) != 0)) {
                    p = 1000;
                }
                pressure.put(p);
            }
            entry.put("pressure_permille", pressure);
            entry.put("rumble_supported", state.rumbleSupported);
            ports.put(entry);
        }
        result.put("ports", ports);
        return result;
    }
}
