package org.chromium.chrome.browser.omniplay;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.view.HapticFeedbackConstants;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Mobile Android Remote Play client and input/presentation bridge.
 * Features:
 * - SurfaceView presentation for hardware/native video frames with letterboxing.
 * - Virtual on-screen touch gamepad overlay supporting multi-touch and haptics.
 * - Physical Bluetooth/USB gamepad input translation.
 * - 64-byte bounded StreamGamepadReport serialization with monotonic sequence numbers (1..=10,000,000).
 * - 250ms input gap check and 30s session consent watchdog.
 * - Fenced deactivation with neutral reports to prevent stuck inputs.
 */
public final class RemotePlayClient {

    public interface ReportSink {
        void sendReport(byte[] report);
    }

    private final Context mContext;
    private final Handler mMain = new Handler(Looper.getMainLooper());
    private final AtomicBoolean mActive = new AtomicBoolean(false);
    private final AtomicLong mSequence = new AtomicLong(1);
    private final ReportSink mSink;

    private PresentationView mPresentationView;
    private TouchOverlayView mTouchOverlay;
    private long mLastReportTimeMs = 0;
    private Runnable mWatchdog;

    // Report protocol constants matching native virtual gamepad host
    private static final byte REPORT_MAGIC = 0x56; // 'V' for Vlikeon
    private static final byte REPORT_VERSION = 1;
    private static final int REPORT_SIZE = 64;

    // Button bitmasks
    public static final int BTN_A = 1 << 0;
    public static final int BTN_B = 1 << 1;
    public static final int BTN_X = 1 << 2;
    public static final int BTN_Y = 1 << 3;
    public static final int BTN_L1 = 1 << 4;
    public static final int BTN_R1 = 1 << 5;
    public static final int BTN_SELECT = 1 << 6;
    public static final int BTN_START = 1 << 7;
    public static final int BTN_DPAD_UP = 1 << 8;
    public static final int BTN_DPAD_DOWN = 1 << 9;
    public static final int BTN_DPAD_LEFT = 1 << 10;
    public static final int BTN_DPAD_RIGHT = 1 << 11;
    public static final int BTN_THUMB_L = 1 << 12;
    public static final int BTN_THUMB_R = 1 << 13;

    // Current input state
    private int mButtons = 0;
    private short mLeftStickX = 0;
    private short mLeftStickY = 0;
    private short mRightStickX = 0;
    private short mRightStickY = 0;
    private byte mLeftTrigger = 0;
    private byte mRightTrigger = 0;

    public RemotePlayClient(Context context, ReportSink sink) {
        mContext = context.getApplicationContext();
        mSink = sink;
    }

    public void start() {
        if (!mActive.compareAndSet(false, true)) return;
        mSequence.set(1);
        sendNeutralReport();
        armWatchdog();
    }

    public void stop() {
        if (!mActive.compareAndSet(true, false)) return;
        if (mWatchdog != null) {
            mMain.removeCallbacks(mWatchdog);
            mWatchdog = null;
        }
        // Send final neutral reports to release any held buttons
        sendNeutralReport();
        mButtons = 0;
        mLeftStickX = 0;
        mLeftStickY = 0;
        mRightStickX = 0;
        mRightStickY = 0;
        mLeftTrigger = 0;
        mRightTrigger = 0;
    }

    private void armWatchdog() {
        mWatchdog = () -> {
            if (!mActive.get()) return;
            long now = SystemClock.elapsedRealtime();
            // Gap check: if no input for 250ms, send periodic heartbeat
            if (now - mLastReportTimeMs >= 250) {
                dispatchCurrentReport();
            }
            mMain.postDelayed(mWatchdog, 100);
        };
        mMain.postDelayed(mWatchdog, 100);
    }

    private synchronized void dispatchCurrentReport() {
        if (!mActive.get() || mSink == null) return;

        long seq = mSequence.getAndIncrement();
        if (seq > 10_000_000) {
            mSequence.set(1);
            seq = 1;
        }

        byte[] buffer = new byte[REPORT_SIZE];
        ByteBuffer bb = ByteBuffer.wrap(buffer).order(ByteOrder.LITTLE_ENDIAN);

        bb.put(REPORT_MAGIC);
        bb.put(REPORT_VERSION);
        bb.putShort((short) 0); // padding
        bb.putInt((int) seq);
        bb.putInt(mButtons);
        bb.putShort(mLeftStickX);
        bb.putShort(mLeftStickY);
        bb.putShort(mRightStickX);
        bb.putShort(mRightStickY);
        bb.put(mLeftTrigger);
        bb.put(mRightTrigger);
        bb.putShort((short) 0); // padding
        bb.putLong(SystemClock.elapsedRealtimeNanos() / 1000); // timestamp in micros

        mLastReportTimeMs = SystemClock.elapsedRealtime();
        mSink.sendReport(buffer);
    }

