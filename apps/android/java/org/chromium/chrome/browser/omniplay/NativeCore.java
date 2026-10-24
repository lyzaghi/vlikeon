package org.chromium.chrome.browser.omniplay;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import org.json.JSONObject;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** A process-owned local profile. Renderers never select paths or handles. */
public final class NativeCore {
    public interface Reply { void accept(JSONObject response); }
    public interface Work { JSONObject run() throws Exception; }
    private static NativeCore sInstance;
    private static boolean sApplicationForeground;
    private static final java.util.WeakHashMap<Object,Boolean> sVisibleActivities = new java.util.WeakHashMap<>();
    private static BackgroundRun sBackgroundRun;
    public static void setApplicationForeground(Object activity, boolean visible) {
        BackgroundRun run;
        synchronized (NativeCore.class) {
            if (visible) sVisibleActivities.put(activity,Boolean.TRUE); else sVisibleActivities.remove(activity);
            sApplicationForeground = !sVisibleActivities.isEmpty(); run = sApplicationForeground ? sBackgroundRun : null;
        }
        if (run != null) run.cancel();
    }
    private final ExecutorService mIo = Executors.newSingleThreadExecutor();
    private final ExecutorService mLifecycle = Executors.newSingleThreadExecutor();
    private final java.util.concurrent.ThreadPoolExecutor mRangeIo=new java.util.concurrent.ThreadPoolExecutor(1,1,0,java.util.concurrent.TimeUnit.MILLISECONDS,new java.util.concurrent.ArrayBlockingQueue<>(1));
    private final java.util.concurrent.atomic.AtomicBoolean mRangeBusy=new java.util.concurrent.atomic.AtomicBoolean();
    // Count executing/queued work, not renderer reply slots: a timed-out reply
    // must not release admission while its native operation is still blocked.
    private final java.util.concurrent.atomic.AtomicInteger mRequests=new java.util.concurrent.atomic.AtomicInteger();
    private final java.util.concurrent.atomic.AtomicInteger mWorkPending=new java.util.concurrent.atomic.AtomicInteger();
    private final java.util.concurrent.atomic.AtomicBoolean mMaintenanceQueued=new java.util.concurrent.atomic.AtomicBoolean();
    private final java.util.concurrent.atomic.AtomicBoolean mNeedsSuspend=new java.util.concurrent.atomic.AtomicBoolean();
    private final java.util.concurrent.atomic.AtomicBoolean mNeedsDetach=new java.util.concurrent.atomic.AtomicBoolean();
    private final Handler mMain = new Handler(Looper.getMainLooper());
    private final File mBase;
    private final Context mContext;
    private volatile long mHandle;
    private final Object mLifecycleLock=new Object();
    private boolean mForeground, mLifecycleScheduled, mLifecycleNeedsCancel;
    private long mLifecycleRevision;
    private String mProfile;
    private File mRoot;
    private boolean mFailed;
    private boolean mPairingAttached;

