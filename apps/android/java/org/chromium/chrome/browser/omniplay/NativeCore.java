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
    private static native byte[] nativePairingClientV1(long handle,byte[] input,int action,int budget);
    // Host: 0 begin, 1 register, 2 poll, 3 close, 4 persist peer, 5 decline,
    // 6 review saved host candidate, 7 activate reviewed host candidate.
    private static native byte[] nativePairingHostV1(long handle,byte[] input,int action,int budget);
    // Raw bearer-proof bytes; never pass to pairingResponse/log/renderer.
    private static native byte[] nativePairingHostShareV1(long handle,byte[] input,int budget);
    private final java.util.concurrent.atomic.AtomicBoolean mPairingClientBusy=new java.util.concurrent.atomic.AtomicBoolean();
    private final java.util.concurrent.atomic.AtomicReference<Object> mPairingClientDeliveryFence=new java.util.concurrent.atomic.AtomicReference<>(new Object());
    /** Dedicated trusted UI lane; consumes and clears the supplied byte array
     * on every path. UI must independently pin host and confirm peer/input.
     * action 0 begin, 1 join, 2 poll, 3 consented persistence, 4 local cancel,
     * 5 activation of the exact reviewed persisted client candidate.
     * Never expose through renderer request/nativeCall or forward raw replies.
     */
    void pairingClient(byte[] ownedInput,int action,java.util.function.BooleanSupplier alive,Reply reply){
        int maximum=action==0?16_384:(action>=5?1024:(action==3?256:128));
        if(ownedInput==null||ownedInput.length==0||ownedInput.length>maximum||action<0||action>6){
            if(ownedInput!=null)java.util.Arrays.fill(ownedInput,(byte)0);reply.accept(failure("","invalid_request","Native pairing request is invalid."));return;
        }
        final Object deliveryFence=action==4?new Object():mPairingClientDeliveryFence.get();
        // Fence already-produced replies even if native invalidation/queueing fails.
        if(action==4)mPairingClientDeliveryFence.set(deliveryFence);
        // Cancel signals native in-flight work before waiting for serialized I/O.
        if(action==4)try{
            long current=mHandle;
            if(current==0||!nativePairingInvalidateEpoch(current)){
                java.util.Arrays.fill(ownedInput,(byte)0);
                reply.accept(failure("","runtime_unavailable","Cancellation was not acknowledged. Inspect native status before continuing."));return;
            }
        }catch(LinkageError unavailable){java.util.Arrays.fill(ownedInput,(byte)0);reply.accept(failure("","unsupported_capability","Native pairing is unavailable."));return;}
        boolean reserved=action==4?false:mPairingClientBusy.compareAndSet(false,true);
        if(action!=4&&!reserved){java.util.Arrays.fill(ownedInput,(byte)0);reply.accept(failure("","busy","Native pairing work is still finishing."));return;}
        if(mWorkPending.incrementAndGet()>32){mWorkPending.decrementAndGet();if(reserved)mPairingClientBusy.set(false);java.util.Arrays.fill(ownedInput,(byte)0);reply.accept(failure("","busy","Native work is still finishing."));return;}
        final long expires=android.os.SystemClock.elapsedRealtime()+25_000;
        try{mIo.execute(()->{
            JSONObject response;
            try{
                String expectedTicket=null,reviewedHash=null;
                JSONObject activation=null;
                if(action>=5){
                    activation=new JSONObject(new String(ownedInput,StandardCharsets.UTF_8));
                    JSONObject address=activation.getJSONObject("address");Object revision=activation.get("expected_revision");
                    if(activation.length()!=(action==5?3:2)||address.length()!=5||!mProfile.equals(address.getString("profile"))||!"pair_encryption".equals(address.getString("kind"))
                            ||!canonicalPairingId(address.getString("device"))||!canonicalPairingId(address.getString("id"))||!canonicalPairingId(address.getString("generation"))
                            ||(action==5&&!activation.getString("reviewed_grant_hash").matches("[0-9a-f]{64}"))||!(revision instanceof Number)
                            ||((Number)revision).doubleValue()!=((Number)revision).longValue()||((Number)revision).longValue()<1||((Number)revision).longValue()>=10_000_000)throw new IllegalStateException();
                }else if(action!=0){
                    JSONObject request=new JSONObject(new String(ownedInput,StandardCharsets.UTF_8));
                    expectedTicket=request.getString("ticket");
                    if(request.length()!=(action==3?2:1)||!canonicalPairingId(expectedTicket))throw new IllegalStateException();
                    if(action==3){reviewedHash=request.getString("reviewed_grant_hash");if(!reviewedHash.matches("[0-9a-f]{64}"))throw new IllegalStateException();}
                }
                long remaining=expires-android.os.SystemClock.elapsedRealtime();
                if(mHandle==0||mFailed||remaining<1000||(action!=4&&(mPairingClientDeliveryFence.get()!=deliveryFence||!alive.getAsBoolean()||!mPairingAttached)))throw new IllegalStateException();
                response=pairingResponse(nativePairingClientV1(mHandle,ownedInput,action,(int)Math.min(25_000,remaining)));
                if("ok".equals(response.optString("status"))){
                    JSONObject value=response.getJSONObject("result");
                    if(action==6){
                        JSONObject address=value.getJSONObject("address"),expected=activation.getJSONObject("address");Object revision=value.get("revision"),expiry=value.get("expires_at_ms");
                        if(value.getInt("credential_protocol")!=1||!"material_ready".equals(value.getString("phase"))||address.length()!=5
                                ||!Boolean.TRUE.equals(value.get("local_consent_required"))||!Boolean.FALSE.equals(value.get("comparison_available"))
                                ||!expected.getString("id").equals(value.getString("pair_id"))||!(value.get("input_allowed") instanceof Boolean)
                                ||!value.getString("grant_hash").matches("[0-9a-f]{64}")||!value.getString("host_public_key").matches("[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]")
                                ||!value.getString("host_fingerprint").matches("[0-9a-f]{64}")||!(revision instanceof Number)
                                ||((Number)revision).doubleValue()!=activation.getLong("expected_revision")||!(expiry instanceof Number)
                                ||((Number)expiry).doubleValue()!=((Number)expiry).longValue()||((Number)expiry).longValue()<=System.currentTimeMillis()
                                ||((Number)expiry).longValue()>9_007_199_254_740_991L)throw new IllegalStateException();
                        for(String field:new String[]{"profile","device","kind","id","generation"})if(!expected.getString(field).equals(address.getString(field)))throw new IllegalStateException();
                    }else if(action==5){
                        JSONObject address=value.getJSONObject("address"),expected=activation.getJSONObject("address");Object revision=value.get("revision");
                        if(value.getInt("credential_protocol")!=1||!"active".equals(value.getString("phase"))||!Boolean.TRUE.equals(value.get("reload_required"))
                                ||!activation.getString("reviewed_grant_hash").equals(value.getString("grant_hash"))||address.length()!=5
                                ||!(revision instanceof Number)||((Number)revision).doubleValue()!=((Number)revision).longValue()
                                ||((Number)revision).longValue()<=activation.getLong("expected_revision")||((Number)revision).longValue()>10_000_000)throw new IllegalStateException();
                        for(String field:new String[]{"profile","device","kind","id","generation"})if(!expected.getString(field).equals(address.getString(field)))throw new IllegalStateException();
                    }else if(action==3){
                        if(!expectedTicket.equals(value.getString("pairing_ticket")))throw new IllegalStateException();
                        if(!reviewedHash.equals(value.getString("grant_hash")))throw new IllegalStateException();
                        JSONObject address=value.getJSONObject("address");
                        if(value.getInt("credential_protocol")!=1||!"material_ready".equals(value.getString("phase"))||!mProfile.equals(address.getString("profile"))||!"pair_encryption".equals(address.getString("kind")))throw new IllegalStateException();
                        Object revision=value.get("revision");
                        if(!(revision instanceof Number)||((Number)revision).doubleValue()!=((Number)revision).longValue()
                                ||((Number)revision).longValue()<1||((Number)revision).longValue()>10_000_000
                                ||!canonicalPairingId(address.getString("device"))||!canonicalPairingId(address.getString("id"))
                                ||!canonicalPairingId(address.getString("generation"))||!value.getString("public_key").matches("[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]"))throw new IllegalStateException();
                    }else if(value.getInt("pairing_protocol")!=1||!canonicalPairingId(value.getString("ticket"))||(expectedTicket!=null&&!expectedTicket.equals(value.getString("ticket"))))throw new IllegalStateException();
                    if(action==0){
                        Object expiry=value.get("expires_at_ms"),revision=value.get("revision");
                        if(!value.getString("comparison_code").matches("[0-9]{6}")
                                ||!value.getString("host_public_key").matches("[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]")
                                ||!value.getString("host_fingerprint").matches("[0-9a-f]{64}")
                                ||!(expiry instanceof Number)||((Number)expiry).doubleValue()!=((Number)expiry).longValue()
                                ||((Number)expiry).longValue()<=System.currentTimeMillis()||((Number)expiry).longValue()>9_007_199_254_740_991L
                                ||!(revision instanceof Number)||((Number)revision).doubleValue()!=((Number)revision).longValue()
                                ||((Number)revision).longValue()<1||((Number)revision).longValue()>10_000_000)throw new IllegalStateException();
                    }
                    if(action==1&&(!"pending".equals(value.getString("state"))||!Boolean.FALSE.equals(value.get("approves_peer"))))throw new IllegalStateException();
                    if(action==2){
                        if(!Boolean.TRUE.equals(value.get("local_consent_required")))throw new IllegalStateException();
                        if(!value.getString("host_public_key").matches("[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]")
                                ||!value.getString("host_fingerprint").matches("[0-9a-f]{64}")||!value.getString("comparison_code").matches("[0-9]{6}"))throw new IllegalStateException();
                        JSONObject status=value.getJSONObject("status");String state=status.getString("state");
                        if("approved".equals(state)){
                            if(!value.getString("grant_hash").matches("[0-9a-f]{64}"))throw new IllegalStateException();
                            Object expiry=status.get("expires_at_ms");
                            if(!canonicalPairingId(status.getString("pair_id"))||!(status.get("input_allowed") instanceof Boolean)
                                    ||!(expiry instanceof Number)||((Number)expiry).doubleValue()!=((Number)expiry).longValue()
                                    ||((Number)expiry).longValue()<=System.currentTimeMillis()||((Number)expiry).longValue()>9_007_199_254_740_991L)throw new IllegalStateException();
                        }else if(!"pending".equals(state)&&!"declined".equals(state))throw new IllegalStateException();
                    }
                    if(action==4&&(!Boolean.TRUE.equals(value.get("cancelled"))||!Boolean.FALSE.equals(value.get("remote_revoked"))))throw new IllegalStateException();
                }
                if(action!=4&&(mPairingClientDeliveryFence.get()!=deliveryFence||!alive.getAsBoolean()))response=failure("","cancelled","Pairing response became stale. Inspect native status before continuing.");
            }catch(Exception|LinkageError error){response=failure("","runtime_unavailable","Pairing result is uncertain. Inspect native status before continuing.");}
            finally{java.util.Arrays.fill(ownedInput,(byte)0);mWorkPending.decrementAndGet();if(reserved)mPairingClientBusy.set(false);}
            final JSONObject completed=response;mMain.post(()->{
                // Queue-to-main delay is part of the original operation. Never
                // deliver an old success into a replaced native dialog.
                if(android.os.SystemClock.elapsedRealtime()>=expires||(action!=4&&(mPairingClientDeliveryFence.get()!=deliveryFence||!alive.getAsBoolean())))reply.accept(failure("","cancelled","Pairing result became stale. Inspect native status before continuing."));
                else reply.accept(completed);
            });
        });}catch(java.util.concurrent.RejectedExecutionException rejected){java.util.Arrays.fill(ownedInput,(byte)0);mWorkPending.decrementAndGet();if(reserved)mPairingClientBusy.set(false);reply.accept(failure("","busy","Native pairing lane is unavailable."));}
    }
    private static native boolean nativePairingInvalidateEpoch(long handle);
    private static native byte[] nativePairingIdentity(int operation,long handle,long revision,byte[] input);
    /** Native host ticket operations only. Never renderer dispatch/raw forwarding. */
    void pairingHostTicket(String ticket,int action,java.util.function.BooleanSupplier alive,Reply reply){
        if(!canonicalPairingId(ticket)||action<1||action>3){reply.accept(failure("","invalid_request","Invalid native host ticket action."));return;}
        final boolean closing=action==3;
        final Object fence=closing?new Object():mPairingClientDeliveryFence.get();
        if(closing){
            mPairingClientDeliveryFence.set(fence);
            try{if(mHandle==0||!nativePairingInvalidateEpoch(mHandle)){reply.accept(failure("","runtime_unavailable","Host close invalidation was not acknowledged. Inspect status."));return;}}
            catch(LinkageError error){reply.accept(failure("","unsupported_capability","Native host control is unavailable."));return;}
        }
        final boolean reserved=!closing&&mPairingClientBusy.compareAndSet(false,true);
        if(!closing&&!reserved){reply.accept(failure("","busy","Native pairing work is still finishing."));return;}
        if(mWorkPending.incrementAndGet()>32){mWorkPending.decrementAndGet();if(reserved)mPairingClientBusy.set(false);reply.accept(failure("","busy","Native queue is full."));return;}
        final long deadline=android.os.SystemClock.elapsedRealtime()+25_000;
        try{mIo.execute(()->{
            JSONObject response;byte[] input=null;
            try{
                long remaining=deadline-android.os.SystemClock.elapsedRealtime();
                if(mHandle==0||mFailed||!mPairingAttached||remaining<1000||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=fence)throw new IllegalStateException();
                input=new JSONObject().put("ticket",ticket).toString().getBytes(StandardCharsets.UTF_8);
                response=pairingResponse(nativePairingHostV1(mHandle,input,action,(int)Math.min(25_000,remaining)));
                if("ok".equals(response.optString("status"))){
                    JSONObject value=pairingResult(response);
                    if(value.getInt("pairing_protocol")!=1||!ticket.equals(value.getString("ticket")))throw new IllegalStateException();
                    if(action==1&&(!Boolean.TRUE.equals(value.get("registered"))||!Boolean.TRUE.equals(value.get("share_available"))||!Boolean.FALSE.equals(value.get("approves_peer"))))throw new IllegalStateException();
                    if(action==2){
                        Object revision=value.get("revision");org.json.JSONArray peers=value.getJSONArray("peers");
                        if(!Boolean.TRUE.equals(value.get("local_consent_required"))||peers.length()>8||!(revision instanceof Number)
                                ||((Number)revision).doubleValue()!=((Number)revision).longValue()||((Number)revision).longValue()<1||((Number)revision).longValue()>10_000_000)throw new IllegalStateException();
                        java.util.HashSet<String> keys=new java.util.HashSet<>();
                        for(int i=0;i<peers.length();i++){
                            JSONObject peer=peers.getJSONObject(i);String key=peer.getString("client_public_key"),label=peer.getString("label");
                            if(!key.matches("[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]")||!keys.add(key)||!peer.getString("hello_hash").matches("[0-9a-f]{64}")
                                    ||!peer.getString("comparison_code").matches("[0-9]{6}")||label.isEmpty()||label.getBytes(StandardCharsets.UTF_8).length>120)throw new IllegalStateException();
                            for(int j=0;j<label.length();j++)if(Character.isISOControl(label.charAt(j)))throw new IllegalStateException();
                        }
                    }
                    if(action==3){
                        if(!Boolean.TRUE.equals(value.get("closed"))||!Boolean.FALSE.equals(value.get("pairs_revoked")))throw new IllegalStateException();
                        if(!Boolean.TRUE.equals(value.get("relay_close_confirmed"))&&(!Boolean.FALSE.equals(value.get("remote_close_required"))||!"never_registered".equals(value.getString("reason"))))throw new IllegalStateException();
                    }
                }
            }catch(Exception|LinkageError error){response=failure("","runtime_unavailable","Host result is uncertain. Inspect native status before retrying.");}
            finally{if(input!=null)java.util.Arrays.fill(input,(byte)0);mWorkPending.decrementAndGet();if(reserved)mPairingClientBusy.set(false);}
            final JSONObject completed=response;mMain.post(()->{
                if(android.os.SystemClock.elapsedRealtime()>=deadline||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=fence)reply.accept(failure("","cancelled","Host result became stale. Inspect status."));
                else reply.accept(completed);
            });
        });}catch(java.util.concurrent.RejectedExecutionException error){mWorkPending.decrementAndGet();if(reserved)mPairingClientBusy.set(false);reply.accept(failure("","busy","Native host queue is unavailable."));}
    }
    /** bytes are borrowed only during callback; never retain/log/forward them. */
    interface HostShareReply {void accept(byte[] bytes,String error);}
    void sharePairingHost(String ticket,java.util.function.BooleanSupplier alive,HostShareReply reply){
        if(!canonicalPairingId(ticket)||!alive.getAsBoolean()){reply.accept(null,"Host share ticket is unavailable.");return;}
        if(!mPairingClientBusy.compareAndSet(false,true)){reply.accept(null,"Native pairing work is still finishing.");return;}
        if(mWorkPending.incrementAndGet()>32){mWorkPending.decrementAndGet();mPairingClientBusy.set(false);reply.accept(null,"Native queue is full.");return;}
        final Object fence=mPairingClientDeliveryFence.get();final long deadline=android.os.SystemClock.elapsedRealtime()+25_000;
        try{mIo.execute(()->{
            byte[] input=null,proof=null;
            try{
                long remaining=deadline-android.os.SystemClock.elapsedRealtime();
                if(mHandle==0||mFailed||!mPairingAttached||remaining<1000||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=fence)throw new IllegalStateException();
                input=new JSONObject().put("ticket",ticket).toString().getBytes(StandardCharsets.UTF_8);
                proof=nativePairingHostShareV1(mHandle,input,(int)Math.min(25_000,remaining));
                if(proof==null||proof.length<=6||proof.length>8192)throw new IllegalStateException();
                byte[] prefix={'O','P','R','P','1','.'};for(int i=0;i<prefix.length;i++)if(proof[i]!=prefix[i])throw new IllegalStateException();
                for(int i=6;i<proof.length;i++){int b=proof[i]&255;if(!((b>=65&&b<=90)||(b>=97&&b<=122)||(b>=48&&b<=57)||b==45||b==95))throw new IllegalStateException();}
                if(android.os.SystemClock.elapsedRealtime()>=deadline||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=fence)throw new IllegalStateException();
                final byte[] borrowed=proof;
                if(!mMain.post(()->{
                    try{
                        if(android.os.SystemClock.elapsedRealtime()>=deadline||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=fence)reply.accept(null,"Host share became stale. Inspect status.");
                        else reply.accept(borrowed,null);
                    }finally{java.util.Arrays.fill(borrowed,(byte)0);}
                }))throw new IllegalStateException();
                proof=null; // main callback now owns clearing, including callback errors.
            }catch(Exception|LinkageError error){mMain.post(()->reply.accept(null,"Host share is unavailable. Inspect status before retrying."));}
            finally{if(input!=null)java.util.Arrays.fill(input,(byte)0);if(proof!=null)java.util.Arrays.fill(proof,(byte)0);mWorkPending.decrementAndGet();mPairingClientBusy.set(false);}
        });}catch(java.util.concurrent.RejectedExecutionException error){mWorkPending.decrementAndGet();mPairingClientBusy.set(false);reply.accept(null,"Native share queue is unavailable.");}
    }
    interface IdentityReply { void accept(IdentityReview review,String error); }
    interface PeerApprovalReply { void accept(PeerApproval review,String error); }
    interface ClientSetupReply { void accept(ClientSetup setup,String error); }
    static final class ClientSetup {
        final java.util.List<String> relayOrigins;final String identityKey;final long expires;
        private final RecoveryRecord identity;private final long revision;private final Object fence;
        private final java.util.concurrent.atomic.AtomicBoolean consumed=new java.util.concurrent.atomic.AtomicBoolean();
        private ClientSetup(RecoveryRecord identity,long revision,java.util.List<String> origins,long expires,Object fence){
            this.identity=identity;this.revision=revision;this.relayOrigins=java.util.Collections.unmodifiableList(origins);
            identityKey=identity.key;this.expires=expires;this.fence=fence;
        }
    }
    /** Read-only trusted native setup: never creates/replaces an identity. */
    void clientPairingSetup(java.util.function.BooleanSupplier alive,ClientSetupReply reply){
        final long deadline=android.os.SystemClock.elapsedRealtime()+30_000;
        final Object fence=mPairingClientDeliveryFence.get();
        java.util.concurrent.atomic.AtomicReference<ClientSetup> prepared=new java.util.concurrent.atomic.AtomicReference<>();
        work(()->{
            if(!alive.getAsBoolean()||!mPairingAttached||mPairingClientDeliveryFence.get()!=fence)throw new IllegalStateException();
            JSONObject response=pairingResponse(nativePairingIdentity(0,mHandle,0,null)),value=pairingResult(response);
            if(!Boolean.TRUE.equals(value.get("policy_available")))return failure("","unsupported_capability","Remote Play pairing is not enabled in this version.");
            RecoverySnapshot snapshot=new RecoverySnapshot(value,mProfile,deadline);
            if(snapshot.namespaceProvenanceMissing||snapshot.pendingClientTicket!=null||snapshot.pendingHostTicket!=null)throw new IllegalStateException();
            RecoveryRecord identity=null;
            for(RecoveryRecord record:snapshot.records)if(record.isIdentity()&&!"retired".equals(record.phase)){
                if(identity!=null||!"active".equals(record.phase)||record.sessionRecoveryRequired)throw new IllegalStateException();identity=record;
            }
            if(identity==null)throw new IllegalStateException();
            org.json.JSONArray configured=value.getJSONArray("pairing_origins");java.util.ArrayList<String> origins=new java.util.ArrayList<>();
            if(configured.length()<1||configured.length()>8)throw new IllegalStateException();
            for(int i=0;i<configured.length();i++){
                String origin=configured.getString(i);java.net.URI uri=new java.net.URI(origin);
                if(!"https".equals(uri.getScheme())||uri.getHost()==null||uri.getRawUserInfo()!=null||uri.getRawQuery()!=null||uri.getRawFragment()!=null||!uri.getRawPath().isEmpty()||origins.contains(origin))throw new IllegalStateException();
                origins.add(origin);
            }
            prepared.set(new ClientSetup(identity,snapshot.revision,origins,deadline,fence));return response;
        },response->{
            ClientSetup setup=prepared.get();
            if(setup!=null&&alive.getAsBoolean()&&mPairingClientDeliveryFence.get()==fence&&android.os.SystemClock.elapsedRealtime()<deadline)reply.accept(setup,null);
            else reply.accept(null,"Pairing setup is unavailable. Enable the configured service and inspect identity/status before continuing.");
        });
    }
    /** Explicit native host creation only; registration/sharing are separate. */
    void beginPairingHost(ClientSetup setup,String audience,String label,java.util.function.BooleanSupplier alive,Reply reply){
        if(setup==null||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=setup.fence||android.os.SystemClock.elapsedRealtime()>=setup.expires
                ||!setup.relayOrigins.contains(audience)||label==null||label.isEmpty()||label.getBytes(StandardCharsets.UTF_8).length>120||!setup.consumed.compareAndSet(false,true)){
            reply.accept(failure("","cancelled","Host setup ended. Reopen setup."));return;
        }
        if(!mPairingClientBusy.compareAndSet(false,true)){reply.accept(failure("","busy","Native pairing work is still finishing."));return;}
        if(mWorkPending.incrementAndGet()>32){mWorkPending.decrementAndGet();mPairingClientBusy.set(false);reply.accept(failure("","busy","Native queue is full."));return;}
        final long deadline=Math.min(setup.expires,android.os.SystemClock.elapsedRealtime()+25_000);
        try{mIo.execute(()->{
            JSONObject response;byte[] input=null;
            try{
                long remaining=deadline-android.os.SystemClock.elapsedRealtime();
                if(mHandle==0||mFailed||!mPairingAttached||remaining<1000||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=setup.fence)throw new IllegalStateException();
                input=new JSONObject().put("identity",setup.identity.address()).put("expected_revision",setup.revision).put("audience",audience).put("label",label).toString().getBytes(StandardCharsets.UTF_8);
                if(input.length>1024)throw new IllegalStateException();
                response=pairingResponse(nativePairingHostV1(mHandle,input,0,(int)Math.min(25_000,remaining)));
                if("ok".equals(response.optString("status"))){
                    JSONObject value=pairingResult(response);Object revision=value.get("revision"),expiry=value.get("expires_at_ms");
                    if(value.getInt("pairing_protocol")!=1||!canonicalPairingId(value.getString("ticket"))||!canonicalPairingId(value.getString("invite_id"))
                            ||!setup.identityKey.equals(value.getString("host_public_key"))||!Boolean.FALSE.equals(value.get("registered"))||!Boolean.FALSE.equals(value.get("share_available"))
                            ||!(revision instanceof Number)||((Number)revision).doubleValue()!=setup.revision||!(expiry instanceof Number)
                            ||((Number)expiry).doubleValue()!=((Number)expiry).longValue()||((Number)expiry).longValue()<=System.currentTimeMillis()
                            ||((Number)expiry).longValue()>9_007_199_254_740_991L)throw new IllegalStateException();
                }
            }catch(Exception|LinkageError error){response=failure("","runtime_unavailable","Host preparation is uncertain. Inspect status before retrying.");}
            finally{if(input!=null)java.util.Arrays.fill(input,(byte)0);mWorkPending.decrementAndGet();mPairingClientBusy.set(false);}
            final JSONObject completed=response;mMain.post(()->{
                if(android.os.SystemClock.elapsedRealtime()>=deadline||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=setup.fence)reply.accept(failure("","cancelled","Host preparation became stale. Inspect status."));
                else reply.accept(completed);
            });
        });}catch(java.util.concurrent.RejectedExecutionException error){mWorkPending.decrementAndGet();mPairingClientBusy.set(false);reply.accept(failure("","busy","Native host queue is unavailable."));}
    }
    /** Native UI independently pins host. UI must clear its code field after call;
     * immutable Java/UI string copies cannot be reliably wiped here. */
    void beginPairingClient(ClientSetup setup,String code,String host,String audience,String label,java.util.function.BooleanSupplier alive,Reply reply){
        if(setup==null||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=setup.fence||android.os.SystemClock.elapsedRealtime()>=setup.expires
                ||!setup.relayOrigins.contains(audience)||code==null||code.length()>8192||!setup.consumed.compareAndSet(false,true)){
            reply.accept(failure("","cancelled","Pairing setup ended. Open setup again."));return;
        }
        try{
            byte[] request=new JSONObject().put("identity",setup.identity.address()).put("expected_revision",setup.revision)
                    .put("code",code).put("host_public_key",host).put("audience",audience).put("label",label).toString().getBytes(StandardCharsets.UTF_8);
            pairingClient(request,0,()->alive.getAsBoolean()&&mPairingClientDeliveryFence.get()==setup.fence&&android.os.SystemClock.elapsedRealtime()<setup.expires,reply);
        }catch(Exception error){reply.accept(failure("","invalid_request","Pairing could not be prepared. Reopen setup."));}
    }
    /** Immutable approved-poll snapshot owned by trusted native UI only. */
    static final class PeerApproval {
        final String pairId,grantHash,hostPublicKey,hostFingerprint,comparisonCode;final boolean inputAllowed;final long expires,grantExpiresAt;
        private final String ticket;private final Object deliveryFence;
        private final java.util.concurrent.atomic.AtomicBoolean consumed=new java.util.concurrent.atomic.AtomicBoolean();
        private final java.util.concurrent.atomic.AtomicBoolean activationIssued=new java.util.concurrent.atomic.AtomicBoolean();
        private PeerApproval(JSONObject value,Object fence) throws Exception {
            JSONObject status=value.getJSONObject("status");
            if(!"approved".equals(status.getString("state")))throw new IllegalStateException();
            ticket=value.getString("ticket");pairId=status.getString("pair_id");grantHash=value.getString("grant_hash");
            hostPublicKey=value.getString("host_public_key");hostFingerprint=value.getString("host_fingerprint");comparisonCode=value.getString("comparison_code");
            inputAllowed=status.getBoolean("input_allowed");deliveryFence=fence;
            grantExpiresAt=status.getLong("expires_at_ms");long remaining=grantExpiresAt-System.currentTimeMillis();
            if(remaining<=0)throw new IllegalStateException();
            expires=android.os.SystemClock.elapsedRealtime()+Math.min(60_000,remaining);
        }
    }
    interface SavedPeerReply {void accept(SavedPeerReview review,String error);}
    static final class SavedPeerReview {
        final String hostPublicKey,hostFingerprint,pairId,generation,audience;
        final boolean inputAllowed;final long grantExpiresAt,expires;
        private final String address,hash;private final long revision;private final Object fence;
        private final java.util.function.BooleanSupplier owner;
        private final java.util.concurrent.atomic.AtomicBoolean consumed=new java.util.concurrent.atomic.AtomicBoolean();
        private SavedPeerReview(JSONObject value,Object fence,java.util.function.BooleanSupplier owner) throws Exception{
            JSONObject scope=value.getJSONObject("address");address=scope.toString();generation=scope.getString("generation");
            hostPublicKey=value.getString("host_public_key");hostFingerprint=value.getString("host_fingerprint");pairId=value.getString("pair_id");audience=value.getString("audience");
            inputAllowed=value.getBoolean("input_allowed");hash=value.getString("grant_hash");revision=value.getLong("revision");this.fence=fence;this.owner=owner;
            grantExpiresAt=value.getLong("expires_at_ms");long remaining=grantExpiresAt-System.currentTimeMillis();
            if(remaining<=0)throw new IllegalStateException();expires=android.os.SystemClock.elapsedRealtime()+Math.min(60_000,remaining);
        }
    }
    void reviewSavedPeer(RecoverySnapshot snapshot,RecoveryRecord record,java.util.function.BooleanSupplier alive,SavedPeerReply reply){
        final Object fence=mPairingClientDeliveryFence.get();
        if(!alive.getAsBoolean()||snapshot.namespaceProvenanceMissing||!snapshot.records.contains(record)||record.isIdentity()
                ||!"material_ready".equals(record.phase)||record.sessionRecoveryRequired||android.os.SystemClock.elapsedRealtime()>=snapshot.expires){reply.accept(null,"Reload native status before reviewing this saved pairing.");return;}
        try{
            byte[] request=new JSONObject().put("address",record.address()).put("expected_revision",snapshot.revision).toString().getBytes(StandardCharsets.UTF_8);
            pairingClient(request,6,()->alive.getAsBoolean()&&android.os.SystemClock.elapsedRealtime()<snapshot.expires,response->{
                SavedPeerReview review;
                try{
                    if(!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=fence||android.os.SystemClock.elapsedRealtime()>=snapshot.expires)throw new IllegalStateException();
                    review=new SavedPeerReview(pairingResult(response),fence,alive);
                }catch(Exception error){reply.accept(null,"Saved pairing could not be reviewed. Reload status; no activation was requested.");return;}
                reply.accept(review,null);
            });
        }catch(Exception error){reply.accept(null,"Saved pairing review could not be queued.");}
    }
    /** Fresh independent host/permission consent is required; no old comparison code exists. */
    void activateSavedPeer(SavedPeerReview review,java.util.function.BooleanSupplier alive,Reply reply){
        if(review==null||!alive.getAsBoolean()||!review.owner.getAsBoolean()||mPairingClientDeliveryFence.get()!=review.fence
                ||android.os.SystemClock.elapsedRealtime()>=review.expires||!review.consumed.compareAndSet(false,true)){
            reply.accept(failure("","cancelled","Saved pairing review ended. Review it again."));return;
        }
        try{
            byte[] request=new JSONObject().put("address",new JSONObject(review.address)).put("expected_revision",review.revision).put("reviewed_grant_hash",review.hash).toString().getBytes(StandardCharsets.UTF_8);
            pairingClient(request,5,()->alive.getAsBoolean()&&review.owner.getAsBoolean()&&mPairingClientDeliveryFence.get()==review.fence&&android.os.SystemClock.elapsedRealtime()<review.expires,reply);
        }catch(Exception error){reply.accept(failure("","runtime_unavailable","Saved pairing activation could not be queued. Inspect status."));}
    }
    static final class PeerActivation {
        final PeerApproval peer;final String generation;final long expires;
        private final String address;private final long revision;
        private final java.util.concurrent.atomic.AtomicBoolean consumed=new java.util.concurrent.atomic.AtomicBoolean();
        private PeerActivation(PeerApproval peer,JSONObject address,long revision) throws Exception{
            this.peer=peer;this.address=address.toString();this.revision=revision;generation=address.getString("generation");expires=peer.expires;
        }
    }
    /** Only the exact successful persistence reply of a consumed peer review. */
    PeerActivation activationForPeer(PeerApproval review,JSONObject response) throws Exception{
        if(review==null||!review.consumed.get()||mPairingClientDeliveryFence.get()!=review.deliveryFence||android.os.SystemClock.elapsedRealtime()>=review.expires)throw new IllegalStateException();
        JSONObject value=pairingResult(response),address=value.getJSONObject("address");Object revision=value.get("revision");
        if(!review.grantHash.equals(value.getString("grant_hash")))throw new IllegalStateException();
        if(value.getInt("credential_protocol")!=1||!"material_ready".equals(value.getString("phase"))||!review.ticket.equals(value.getString("pairing_ticket"))
                ||address.length()!=5||!mProfile.equals(address.getString("profile"))||!review.pairId.equals(address.getString("id"))||!"pair_encryption".equals(address.getString("kind"))
                ||!canonicalPairingId(address.getString("device"))||!canonicalPairingId(address.getString("generation"))||!(revision instanceof Number)
                ||((Number)revision).doubleValue()!=((Number)revision).longValue()||((Number)revision).longValue()<1||((Number)revision).longValue()>=10_000_000)throw new IllegalStateException();
        if(!review.activationIssued.compareAndSet(false,true))throw new IllegalStateException();
        return new PeerActivation(review,address,((Number)revision).longValue());
    }
    void activatePairingPeer(PeerActivation activation,java.util.function.BooleanSupplier alive,Reply reply){
        if(activation==null||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=activation.peer.deliveryFence
                ||android.os.SystemClock.elapsedRealtime()>=activation.expires||!activation.consumed.compareAndSet(false,true)){
            reply.accept(failure("","cancelled","Activation review ended. Inspect native pairing status."));return;
        }
        try{
            byte[] request=new JSONObject().put("address",new JSONObject(activation.address)).put("expected_revision",activation.revision)
                    .put("reviewed_grant_hash",activation.peer.grantHash).toString().getBytes(StandardCharsets.UTF_8);
            pairingClient(request,5,()->alive.getAsBoolean()&&mPairingClientDeliveryFence.get()==activation.peer.deliveryFence&&android.os.SystemClock.elapsedRealtime()<activation.expires,reply);
        }catch(Exception error){reply.accept(failure("","runtime_unavailable","Activation could not be queued. Inspect status."));}
    }
    /** Poll explicitly; pending/declined is not an approvable snapshot. */
    void reviewPairingPeer(String ticket,java.util.function.BooleanSupplier alive,PeerApprovalReply reply){
        final Object fence=mPairingClientDeliveryFence.get();
        try{
            byte[] request=new JSONObject().put("ticket",ticket).toString().getBytes(StandardCharsets.UTF_8);
            pairingClient(request,2,alive,response->{
                PeerApproval review;
                try{
                    if(!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=fence)throw new IllegalStateException();
                    review=new PeerApproval(pairingResult(response),fence);
                }catch(Exception error){reply.accept(null,"No current host approval is available. Read pairing status before continuing.");return;}
                reply.accept(review,null);
            });
        }catch(Exception error){reply.accept(null,"Pairing ticket is unavailable.");}
    }
    /** Call only after actual native peer/comparison/input consent to this snapshot. */
    void confirmPairingPeer(PeerApproval review,java.util.function.BooleanSupplier alive,Reply reply){
        if(review==null||!alive.getAsBoolean()||mPairingClientDeliveryFence.get()!=review.deliveryFence
                ||android.os.SystemClock.elapsedRealtime()>=review.expires||!review.consumed.compareAndSet(false,true)){
            reply.accept(failure("","cancelled","Peer review ended. Review the current host approval again."));return;
        }
        try{
            byte[] request=new JSONObject().put("ticket",review.ticket).put("reviewed_grant_hash",review.grantHash).toString().getBytes(StandardCharsets.UTF_8);
            pairingClient(request,3,()->alive.getAsBoolean()&&mPairingClientDeliveryFence.get()==review.deliveryFence&&android.os.SystemClock.elapsedRealtime()<review.expires,reply);
        }catch(Exception error){reply.accept(failure("","runtime_unavailable","Peer confirmation could not be queued. Inspect native status."));}
    }
    static final class IdentityReview {
        final String id,key,device,generation; final long expires;
        private IdentityReview(JSONObject value,String profile) throws Exception {
            JSONObject address=value.getJSONObject("address");
            id=value.getString("review_id");key=value.getString("public_key");device=address.getString("device");generation=address.getString("generation");
            long ttl=value.getLong("expires_in_ms");
            if(value.getInt("credential_protocol")!=1||!"activate_local_identity".equals(value.getString("action"))||value.getBoolean("approves_remote_peer")
                    ||!profile.equals(address.getString("profile"))||!device.equals(address.getString("id"))||!"device_signing".equals(address.getString("kind"))
                    ||!canonicalPairingId(id)||!canonicalPairingId(device)||!canonicalPairingId(generation)||!key.matches("[A-Za-z0-9_-]{43}")||ttl<=0||ttl>60_000)throw new IllegalStateException();
            expires=android.os.SystemClock.elapsedRealtime()+ttl;
        }
    }
    private static boolean canonicalPairingId(String id){
        try{return UUID.fromString(id).toString().equals(id)&&!new UUID(0,0).toString().equals(id);}catch(IllegalArgumentException error){return false;}
    }
    private static JSONObject pairingResponse(byte[] bytes) throws Exception {
        try{if(bytes==null||bytes.length==0||bytes.length>1_048_576)throw new IllegalStateException();return new JSONObject(new String(bytes,StandardCharsets.UTF_8));}
        finally{if(bytes!=null)java.util.Arrays.fill(bytes,(byte)0);}
    }
    private static JSONObject pairingResult(JSONObject response) throws Exception {
        if(!"ok".equals(response.optString("status")))throw new IllegalStateException();return response.getJSONObject("result");
    }
    /** Native UI only; no automatic replacement or renderer reply forwarding. */
    void reviewPairingIdentity(java.util.function.BooleanSupplier alive,IdentityReply reply){
        java.util.concurrent.atomic.AtomicReference<IdentityReview> candidate=new java.util.concurrent.atomic.AtomicReference<>();
        work(()->{
            if(!alive.getAsBoolean()||!mPairingAttached)throw new IllegalStateException();
            JSONObject status=pairingResult(pairingResponse(nativePairingIdentity(0,mHandle,0,null)));
            if(!status.getBoolean("policy_available"))return failure("","unsupported_capability","Remote Play is not enabled in this version.");
            if(!mProfile.equals(status.getString("profile_id")))throw new IllegalStateException();
            long revision=status.getLong("revision");org.json.JSONArray records=status.getJSONArray("records");JSONObject identity=null;
            for(int i=0;i<records.length();i++){
                JSONObject record=records.getJSONObject(i);
                if("device_signing".equals(record.getJSONObject("address").getString("kind"))&&!"retired".equals(record.getString("phase"))){if(identity!=null)throw new IllegalStateException();identity=record;}
            }
            if(!alive.getAsBoolean())throw new IllegalStateException();
            if(identity==null){
                JSONObject prepared=pairingResult(pairingResponse(nativePairingIdentity(1,mHandle,revision,null)));
                if(!"material_ready".equals(prepared.getString("phase")))throw new IllegalStateException();revision=prepared.getLong("revision");
            }else if(!"material_ready".equals(identity.getString("phase")))return failure("","conflict","An identity already exists or needs recovery. No replacement key was created.");
            if(!alive.getAsBoolean())throw new IllegalStateException();
            JSONObject response=pairingResponse(nativePairingIdentity(2,mHandle,revision,null));JSONObject reviewed=pairingResult(response);
            if(!alive.getAsBoolean()||reviewed.getLong("revision")!=revision||!status.getString("device_id").equals(reviewed.getJSONObject("address").getString("device"))){pairingResponse(nativePairingIdentity(4,mHandle,0,null));throw new IllegalStateException();}
            candidate.set(new IdentityReview(reviewed,mProfile));
            return response;
        },response->{
            IdentityReview review=candidate.get();
            if("ok".equals(response.optString("status"))&&review!=null){reply.accept(review,null);return;}
            if(alive.getAsBoolean())cancelPairingIdentityReview();JSONObject detail=response.optJSONObject("error");reply.accept(null,detail==null?"Identity review ended. Reload its status before continuing.":detail.optString("message","Identity review is unavailable."));
        });
    }
    /** Invoked only by the trusted dialog's explicit approve action. */
    void confirmPairingIdentity(IdentityReview review,java.util.function.BooleanSupplier alive,Reply reply){
        work(()->{
            if(!alive.getAsBoolean()||android.os.SystemClock.elapsedRealtime()>=review.expires||!mPairingAttached)throw new IllegalStateException();
            byte[] input=new JSONObject().put("review_id",review.id).toString().getBytes(StandardCharsets.UTF_8);
            JSONObject response;
            try{response=pairingResponse(nativePairingIdentity(3,mHandle,0,input));}finally{java.util.Arrays.fill(input,(byte)0);}
            JSONObject value=pairingResult(response),address=value.getJSONObject("address");
            if(!alive.getAsBoolean()||!"active".equals(value.getString("phase"))||value.getBoolean("approves_remote_peer")
                    ||!mProfile.equals(address.getString("profile"))||!review.device.equals(address.getString("device"))||!review.device.equals(address.getString("id"))
                    ||!review.generation.equals(address.getString("generation"))||!"device_signing".equals(address.getString("kind")))throw new IllegalStateException();
            return response;
        },reply);
    }
    void cancelPairingIdentityReview(){
        // Native epoch/token invalidation bypasses the serialized lane, which
        // may currently be blocked inside synchronous relay/vault work.
        try{if(mHandle!=0)nativePairingInvalidateEpoch(mHandle);}catch(LinkageError unavailable){/* UI lifetime is already cancelled; native artifact needs qualification. */}
        mIo.execute(()->{if(mHandle!=0)try{pairingResponse(nativePairingIdentity(4,mHandle,0,null));}catch(Exception|LinkageError ignored){/* Review remains bounded by epoch/expiry. */}});
    }
    private static native byte[] nativePairingRecoveryV2(long handle,byte[] input,int budget,int action);
    interface RecoveryReply { void accept(RecoverySnapshot snapshot,String error); }
    static final class RecoveryRecord {
        final String profile,device,kind,item,generation,phase,key,nextStep;
        final boolean sessionRecoveryRequired;
        private RecoveryRecord(JSONObject record,String expectedProfile,String expectedDevice,JSONObject action) throws Exception {
            JSONObject address=record.getJSONObject("address");profile=address.getString("profile");device=address.getString("device");
            kind=address.getString("kind");item=address.getString("id");generation=address.getString("generation");phase=record.getString("phase");key=record.getString("public_key");
            nextStep=action==null?null:action.getString("next_step");
            sessionRecoveryRequired=record.getBoolean("session_recovery_required");
            if(!profile.equals(expectedProfile)||!device.equals(expectedDevice)||!canonicalPairingId(profile)||!canonicalPairingId(device)||!canonicalPairingId(item)||!canonicalPairingId(generation)
                    ||!("device_signing".equals(kind)||"pair_encryption".equals(kind))||("device_signing".equals(kind)&&!item.equals(device))
                    ||!java.util.Arrays.asList("reserved","writing","material_ready","active","revoking","retired").contains(phase)||!key.matches("[A-Za-z0-9_-]{43}")
                    ||!matchesAddress(address)||("retired".equals(phase)!=(action==null)))throw new IllegalStateException();
            if(action!=null&&(!matchesAddress(action.getJSONObject("address"))||!java.util.Arrays.asList("fence_generation","revalidate_before_use","retire_writer","retire_local_sessions","wait_for_pair_cleanup","revoke_relay","remove_material","finalize_retirement").contains(nextStep)))throw new IllegalStateException();
        }
        boolean isIdentity(){return "device_signing".equals(kind);}
        boolean canAdvance(){return "active".equals(phase)||"fence_generation".equals(nextStep)||"finalize_retirement".equals(nextStep)||"remove_material".equals(nextStep)||"revoke_relay".equals(nextStep);}
        private boolean matchesAddress(JSONObject value) throws Exception {
            return value.length()==5&&profile.equals(value.getString("profile"))&&device.equals(value.getString("device"))&&kind.equals(value.getString("kind"))&&item.equals(value.getString("id"))&&generation.equals(value.getString("generation"));
        }
        private JSONObject address() throws Exception{return new JSONObject().put("profile",profile).put("device",device).put("kind",kind).put("id",item).put("generation",generation);}
    }
    static final class RecoverySnapshot {
        final long revision,expires;final java.util.List<RecoveryRecord> records;
        final boolean relayRetirementAvailable;
        final boolean namespaceProvenanceMissing;
        final String pendingClientTicket;
        final String pendingHostTicket;
        boolean needsAttention(){if(namespaceProvenanceMissing)return true;for(RecoveryRecord record:records)if(record.sessionRecoveryRequired||(record.nextStep!=null&&!"revalidate_before_use".equals(record.nextStep)))return true;return false;}
        private RecoverySnapshot(JSONObject status,String profile,long deadline) throws Exception {
            if(status.getInt("credential_protocol")!=1||status.getInt("recovery_protocol")!=1||!profile.equals(status.getString("profile_id"))||!canonicalPairingId(status.getString("device_id")))throw new IllegalStateException();
            revision=status.getLong("revision");expires=deadline;
            relayRetirementAvailable=status.getBoolean("relay_retirement_available");
            namespaceProvenanceMissing=status.getBoolean("namespace_provenance_missing");
            JSONObject client=status.optJSONObject("pending_client");String ticket=null;
            if(client!=null){
                String id=client.getString("ticket");if(!canonicalPairingId(id))throw new IllegalStateException();
                if(!client.getBoolean("expired")&&client.getLong("expires_at_ms")>System.currentTimeMillis())ticket=id;
            }
            pendingClientTicket=ticket;
            JSONObject host=status.optJSONObject("pending_host");String hostTicket=null;
            if(host!=null){
                hostTicket=host.getString("ticket");
                if(!canonicalPairingId(hostTicket)||!(host.get("expired") instanceof Boolean)||!(host.get("locally_closed") instanceof Boolean)||!(host.get("remote_close_required") instanceof Boolean))throw new IllegalStateException();
            }
            pendingHostTicket=hostTicket;
            if(revision<0||revision>10_000_000||android.os.SystemClock.elapsedRealtime()>=expires)throw new IllegalStateException();
            org.json.JSONArray rows=status.getJSONArray("records"),actions=status.getJSONArray("recovery");
            if(rows.length()>128||actions.length()>128)throw new IllegalStateException();
            java.util.Map<String,JSONObject> pending=new java.util.HashMap<>();
            for(int i=0;i<actions.length();i++){
                JSONObject action=actions.getJSONObject(i);String generation=action.getJSONObject("address").getString("generation");
                if(pending.put(generation,action)!=null)throw new IllegalStateException();
            }
            java.util.ArrayList<RecoveryRecord> parsed=new java.util.ArrayList<>();java.util.Set<String> seen=new java.util.HashSet<>();
            for(int i=0;i<rows.length();i++){
                JSONObject row=rows.getJSONObject(i);String generation=row.getJSONObject("address").getString("generation");
                if(!seen.add(generation))throw new IllegalStateException();
                parsed.add(new RecoveryRecord(row,profile,status.getString("device_id"),pending.remove(generation)));
            }
            if(!pending.isEmpty())throw new IllegalStateException();records=java.util.Collections.unmodifiableList(parsed);
        }
    }
    /** Read only, including when installed streaming policy is disabled. */
    void pairingRecoveryStatus(java.util.function.BooleanSupplier alive,RecoveryReply reply){
        java.util.concurrent.atomic.AtomicReference<RecoverySnapshot> snapshot=new java.util.concurrent.atomic.AtomicReference<>();
        work(()->{
            if(!alive.getAsBoolean()||!mPairingAttached)throw new IllegalStateException();
            long expires=android.os.SystemClock.elapsedRealtime()+30_000;
            JSONObject response=pairingResponse(nativePairingIdentity(0,mHandle,0,null));
            snapshot.set(new RecoverySnapshot(pairingResult(response),mProfile,expires));
            if(!alive.getAsBoolean())throw new IllegalStateException();return response;
        },response->{if("ok".equals(response.optString("status"))&&snapshot.get()!=null)reply.accept(snapshot.get(),null);else reply.accept(null,"Recovery status is unavailable. Open it again before continuing.");});
    }
    /** Explicit approval of the exact snapshot record; no automatic cleanup. */
    void advancePairingRecovery(RecoverySnapshot snapshot,RecoveryRecord record,java.util.function.BooleanSupplier alive,Reply reply){
        work(()->{
            long remaining=snapshot.expires-android.os.SystemClock.elapsedRealtime();
            if(!alive.getAsBoolean()||!mPairingAttached||remaining<1_000||snapshot.namespaceProvenanceMissing||!snapshot.records.contains(record)||!record.canAdvance()||("revoke_relay".equals(record.nextStep)&&!snapshot.relayRetirementAvailable))throw new IllegalStateException();
            byte[] input=new JSONObject().put("address",record.address()).put("expected_revision",snapshot.revision).toString().getBytes(StandardCharsets.UTF_8);
            JSONObject response;
            int action="active".equals(record.phase)?3:("revoke_relay".equals(record.nextStep)?2:("remove_material".equals(record.nextStep)?1:0));
            try{response=pairingResponse(nativePairingRecoveryV2(mHandle,input,(int)Math.min(25_000,remaining),action));}finally{java.util.Arrays.fill(input,(byte)0);}
            JSONObject value=pairingResult(response);String outcome=value.getJSONObject("result").getString("outcome");
            if(!alive.getAsBoolean()||value.getInt("recovery_protocol")!=1||!value.getBoolean("reload_required")||!record.matchesAddress(value.getJSONObject("address"))
                    ||!java.util.Arrays.asList("committed","pending","revalidation_required","already_retired").contains(outcome))throw new IllegalStateException();
            return response;
        },reply);
    }
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