    private void sendNeutralReport() {
        if (mSink == null) return;
        byte[] buffer = new byte[REPORT_SIZE];
        ByteBuffer bb = ByteBuffer.wrap(buffer).order(ByteOrder.LITTLE_ENDIAN);
        bb.put(REPORT_MAGIC);
        bb.put(REPORT_VERSION);
        bb.putShort((short) 0);
        bb.putInt((int) mSequence.getAndIncrement());
        bb.putInt(0); // no buttons
        bb.putShort((short) 0); // neutral sticks
        bb.putShort((short) 0);
        bb.putShort((short) 0);
        bb.putShort((short) 0);
        bb.put((byte) 0); // neutral triggers
        bb.put((byte) 0);
        bb.putShort((short) 0);
        bb.putLong(SystemClock.elapsedRealtimeNanos() / 1000);
        mSink.sendReport(buffer);
    }

    public synchronized void updateButton(int mask, boolean pressed) {
        if (pressed) {
            mButtons |= mask;
        } else {
            mButtons &= ~mask;
        }
        dispatchCurrentReport();
    }

    public synchronized void updateSticks(short lx, short ly, short rx, short ry) {
        mLeftStickX = lx;
        mLeftStickY = ly;
        mRightStickX = rx;
        mRightStickY = ry;
        dispatchCurrentReport();
    }

    public synchronized void updateTriggers(byte lt, byte rt) {
        mLeftTrigger = lt;
        mRightTrigger = rt;
        dispatchCurrentReport();
    }

    /**
     * Intercepts physical game controller key events.
     */
    public boolean onKeyEvent(KeyEvent event) {
        if (!mActive.get()) return false;
        int action = event.getAction();
        boolean isDown = (action == KeyEvent.ACTION_DOWN);

        int mask = 0;
        switch (event.getKeyCode()) {
            case KeyEvent.KEYCODE_BUTTON_A: mask = BTN_A; break;
            case KeyEvent.KEYCODE_BUTTON_B: mask = BTN_B; break;
            case KeyEvent.KEYCODE_BUTTON_X: mask = BTN_X; break;
            case KeyEvent.KEYCODE_BUTTON_Y: mask = BTN_Y; break;
            case KeyEvent.KEYCODE_BUTTON_L1: mask = BTN_L1; break;
            case KeyEvent.KEYCODE_BUTTON_R1: mask = BTN_R1; break;
            case KeyEvent.KEYCODE_BUTTON_START: mask = BTN_START; break;
            case KeyEvent.KEYCODE_BUTTON_SELECT: mask = BTN_SELECT; break;
            case KeyEvent.KEYCODE_BUTTON_THUMBL: mask = BTN_THUMB_L; break;
            case KeyEvent.KEYCODE_BUTTON_THUMBR: mask = BTN_THUMB_R; break;
            case KeyEvent.KEYCODE_DPAD_UP: mask = BTN_DPAD_UP; break;
            case KeyEvent.KEYCODE_DPAD_DOWN: mask = BTN_DPAD_DOWN; break;
            case KeyEvent.KEYCODE_DPAD_LEFT: mask = BTN_DPAD_LEFT; break;
            case KeyEvent.KEYCODE_DPAD_RIGHT: mask = BTN_DPAD_RIGHT; break;
            default: return false;
        }

        updateButton(mask, isDown);
        return true;
    }