    public static synchronized NativeCore get(Context context) {
        if (sInstance == null) sInstance = new NativeCore(context.getApplicationContext());
        return sInstance;
    }
    private NativeCore(Context context) {
        mContext=context.getApplicationContext();
        mBase = new File(context.getNoBackupFilesDir(), "OmniPlay");
        mIo.execute(() -> {
            try {
                System.loadLibrary("omniplay_mobile");
                if (!mBase.isDirectory() && !mBase.mkdirs()) throw new IllegalStateException();
                File identity = new File(mBase, "profile-id");
                if (identity.exists()) {
                    if (identity.length() > 64) throw new IllegalStateException();
                    mProfile = UUID.fromString(new String(Files.readAllBytes(identity.toPath()), StandardCharsets.UTF_8).trim()).toString();
                } else {
                    mProfile = UUID.randomUUID().toString();
                    File temporary = new File(mBase, "profile-id.pending");
                    try (FileOutputStream out = new FileOutputStream(temporary)) {
                        out.write(mProfile.getBytes(StandardCharsets.UTF_8)); out.getFD().sync();
                    }
                    if (!temporary.renameTo(identity)) throw new IllegalStateException();
                }
                // Resolve trusted OS storage aliases before the Rust profile/
                // credential lease pins its canonical directory identity.
                mRoot = new File(mBase, mProfile).getCanonicalFile();
                try { pruneStaging(mRoot); } catch (java.io.IOException ignored) { /* Retry bounded private staging cleanup on a later launch. */ }
                byte[] openRequest = new JSONObject().put("data_root", mRoot.getAbsolutePath()).put("profile_id", mProfile).toString().getBytes(StandardCharsets.UTF_8);
                byte[] openReply = null; JSONObject opened;
                try {
                    openReply = nativeOpen(openRequest, new SecureCredentials(mContext, UUID.fromString(mProfile)));
                    if (openReply == null || openReply.length > 1_048_576) throw new IllegalStateException();
                    opened = new JSONObject(new String(openReply, StandardCharsets.UTF_8));
                } finally { java.util.Arrays.fill(openRequest, (byte) 0); if (openReply != null) java.util.Arrays.fill(openReply, (byte) 0); }
                mHandle = opened.getJSONObject("result").getLong("handle");
                if (!"ok".equals(opened.optString("status")) || mHandle == 0) throw new IllegalStateException();
                // Native attachment alone creates/reads no key and enables no
                // streaming capability. Failure must not disable local play or
                // silently substitute the save-sync credential provider.
                try { mPairingAttached = attachPairingCredentials(); }
                catch (Exception | LinkageError unavailable) { mPairingAttached = false; }
                refreshLifecycle();
                try { refreshBackgroundSchedule(); } catch (Exception ignored) { /* Scheduling can be restricted without making local play unavailable. */ }
            } catch (Exception | LinkageError error) { mFailed = true; }
        });
    }
    private static native byte[] nativeCall(int operation, long handle, byte[] input);
    private static native byte[] nativeOpen(byte[] input, SecureCredentials provider);
    private static native byte[] nativePairingScope(long handle);
    private static native byte[] nativeAttachPairing(long handle, PairingCredentials provider);
    private boolean attachPairingCredentials() throws Exception {
        byte[] scopeBytes = null, attachedBytes = null;
        try {
            scopeBytes = nativePairingScope(mHandle);
            if (scopeBytes == null || scopeBytes.length > 4096) return false;
            JSONObject scopeReply = new JSONObject(new String(scopeBytes, StandardCharsets.UTF_8));
            if (!"ok".equals(scopeReply.optString("status"))) return false;
            JSONObject scope = scopeReply.getJSONObject("result");
            if (!mProfile.equals(scope.getString("profile_id"))) return false;
            String deviceText = scope.getString("device_id"); UUID device = UUID.fromString(deviceText);
            if (!device.toString().equals(deviceText) || device.equals(new UUID(0, 0))) return false;
            attachedBytes = nativeAttachPairing(mHandle, new PairingCredentials(mContext, UUID.fromString(mProfile), device));
            if (attachedBytes == null || attachedBytes.length > 4096) return false;
            JSONObject reply = new JSONObject(new String(attachedBytes, StandardCharsets.UTF_8));
            return "ok".equals(reply.optString("status")) && reply.getJSONObject("result").optBoolean("attached", false)
                    && reply.getJSONObject("result").optInt("credential_abi", 0) == 1;
        } finally {
            if (scopeBytes != null) java.util.Arrays.fill(scopeBytes, (byte) 0);
            if (attachedBytes != null) java.util.Arrays.fill(attachedBytes, (byte) 0);
        }
    }
    private static native byte[] nativeAuthorizeMedia(long handle, byte[] input, SelectedDiscTree provider);
    private static native void nativeSetForeground(long handle, boolean foreground);
    private static native byte[] nativeBackgroundStart(long handle, int budget);
    private static native byte[] nativeBackgroundStatus(long handle, byte[] id, boolean cancel);
    /** Native OS task ticket. Cancellation before open/start remains sticky. */
    static final class BackgroundRun {
        private boolean stopped;
        private long handle;
        private String id;
        synchronized boolean active() { return !stopped; }
        synchronized void bind(long owned, String ticket) { handle = owned; id = ticket; if (stopped) status(true); }
        synchronized void cancel() { if (!stopped) { stopped = true; if (id != null) status(true); } }
        synchronized JSONObject status() { return status(false); }
        private JSONObject status(boolean cancel) {
            if (id == null) return failure("", "not_found", "The background sync task has not started.");
            return decodeBackground(nativeBackgroundStatus(handle, id.getBytes(StandardCharsets.US_ASCII), cancel));
        }
    }
    private static JSONObject decodeBackground(byte[] bytes) {
        try { if (bytes == null || bytes.length == 0 || bytes.length > 1_048_576) throw new IllegalStateException(); return new JSONObject(new String(bytes, StandardCharsets.UTF_8)); }
        catch (Exception | LinkageError error) { return failure("", "storage_failure", "Background synchronization is unavailable."); }
        finally { if (bytes != null) java.util.Arrays.fill(bytes, (byte) 0); }
    }
    BackgroundRun beginBackground(long deadline, Reply reply) {
        BackgroundRun run = new BackgroundRun();
        synchronized (NativeCore.class) { sApplicationForeground = !sVisibleActivities.isEmpty(); if (sApplicationForeground) run.cancel(); sBackgroundRun = run; }
        mIo.execute(() -> {
            JSONObject response;
            long remaining = deadline - android.os.SystemClock.elapsedRealtime();
            try {
                if (!run.active() || mFailed || mHandle == 0 || remaining < 1_000) throw new IllegalStateException();
                response = decodeBackground(nativeBackgroundStart(mHandle, (int)Math.min(remaining, 25_000)));
                if ("ok".equals(response.optString("status"))) run.bind(mHandle, response.getJSONObject("result").getString("id"));
            } catch (Exception | LinkageError error) { response = failure("", "cancelled", "The background opportunity ended; local saves remain available."); }
            final JSONObject value = response; mMain.post(() -> reply.accept(value));
        });
        return run;
    }
    private JSONObject refreshBackgroundSchedule() throws Exception {
        JSONObject response = call(6, null);
        if (!"ok".equals(response.optString("status"))) throw new IllegalStateException();
        return BackgroundSyncService.reconcile(mContext, response.getJSONObject("result"));
    }
    private static boolean regular(File path) { return Files.isRegularFile(path.toPath(), java.nio.file.LinkOption.NOFOLLOW_LINKS); }
    private static boolean directory(File path) { return Files.isDirectory(path.toPath(), java.nio.file.LinkOption.NOFOLLOW_LINKS); }
    /** Startup only, before any renderer/profile operation. Never walk game/save data. */
    private static void pruneStaging(File root) throws Exception {
        File inbox=new File(root,"NativeInbox");int inspected=0;
        if(directory(inbox))try(java.nio.file.DirectoryStream<java.nio.file.Path> files=Files.newDirectoryStream(inbox.toPath())) {
            for(java.nio.file.Path path:files){if(++inspected>512)break;File file=path.toFile();String name=file.getName();int dot=name.indexOf('.');
                if(dot!=36||!regular(file)||!name.substring(37).matches("[a-z0-9]{1,12}"))continue;
                try{if(UUID.fromString(name.substring(0,36)).toString().equals(name.substring(0,36)))Files.deleteIfExists(path);}catch(IllegalArgumentException ignored){}
            }
        }
        File exports=new File(root,"NativeExports");inspected=0;
        if(directory(exports))try(java.nio.file.DirectoryStream<java.nio.file.Path> folders=Files.newDirectoryStream(exports.toPath())) {
            for(java.nio.file.Path path:folders){if(++inspected>256)break;File folder=path.toFile();if(!directory(folder))continue;
                try{if(!UUID.fromString(folder.getName()).toString().equals(folder.getName()))continue;}catch(IllegalArgumentException ignored){continue;}
                int children=0;try(java.nio.file.DirectoryStream<java.nio.file.Path> files=Files.newDirectoryStream(path)){
                    for(java.nio.file.Path child:files){if(++children>32)break;if(regular(child.toFile())&&child.getFileName().toString().matches("[A-Za-z0-9_.-]{1,128}"))Files.deleteIfExists(child);}
                }
                try{Files.deleteIfExists(path);}catch(java.nio.file.DirectoryNotEmptyException ignored){}
            }
        }
    }
    private JSONObject call(int operation, JSONObject input) throws Exception {
        byte[] bytes = input == null ? new byte[0] : input.toString().getBytes(StandardCharsets.UTF_8);
        byte[] response = null;
        try {
            if (bytes.length > 1_048_576) throw new IllegalArgumentException();
            response = nativeCall(operation, mHandle, bytes);
            if (response == null || response.length > 1_048_576) throw new IllegalStateException();
            return new JSONObject(new String(response, StandardCharsets.UTF_8));
        } finally { java.util.Arrays.fill(bytes, (byte) 0); if (response != null) java.util.Arrays.fill(response, (byte) 0); }
    }
    /** Work is native-owned and serialized with requests and file authorization. */
    public void work(Work operation, Reply reply) {
        if(mWorkPending.incrementAndGet()>32){mWorkPending.decrementAndGet();reply.accept(failure("","busy","Local document operations are still finishing."));return;}
        mIo.execute(() -> {
            JSONObject value;
            try {
                if (mFailed || mHandle == 0) throw new IllegalStateException();
                value = operation.run();
            } catch (Exception | LinkageError error) { value = failure("", "storage_failure", "The local operation could not be completed. Check storage and access, then retry."); }
            finally {mWorkPending.decrementAndGet();}
            final JSONObject response = value;
            mMain.post(() -> reply.accept(response));
        });
    }
    public void request(JSONObject request, Reply reply) {
        request(request, () -> true, reply);
    }
    public void request(JSONObject request, java.util.function.BooleanSupplier ownerAlive, Reply reply) {
        final String id = request.optString("id");
        JSONObject selectedCommand=request.optJSONObject("command");
        if(selectedCommand!=null&&"asset_range_read".equals(selectedCommand.optString("method"))){rangeRequest(request,ownerAlive,reply);return;}
        if(mRequests.incrementAndGet()>16){mRequests.decrementAndGet();reply.accept(failure(id,"busy","Native operations are still finishing. Please retry."));return;}
        work(() -> {
            if(!ownerAlive.getAsBoolean()||request.optLong("deadline_ms",0)<=System.currentTimeMillis())return failure(id,"cancelled","The request expired or its document closed.");
            request.put("profile_id", mProfile);JSONObject response=call(1,request);JSONObject command=request.optJSONObject("command");
            String method = command == null ? "" : command.optString("method");
            if ("ok".equals(response.optString("status")) && ("hello".equals(method) || "sync_status".equals(method) || "sync_background_configure".equals(method) || "settings_set".equals(method) || method.startsWith("sync_"))) {
                try {
                    JSONObject policy = refreshBackgroundSchedule();
                    if ("sync_status".equals(method)) response.getJSONObject("result").put("background", policy);
                    if ("sync_background_configure".equals(method)) response.put("result", policy);
                } catch (Exception ignored) { /* Persisted consent is authoritative; retry scheduling on the next status/launch. */ }
            }
            if(command!=null&&"hello".equals(command.optString("method"))&&"ok".equals(response.optString("status"))&&SelectedDiscTree.supported()){
                JSONObject result=response.getJSONObject("result");result.getJSONArray("capabilities").put("disc_import");result.put("disc_provider_protocol",1);
            }
            return command!=null&&"device_snapshot".equals(command.optString("method"))?DeviceSnapshot.augment(mContext,response):response;
        }, response -> {
            mRequests.decrementAndGet();
            try { response.put("id", id); } catch (Exception ignored) {}
            reply.accept(response);
        });
    }
    private void rangeRequest(JSONObject request,java.util.function.BooleanSupplier ownerAlive,Reply reply){
        final String id=request.optString("id");
        if(!mRangeBusy.compareAndSet(false,true)){reply.accept(failure(id,"busy","A native media read is still finishing."));return;}
        try{mRangeIo.execute(()->{
            JSONObject value;
            try{if(mHandle==0||mFailed)throw new IllegalStateException();if(!ownerAlive.getAsBoolean()||request.optLong("deadline_ms",0)<=System.currentTimeMillis())value=failure(id,"cancelled","The media request expired or its document closed.");else{request.put("profile_id",mProfile);value=call(1,request);}}
            catch(Exception|LinkageError error){value=failure(id,"storage_failure","The native game asset could not be read.");}
            finally{mRangeBusy.set(false);}
            final JSONObject response=value;mMain.post(()->reply.accept(response));
        });}catch(java.util.concurrent.RejectedExecutionException error){mRangeBusy.set(false);reply.accept(failure(id,"busy","The native media I/O lane is unavailable."));}
    }
    public void storage(java.util.function.Consumer<File> ready, Runnable failed) {
        mIo.execute(() -> {
            if (mFailed || mHandle == 0) { mMain.post(failed); return; }
            try{if(!"ok".equals(call(5,null).optString("status"))){mMain.post(failed);return;}}catch(Exception|LinkageError error){mMain.post(failed);return;}
            final File root=mRoot; mMain.post(() -> ready.accept(root));
        });
    }
    // Native document delegates only; authorization runs in work(), never from message data.
    public File root() { return mRoot; }
    // Call only after storage() is ready, from native document delegates.
    public UUID profileForDocuments() { return UUID.fromString(mProfile); }
    public JSONObject authorize(JSONObject authorization) throws Exception { return call(2, authorization); }
    public JSONObject authorizeMedia(JSONObject metadata,SelectedDiscTree provider)throws Exception {
        byte[] input=metadata.toString().getBytes(StandardCharsets.UTF_8),output=null;
        try {
            if(input.length>1_048_576)throw new IllegalArgumentException();output=nativeAuthorizeMedia(mHandle,input,provider);
            if(output==null||output.length>1_048_576)throw new IllegalStateException();JSONObject response=new JSONObject(new String(output,StandardCharsets.UTF_8));
            if(!"ok".equals(response.optString("status")))provider.abandon();return response;
        }catch(Exception|LinkageError error){provider.abandon();throw error;}
        finally{java.util.Arrays.fill(input,(byte)0);if(output!=null)java.util.Arrays.fill(output,(byte)0);}
    }
    private void scheduleLifecycleLocked() { if(!mLifecycleScheduled){mLifecycleScheduled=true;mLifecycle.execute(this::drainLifecycle);} }
    private void refreshLifecycle() { synchronized(mLifecycleLock){++mLifecycleRevision;scheduleLifecycleLocked();} }
    public void setForeground(boolean value) { synchronized(mLifecycleLock){mForeground=value;if(!value)mLifecycleNeedsCancel=true;++mLifecycleRevision;scheduleLifecycleLocked();} }
    private void drainLifecycle() {
        while(true){
            final boolean active,cancel;final long revision,owned;
            synchronized(mLifecycleLock){active=mForeground;cancel=mLifecycleNeedsCancel;mLifecycleNeedsCancel=false;revision=mLifecycleRevision;owned=mHandle;}
            if(owned!=0){if(cancel)nativeSetForeground(owned,false);if(active||!cancel)nativeSetForeground(owned,active);}
            synchronized(mLifecycleLock){if(revision==mLifecycleRevision){mLifecycleScheduled=false;return;}}
        }
    }
    private void scheduleMaintenance(){
        if(!mMaintenanceQueued.compareAndSet(false,true))return;
        mIo.execute(()->{
            try{
                boolean detach=mNeedsDetach.getAndSet(false),suspend=mNeedsSuspend.getAndSet(false);
                if(!mFailed&&mHandle!=0){if(detach)call(4,null);if(suspend)call(3,null);}
            }catch(Exception|LinkageError error){/* Keep local data; no synthetic successful save receipt. */}
            finally{mMaintenanceQueued.set(false);if(mNeedsDetach.get()||mNeedsSuspend.get())scheduleMaintenance();}
        });
    }
    public void suspend() { mNeedsSuspend.set(true);scheduleMaintenance(); }
    public void detach() { setForeground(false);mNeedsDetach.set(true);scheduleMaintenance(); }
    public static JSONObject success(JSONObject request, Object result) {
        try { return new JSONObject().put("protocol_major", 1).put("protocol_minor", 0).put("id", request.optString("id")).put("status", "ok").put("result", result); }
        catch (Exception impossible) { throw new IllegalStateException(impossible); }
    }
    public static JSONObject failure(String id, String code, String message) {
        try { return new JSONObject().put("protocol_major", 1).put("protocol_minor", 0).put("id", id).put("status", "error").put("error", new JSONObject().put("code", code).put("message", message).put("recoverable", true)); }
        catch (Exception impossible) { throw new IllegalStateException(impossible); }
    }
}
