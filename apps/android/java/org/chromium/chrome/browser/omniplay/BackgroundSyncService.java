package org.chromium.chrome.browser.omniplay;

import android.app.ActivityManager;
import android.app.job.JobInfo;
import android.app.job.JobParameters;
import android.app.job.JobScheduler;
import android.app.job.JobService;
import android.content.ComponentName;
import android.content.Context;
import android.content.BroadcastReceiver;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import org.json.JSONObject;

/** OS-owned periodic encrypted save reconciliation; no ROMs or account changes. */
public final class BackgroundSyncService extends JobService {
    // Check the component before touching this app-wide ID; never replace a
    // Chromium task if a future upstream revision allocates the same number.
    private static final int JOB_ID = 0x4f505301;
    private static final long PERIOD = 15 * 60_000L;
    private final Handler mMain = new Handler(Looper.getMainLooper());
    private NativeCore.BackgroundRun mRun;
    private JobParameters mParameters;
    private Runnable mPoll, mExpiry;
    private boolean mReceiverRegistered;
    private final BroadcastReceiver mLockReceiver = new BroadcastReceiver() {
        @Override public void onReceive(Context context, Intent intent) { if (Intent.ACTION_SCREEN_OFF.equals(intent.getAction()) && mParameters != null) finish(mParameters, false); }
    };

    static JSONObject reconcile(Context context, JSONObject policy) {
        String state = "unavailable"; boolean scheduled = false;
        try {
            JobScheduler scheduler = context.getSystemService(JobScheduler.class);
            ComponentName component = new ComponentName(context, BackgroundSyncService.class);
            if (scheduler != null) {
                JobInfo previous = scheduler.getPendingJob(JOB_ID);
                if (previous != null && !component.equals(previous.getService())) {
                    state = "unavailable";
                } else if (!policy.optBoolean("eligible", false)) {
                    if (previous != null) scheduler.cancel(JOB_ID);
                    state = "disabled";
                } else {
                    scheduled = previous != null || scheduler.schedule(new JobInfo.Builder(JOB_ID, component)
                        .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
                        .setRequiresBatteryNotLow(true).setRequiresStorageNotLow(true)
                        .setPeriodic(PERIOD, 5 * 60_000L).setPersisted(true).build()) == JobScheduler.RESULT_SUCCESS;
                    ActivityManager manager = context.getSystemService(ActivityManager.class);
                    state = manager != null && manager.isBackgroundRestricted() ? "restricted" : scheduled ? "scheduled" : "unavailable";
                }
            }
            policy.put("scheduled", scheduled).put("scheduler_state", state);
        } catch (RuntimeException | org.json.JSONException error) {
            try { policy.put("scheduled", false).put("scheduler_state", "unavailable"); } catch (org.json.JSONException ignored) {}
        }
        return policy;
    }

    @Override public boolean onStartJob(JobParameters parameters) {
        if (parameters.getJobId() != JOB_ID || mParameters != null) return false;
        android.os.PowerManager pm = getSystemService(android.os.PowerManager.class);
        if (pm != null && pm.isPowerSaveMode()) return false;
        mParameters = parameters;
        try {
            IntentFilter filter = new IntentFilter(Intent.ACTION_SCREEN_OFF);
            if (android.os.Build.VERSION.SDK_INT >= 33) registerReceiver(mLockReceiver,filter,Context.RECEIVER_NOT_EXPORTED);
            else registerReceiver(mLockReceiver,filter);
            mReceiverRegistered = true;
        }
        catch (RuntimeException error) { mParameters = null; return false; }
        long deadline = SystemClock.elapsedRealtime() + 25_000;
        mExpiry = () -> finish(parameters, false);
        mMain.postDelayed(mExpiry, 25_000);
        mRun = NativeCore.get(this).beginBackground(deadline, response -> {
            if (mParameters != parameters) return;
            if (!"ok".equals(response.optString("status"))) { finish(parameters, false); return; }
            poll(parameters);
        });
        return true;
    }
    private void poll(JobParameters parameters) {
        if (mParameters != parameters || mRun == null) return;
        JSONObject response = mRun.status(), value = response.optJSONObject("result");
        if (!"ok".equals(response.optString("status")) || value == null) { finish(parameters, false); return; }
        String state = value.optString("state");
        if (!"running".equals(state)) { finish(parameters, "complete".equals(state)); return; }
        mPoll = () -> poll(parameters); mMain.postDelayed(mPoll, 500);
    }
    private void clear() {
        if (mReceiverRegistered) { unregisterReceiver(mLockReceiver); mReceiverRegistered = false; }
        if (mPoll != null) mMain.removeCallbacks(mPoll);
        if (mExpiry != null) mMain.removeCallbacks(mExpiry);
        if (mRun != null) mRun.cancel();
        mRun = null; mParameters = null; mPoll = null; mExpiry = null;
    }
    private void finish(JobParameters parameters, boolean success) {
        if (mParameters != parameters) return;
        clear();
        // Periodic scheduling provides the next opportunity. Do not add an
        // immediate retry loop after expiry, lock, foreground use or network loss.
        jobFinished(parameters, false);
    }
    @Override public boolean onStopJob(JobParameters parameters) {
        if (mParameters == parameters) clear();
        return false;
    }
    @Override public void onTrimMemory(int level) {
        super.onTrimMemory(level);
        if (level >= android.content.ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL && mParameters != null) {
            finish(mParameters, false);
        }
    }
    @Override public void onDestroy() { clear(); super.onDestroy(); }
}