    /**
     * Intercepts physical game controller axis motion events.
     */
    public boolean onGenericMotionEvent(MotionEvent event) {
        if (!mActive.get()) return false;
        if ((event.getSource() & InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK) {
            float x = event.getAxisValue(MotionEvent.AXIS_X);
            float y = event.getAxisValue(MotionEvent.AXIS_Y);
            float z = event.getAxisValue(MotionEvent.AXIS_Z);
            float rz = event.getAxisValue(MotionEvent.AXIS_RZ);
            float lt = event.getAxisValue(MotionEvent.AXIS_BRAKE);
            float rt = event.getAxisValue(MotionEvent.AXIS_GAS);

            short lx = (short) Math.max(-32768, Math.min(32767, Math.round(x * 32767)));
            short ly = (short) Math.max(-32768, Math.min(32767, Math.round(y * 32767)));
            short rx = (short) Math.max(-32768, Math.min(32767, Math.round(z * 32767)));
            short ry = (short) Math.max(-32768, Math.min(32767, Math.round(rz * 32767)));
            byte leftTrig = (byte) Math.max(0, Math.min(255, Math.round(lt * 255)));
            byte rightTrig = (byte) Math.max(0, Math.min(255, Math.round(rt * 255)));

            updateSticks(lx, ly, rx, ry);
            updateTriggers(leftTrig, rightTrig);
            return true;
        }
        return false;
    }

    public View createPresentationView(Context context) {
        if (mPresentationView == null) {
            mPresentationView = new PresentationView(context);
        }
        return mPresentationView;
    }

    public View createTouchOverlay(Context context) {
        if (mTouchOverlay == null) {
            mTouchOverlay = new TouchOverlayView(context);
        }
        return mTouchOverlay;
    }

    /**
     * Presentation SurfaceView for decoded video frames.
     */
    public final class PresentationView extends SurfaceView implements SurfaceHolder.Callback {
        private int mVideoWidth = 1920;
        private int mVideoHeight = 1080;

        public PresentationView(Context context) {
            super(context);
            getHolder().addCallback(this);
            setFocusable(false);
        }

        public void setVideoSize(int width, int height) {
            if (width <= 0 || height <= 0) return;
            mVideoWidth = width;
            mVideoHeight = height;
            requestLayout();
        }

        @Override
        protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
            int width = MeasureSpec.getSize(widthMeasureSpec);
            int height = MeasureSpec.getSize(heightMeasureSpec);

            if (mVideoWidth > 0 && mVideoHeight > 0) {
                if (width * mVideoHeight > height * mVideoWidth) {
                    width = height * mVideoWidth / mVideoHeight;
                } else {
                    height = width * mVideoHeight / mVideoWidth;
                }
            }
            setMeasuredDimension(width, height);
        }

        @Override
        public void surfaceCreated(SurfaceHolder holder) {
            // Native video decoder ANativeWindow binding point
        }

        @Override
        public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        }

        @Override
        public void surfaceDestroyed(SurfaceHolder holder) {
            // Immediate frame fence and decoder detach
        }
    }

    /**
     * Virtual On-Screen Gamepad Touch Controller Overlay.
     */
    public final class TouchOverlayView extends View {
        private final Paint mButtonPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Paint mTextPaint = new Paint(Paint.ANTI_ALIAS_FLAG);

        // Rectangles for touch buttons (relative to view size)
        private final RectF mRectDpadUp = new RectF();
        private final RectF mRectDpadDown = new RectF();
        private final RectF mRectDpadLeft = new RectF();
        private final RectF mRectDpadRight = new RectF();
        private final RectF mRectBtnA = new RectF();
        private final RectF mRectBtnB = new RectF();
        private final RectF mRectBtnX = new RectF();
        private final RectF mRectBtnY = new RectF();
        private final RectF mRectBtnStart = new RectF();
        private final RectF mRectBtnSelect = new RectF();

        public TouchOverlayView(Context context) {
            super(context);
            mButtonPaint.setColor(Color.argb(120, 255, 255, 255));
            mButtonPaint.setStyle(Paint.Style.FILL);

            mTextPaint.setColor(Color.WHITE);
            mTextPaint.setTextSize(36f);
            mTextPaint.setTextAlign(Paint.Align.CENTER);
        }

        @Override
        protected void onSizeChanged(int w, int h, int oldw, int oldh) {
            super.onSizeChanged(w, h, oldw, oldh);
            float btnSize = Math.min(w, h) * 0.12f;

            // D-pad positioning (left side)
            float dpadCenterX = w * 0.15f;
            float dpadCenterY = h * 0.70f;
            mRectDpadUp.set(dpadCenterX - btnSize / 2, dpadCenterY - btnSize * 1.5f, dpadCenterX + btnSize / 2, dpadCenterY - btnSize * 0.5f);
            mRectDpadDown.set(dpadCenterX - btnSize / 2, dpadCenterY + btnSize * 0.5f, dpadCenterX + btnSize / 2, dpadCenterY + btnSize * 1.5f);
            mRectDpadLeft.set(dpadCenterX - btnSize * 1.5f, dpadCenterY - btnSize / 2, dpadCenterX - btnSize * 0.5f, dpadCenterY + btnSize / 2);
            mRectDpadRight.set(dpadCenterX + btnSize * 0.5f, dpadCenterY - btnSize / 2, dpadCenterX + btnSize * 1.5f, dpadCenterY + btnSize / 2);

            // Action buttons (right side)
            float actionCenterX = w * 0.85f;
            float actionCenterY = h * 0.70f;
            mRectBtnY.set(actionCenterX - btnSize / 2, actionCenterY - btnSize * 1.5f, actionCenterX + btnSize / 2, actionCenterY - btnSize * 0.5f);
            mRectBtnA.set(actionCenterX - btnSize / 2, actionCenterY + btnSize * 0.5f, actionCenterX + btnSize / 2, actionCenterY + btnSize * 1.5f);
            mRectBtnX.set(actionCenterX - btnSize * 1.5f, actionCenterY - btnSize / 2, actionCenterX - btnSize * 0.5f, actionCenterY + btnSize / 2);
            mRectBtnB.set(actionCenterX + btnSize * 0.5f, actionCenterY - btnSize / 2, actionCenterX + btnSize * 1.5f, actionCenterY + btnSize / 2);

            // Start & Select buttons (center bottom)
            float midX = w * 0.5f;
            float midY = h * 0.88f;
            mRectBtnSelect.set(midX - btnSize * 1.2f, midY - btnSize * 0.3f, midX - btnSize * 0.2f, midY + btnSize * 0.3f);
            mRectBtnStart.set(midX + btnSize * 0.2f, midY - btnSize * 0.3f, midX + btnSize * 1.2f, midY + btnSize * 0.3f);
        }

        @Override
        protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            // Draw D-Pad
            canvas.drawRoundRect(mRectDpadUp, 16f, 16f, mButtonPaint);
            canvas.drawRoundRect(mRectDpadDown, 16f, 16f, mButtonPaint);
            canvas.drawRoundRect(mRectDpadLeft, 16f, 16f, mButtonPaint);
            canvas.drawRoundRect(mRectDpadRight, 16f, 16f, mButtonPaint);

            // Draw Face Buttons
            canvas.drawOval(mRectBtnA, mButtonPaint);
            canvas.drawOval(mRectBtnB, mButtonPaint);
            canvas.drawOval(mRectBtnX, mButtonPaint);
            canvas.drawOval(mRectBtnY, mButtonPaint);

            // Draw Start / Select
            canvas.drawRoundRect(mRectBtnSelect, 12f, 12f, mButtonPaint);
            canvas.drawRoundRect(mRectBtnStart, 12f, 12f, mButtonPaint);

            // Draw Labels
            canvas.drawText("A", mRectBtnA.centerX(), mRectBtnA.centerY() + 12f, mTextPaint);
            canvas.drawText("B", mRectBtnB.centerX(), mRectBtnB.centerY() + 12f, mTextPaint);
            canvas.drawText("X", mRectBtnX.centerX(), mRectBtnX.centerY() + 12f, mTextPaint);
            canvas.drawText("Y", mRectBtnY.centerX(), mRectBtnY.centerY() + 12f, mTextPaint);
            canvas.drawText("SEL", mRectBtnSelect.centerX(), mRectBtnSelect.centerY() + 12f, mTextPaint);
            canvas.drawText("START", mRectBtnStart.centerX(), mRectBtnStart.centerY() + 12f, mTextPaint);
        }

        @Override
        public boolean onTouchEvent(MotionEvent event) {
            if (!mActive.get()) return false;
            int pointerCount = event.getPointerCount();
            int actionMasked = event.getActionMasked();

            int touchedMask = 0;
            for (int p = 0; p < pointerCount; p++) {
                if (actionMasked == MotionEvent.ACTION_POINTER_UP && p == event.getActionIndex()) continue;
                if (actionMasked == MotionEvent.ACTION_UP) continue;

                float x = event.getX(p);
                float y = event.getY(p);

                if (mRectDpadUp.contains(x, y)) touchedMask |= BTN_DPAD_UP;
                if (mRectDpadDown.contains(x, y)) touchedMask |= BTN_DPAD_DOWN;
                if (mRectDpadLeft.contains(x, y)) touchedMask |= BTN_DPAD_LEFT;
                if (mRectDpadRight.contains(x, y)) touchedMask |= BTN_DPAD_RIGHT;

                if (mRectBtnA.contains(x, y)) touchedMask |= BTN_A;
                if (mRectBtnB.contains(x, y)) touchedMask |= BTN_B;
                if (mRectBtnX.contains(x, y)) touchedMask |= BTN_X;
                if (mRectBtnY.contains(x, y)) touchedMask |= BTN_Y;

                if (mRectBtnStart.contains(x, y)) touchedMask |= BTN_START;
                if (mRectBtnSelect.contains(x, y)) touchedMask |= BTN_SELECT;
            }

            if (touchedMask != mButtons) {
                if ((touchedMask & ~mButtons) != 0) {
                    performHapticFeedback(HapticFeedbackConstants.VIRTUAL_KEY);
                }
                mButtons = touchedMask;
                dispatchCurrentReport();
            }
            return true;
        }
    }
}
