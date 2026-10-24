package org.chromium.chrome.browser.omniplay;

import android.app.AlertDialog;
import android.os.Handler;
import android.os.Looper;
import org.chromium.chrome.browser.ChromeTabbedActivity;
import org.chromium.chrome.browser.tab.Tab;
import org.chromium.chrome.browser.tab.TabObserver;
import org.chromium.chrome.browser.tab.TabLaunchType;
import org.chromium.chrome.browser.tab.TabSelectionType;
import org.chromium.content_public.browser.GlobalRenderFrameHostId;
import org.chromium.content_public.browser.LoadUrlParams;
import org.chromium.content_public.browser.MessagePayload;
import org.chromium.content_public.browser.MessagePort;
import org.chromium.content_public.browser.NavigationHandle;
import org.chromium.content_public.browser.Page;
import org.chromium.content_public.browser.WebContents;
import org.chromium.content_public.browser.WebContentsObserver;
import org.chromium.url.GURL;
import org.json.JSONObject;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.Set;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;

/** One explicitly opened Chromium tab owns a port; ordinary browsing has no native bridge. */
public final class GameHubController implements AutoCloseable {
    private static GameHubController sActive;
    private static final class Channel {
        final MessagePort port; final WebContents contents;
        final Set<String> requests=new HashSet<>();
        final Map<String,Runnable> expiries=new HashMap<>();
        volatile boolean closed;
        Channel(MessagePort value,WebContents owner){port=value;contents=owner;}
    }
    private final ChromeTabbedActivity mActivity;
    private final Handler mMain=new Handler(Looper.getMainLooper());
    private final NativeCore mCore;
    private AssetServer mServer;
    private NativeDocuments mDocuments;
    private Tab mTab;
    private WebContentsObserver mObserver;
    private Channel mChannel;
    private boolean mClosed;
    private boolean mOpening;
    private boolean mActivityResumed=true;
    private String mSuspension;
    private long mHandshake;
    private java.util.concurrent.atomic.AtomicBoolean mIdentityFlow;
    private AlertDialog mIdentityDialog;
    private Runnable mIdentityExpiry;
    private boolean mPairingStartupChecked;
    private java.util.concurrent.atomic.AtomicBoolean mPairingStartupOwner;
    private final Runnable mPauseFallback;
    private final TabObserver mTabObserver=new TabObserver(){
        @Override public void onShown(Tab tab,int type){resume();}
        @Override public void onHidden(Tab tab,int type){suspend();}
        @Override public void onDestroyed(Tab tab){close();}
    };

    public GameHubController(ChromeTabbedActivity activity){mActivity=activity;mCore=NativeCore.get(activity);mPauseFallback=()->{mSuspension=null;mCore.suspend();};}
    public boolean isClosed(){return mClosed;}
    public void onActivityPause(){mActivityResumed=false;suspend();}
    public void onActivityResume(){mActivityResumed=true;resume();}
    public void resume(){if(!mClosed&&mActivityResumed&&mTab!=null&&mActivity.getActivityTab()==mTab&&!mTab.isIncognito()&&!mActivity.isFinishing()){mMain.removeCallbacks(mPauseFallback);mSuspension=null;mCore.setForeground(true);inspectPairingStartup();}}
    public void open(){
        Tab current=mActivity.getActivityTab();
        if(mClosed||mOpening||mActivity.getTabModelSelector().isIncognitoSelected()||current!=null&&current.isIncognito())return;
        if(mTab!=null&&!mTab.isDestroyed()){
            var model=mActivity.getTabModelSelector().getModel(false);
            int index=model.indexOf(mTab);if(index>=0){model.setIndex(index,TabSelectionType.FROM_USER);resume();showNativeHubActions();return;}
        }
        mOpening=true;if(sActive!=null&&sActive!=this)sActive.close();sActive=this;
        mCore.storage(root->{
            if(mClosed)return;
            // Opening/parsing the small inventory runs off the UI thread.
            new Thread(()->{
                AssetServer server;
                try{server=new AssetServer(mActivity.getAssets());}catch(Exception error){mMain.post(this::showUnavailable);return;}
                mMain.post(()->{if(mClosed){server.close();return;}attach(server);});
            },"OmniPlayHubStart").start();
        },this::showUnavailable);
    }
    private void attach(AssetServer server){
        mOpening=false;
        if(mClosed||mActivity.getTabModelSelector().isIncognitoSelected()){server.close();return;}
        mServer=server;mDocuments=new NativeDocuments(mActivity,mActivity.getWindowAndroid(),mCore);
        // Install the observer on about:blank before the trusted page is loaded.
        mTab=mActivity.getTabCreator(false).createNewTab(new LoadUrlParams("about:blank"),TabLaunchType.FROM_CHROME_UI,null);
        if(mTab==null||mTab.getWebContents()==null){close();showUnavailable();return;}
        mTab.addObserver(mTabObserver);
        mObserver=new WebContentsObserver(mTab.getWebContents()){
            @Override public void didStartNavigationInPrimaryMainFrame(NavigationHandle handle){if(!handle.isSameDocument())disconnect();}
            @Override public void primaryPageChanged(Page page){mMain.post(GameHubController.this::waitForBridge);}
            @Override public void didFinishLoadInPrimaryMainFrame(Page page,GlobalRenderFrameHostId frame,GURL url,boolean valid,int state){
                if(valid&&mServer!=null&&mServer.owns(url.getSpec()))waitForBridge();
            }
            @Override public void primaryMainFrameRenderProcessGone(int status){disconnect();mCore.suspend();}
            @Override public void webContentsDestroyed(){close();}
        };
        mTab.loadUrl(new LoadUrlParams(server.entryURL()));
    }
    private boolean owns(Channel channel){return !mClosed&&!channel.closed&&mChannel==channel&&mServer!=null&&!channel.contents.isDestroyed()&&mServer.owns(channel.contents.getLastCommittedUrl().getSpec());}
    private void waitForBridge(){
        if(mClosed||mChannel!=null||mTab==null||mTab.isDestroyed()||mServer==null)return;
        WebContents contents=mTab.getWebContents();if(contents==null||!mServer.owns(contents.getLastCommittedUrl().getSpec()))return;
        long attempt=++mHandshake;pollBridge(contents,attempt,android.os.SystemClock.elapsedRealtime()+25000);
    }
    private void pollBridge(WebContents contents,long attempt,long until){
        if(mClosed||attempt!=mHandshake||mChannel!=null||contents.isDestroyed()||mServer==null||!mServer.owns(contents.getLastCommittedUrl().getSpec()))return;
        // App startup has a top-level await for hello. Waiting for the load event
        // would deadlock it; poll only this constant readiness marker instead.
        contents.evaluateJavaScript("globalThis.__omniplayAndroidBridgeReady === true",value->{
            if(mClosed||attempt!=mHandshake||contents.isDestroyed())return;
            if("true".equals(value))connect();
            else if(android.os.SystemClock.elapsedRealtime()<until)mMain.postDelayed(()->pollBridge(contents,attempt,until),100);
        });
    }
    private void connect(){
        if(mClosed||mChannel!=null||mTab==null||mTab.isDestroyed()||mServer==null)return;
        WebContents contents=mTab.getWebContents();if(contents==null||!mServer.owns(contents.getLastCommittedUrl().getSpec()))return;
        disconnect();MessagePort[] ports=contents.createMessageChannel();Channel channel=new Channel(ports[0],contents);mChannel=channel;
        ports[0].setMessageCallback((message,sent)->{
            if(sent!=null){for(MessagePort unexpected:sent)unexpected.close();disconnect();return;}
            if(!owns(channel))return;
            try{receive(channel,message.getAsString());}catch(Exception error){disconnect();mCore.suspend();}
        },mMain);
        contents.postMessageToMainFrame(new MessagePayload("omniplay.android.channel.v1"),mServer.origin(),mServer.origin(),new MessagePort[]{ports[1]});
        resume();
    }
    private void receive(Channel channel,String raw)throws Exception{
        if(raw==null||raw.length()>1_048_576||raw.getBytes(StandardCharsets.UTF_8).length>1_048_576)throw new IllegalArgumentException();
        JSONObject request=new JSONObject(raw);
        if("lifecycle_flushed".equals(request.optString("event"))){if(mSuspension!=null&&mSuspension.equals(request.optString("suspension_id"))){mMain.removeCallbacks(mPauseFallback);mSuspension=null;mCore.suspend();}return;}
        String id=request.getString("id");if(!UUID.fromString(id).toString().equalsIgnoreCase(id))throw new IllegalArgumentException();
        long now=System.currentTimeMillis(),deadline=request.getLong("deadline_ms");
        if(!(request.get("deadline_ms") instanceof Number)||((Number)request.get("deadline_ms")).doubleValue()!=deadline||deadline<=now||deadline-now>120_000||request.getInt("protocol_major")!=1||request.getInt("protocol_minor")!=0||request.optJSONObject("command")==null)throw new IllegalArgumentException();
        if(channel.requests.size()>=16||!channel.requests.add(id)){send(channel,NativeCore.failure(id,"busy","Wait for the current native operations."));return;}
        final boolean[] finished={false};
        Runnable expiry=()->{channel.expiries.remove(id);if(!finished[0]){finished[0]=true;channel.requests.remove(id);send(channel,NativeCore.failure(id,"deadline_exceeded","The native operation deadline expired."));}};
        channel.expiries.put(id,expiry);
        mMain.postDelayed(expiry,deadline-now);
        NativeCore.Reply reply=response->{if(finished[0]||!owns(channel))return;finished[0]=true;mMain.removeCallbacks(expiry);channel.expiries.remove(id);channel.requests.remove(id);try{response.put("id",id);}catch(Exception ignored){}send(channel,response);};
        if(mDocuments.intercept(request,reply))return;
        mCore.request(request,()->!channel.closed,response->{if(!owns(channel)||finished[0])return;if(!mDocuments.finishExport(request,response,reply))reply.accept(response);});
    }
    private void send(Channel channel,JSONObject response){if(owns(channel))channel.port.postMessage(new MessagePayload(response.toString()),null);}
    private boolean identityUiAvailable(){
        android.app.KeyguardManager keyguard=(android.app.KeyguardManager)mActivity.getSystemService(android.content.Context.KEYGUARD_SERVICE);
        android.os.UserManager user=(android.os.UserManager)mActivity.getSystemService(android.content.Context.USER_SERVICE);
        return !mClosed&&mActivityResumed&&!mActivity.isFinishing()&&!mActivity.isDestroyed()&&mTab!=null&&!mTab.isDestroyed()
                &&mActivity.getActivityTab()==mTab&&!mTab.isIncognito()&&mChannel!=null&&owns(mChannel)
                &&keyguard!=null&&!keyguard.isDeviceLocked()&&user!=null&&user.isUserUnlocked();
    }
    private void inspectPairingStartup(){
        if(mPairingStartupChecked||mPairingStartupOwner!=null||mIdentityFlow!=null||mIdentityDialog!=null||!identityUiAvailable())return;
        mPairingStartupChecked=true;java.util.concurrent.atomic.AtomicBoolean owner=new java.util.concurrent.atomic.AtomicBoolean(true);mPairingStartupOwner=owner;
        mCore.pairingRecoveryStatus(owner::get,(snapshot,error)->{
            if(mPairingStartupOwner!=owner||!owner.get())return;mPairingStartupOwner=null;owner.set(false);
            if(!identityUiAvailable()||mIdentityFlow!=null||mIdentityDialog!=null)return;
            if(snapshot==null||android.os.SystemClock.elapsedRealtime()>=snapshot.expires)identityNotice("Remote Play status could not be inspected. Reopen the Game Hub menu and choose status and recovery. Local play remains available.");
            else if(snapshot.needsAttention())identityNotice("Remote Play setup or cleanup needs attention. Reopen the Game Hub menu and choose status and recovery.");
        });
    }
    /** Reachable only by reopening the native Chrome Game Hub menu item. */
    private void showNativeHubActions(){
        if(!identityUiAvailable()||mIdentityDialog!=null||mIdentityFlow!=null)return;
        AlertDialog menu=new AlertDialog.Builder(mActivity).setTitle("Vlikeon Game Hub")
                .setItems(new String[]{"Continue to library","Remote Play identity","Remote Play status and recovery"},(dialog,index)->{mIdentityDialog=null;if(index==1)beginIdentityReview();else if(index==2)beginPairingRecovery();})
                .setNegativeButton(android.R.string.cancel,(dialog,which)->mIdentityDialog=null).create();
        mIdentityDialog=menu;menu.setOnDismissListener(dialog->{if(mIdentityDialog==menu)mIdentityDialog=null;});menu.show();
    }
    private void beginIdentityReview(){
        if(!identityUiAvailable()||mIdentityFlow!=null)return;
        java.util.concurrent.atomic.AtomicBoolean owner=new java.util.concurrent.atomic.AtomicBoolean(true);mIdentityFlow=owner;
        mCore.reviewPairingIdentity(owner::get,(review,error)->{
            if(mIdentityFlow!=owner||!owner.get())return;
            if(!identityUiAvailable()){cancelIdentityReview();return;}
            if(review==null){cancelIdentityReview();identityNotice(error);return;}
            long remaining=review.expires-android.os.SystemClock.elapsedRealtime();
            if(remaining<=0){cancelIdentityReview();identityNotice("The identity review expired. Open its status again.");return;}
            AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Confirm Remote Play identity")
                    .setMessage("Activate this device's local identity?\n\nDevice: "+review.device+"\nGeneration: "+review.generation+"\nPublic key:\n"+review.key+"\n\nThis does not approve any remote device, connection or input control.")
                    .setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview())
                    .setPositiveButton("Activate local identity",(dialog,which)->{
                        mIdentityDialog=null;
                        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=review.expires){cancelIdentityReview();return;}
                        mCore.confirmPairingIdentity(review,owner::get,response->{
                            if(mIdentityFlow!=owner||!owner.get())return;
                            cancelIdentityReview();
                            if(!identityUiAvailable())return;
                            identityNotice("ok".equals(response.optString("status"))?"Local identity activated. No remote device has been approved.":"Activation could not be confirmed. Open identity status again before continuing.");
                        });
                    }).create();
            mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());
            mIdentityExpiry=()->{if(mIdentityFlow==owner)cancelIdentityReview();};mMain.postDelayed(mIdentityExpiry,remaining);alert.show();
        });
    }
    private void beginPairingRecovery(){
        if(!identityUiAvailable()||mIdentityFlow!=null)return;
        java.util.concurrent.atomic.AtomicBoolean owner=new java.util.concurrent.atomic.AtomicBoolean(true);mIdentityFlow=owner;
        mCore.pairingRecoveryStatus(owner::get,(snapshot,error)->{
            if(mIdentityFlow!=owner||!owner.get())return;
            if(!identityUiAvailable()){cancelIdentityReview();return;}
            if(snapshot==null){cancelIdentityReview();identityNotice(error);return;}
            long remaining=snapshot.expires-android.os.SystemClock.elapsedRealtime();
            if(remaining<=0){cancelIdentityReview();identityNotice("Recovery status expired. Open it again.");return;}
            if(snapshot.records.isEmpty()){cancelIdentityReview();identityNotice("This device has no Remote Play identity or pair records.");return;}
            String[] titles=new String[snapshot.records.size()];
            for(int i=0;i<titles.length;i++){NativeCore.RecoveryRecord record=snapshot.records.get(i);titles[i]=(record.isIdentity()?"Identity · ":"Device pair · ")+record.generation;}
            AlertDialog list=new AlertDialog.Builder(mActivity).setTitle("Remote Play status")
                    .setItems(titles,(dialog,index)->{mIdentityDialog=null;showPairingRecovery(snapshot,snapshot.records.get(index),owner);})
                    .setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview()).create();
            mIdentityDialog=list;list.setOnCancelListener(dialog->cancelIdentityReview());
            mIdentityExpiry=()->{if(mIdentityFlow==owner)cancelIdentityReview();};mMain.postDelayed(mIdentityExpiry,remaining);list.show();
        });
    }
    private static String recoveryPhase(String phase){
        switch(phase){case "reserved":return "Reserved";case "writing":return "Key write not confirmed";case "material_ready":return "Awaiting identity approval";case "active":return "Active";case "revoking":return "Being revoked";case "retired":return "Cleanup complete";default:return "Unknown";}
    }
    private static String recoveryStep(String step){
        if(step==null)return "No cleanup step remains.";
        switch(step){
            case "fence_generation":return "Disable this generation before recovery.";
            case "revalidate_before_use":return "Live authorization must be checked before use.";
            case "retire_writer":return "Waiting for confirmation that the key writer has stopped.";
            case "retire_local_sessions":return "Waiting for confirmation that local sessions have stopped.";
            case "wait_for_pair_cleanup":return "Waiting for device pair cleanup.";
            case "revoke_relay":return "Waiting for relay revocation confirmation.";
            case "remove_material":return "Waiting for key removal and vault verification.";
            case "finalize_retirement":return "Finish cleanup.";
            default:return "Recovery status must be reloaded.";
        }
    }
    private void showPairingRecovery(NativeCore.RecoverySnapshot snapshot,NativeCore.RecoveryRecord record,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=snapshot.expires){cancelIdentityReview();return;}
        String message="Device: "+record.device+"\nGeneration: "+record.generation+"\nStatus: "+recoveryPhase(record.phase)+"\nPublic key:\n"+record.key+"\n\n"+recoveryStep(record.nextStep);
        if("fence_generation".equals(record.nextStep))message+="\n\nDisabling this generation is permanent. Disabling an identity also disables its pairs. This does not confirm that connections stopped or keys were deleted.";
        if("remove_material".equals(record.nextStep))message+="\n\nPermanently delete only this generation's key from the device vault. This action cannot restore it. Removal is allowed only after all stop/revocation confirmations already exist.";
        if("revoke_relay".equals(record.nextStep))message+=snapshot.relayRetirementAvailable?"\n\nRequest revocation of this pair from the configured connection service. Only the exact pair acknowledgment completes this step; device keys are not deleted.":"\n\nThis version has no configured connection service for completing revocation.";
        if(record.sessionRecoveryRequired)message+="\n\nAn earlier connection has not been confirmed stopped. Complete recovery before starting another session. Restarting alone does not clear it.";
        if(snapshot.namespaceProvenanceMissing)message+="\n\nThis profile lacks verified credential storage provenance. Reviewed profile recovery is required before changing keys or lifecycle state.";
        AlertDialog.Builder builder=new AlertDialog.Builder(mActivity).setTitle("Remote Play record").setMessage(message)
                .setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview());
        String actionTitle="revoke_relay".equals(record.nextStep)?"Revoke on service":("remove_material".equals(record.nextStep)?"Delete this generation's key":("fence_generation".equals(record.nextStep)?"Disable this generation":"Finish cleanup"));
        if(!snapshot.namespaceProvenanceMissing&&record.canAdvance()&&(!"revoke_relay".equals(record.nextStep)||snapshot.relayRetirementAvailable))builder.setPositiveButton(actionTitle,(dialog,which)->{
            mIdentityDialog=null;
            if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=snapshot.expires){cancelIdentityReview();return;}
            mCore.advancePairingRecovery(snapshot,record,owner::get,response->{
                if(mIdentityFlow!=owner||!owner.get())return;cancelIdentityReview();
                if(!identityUiAvailable())return;
                identityNotice("ok".equals(response.optString("status"))?"Open status and recovery again to inspect the current result and next step.":"The result could not be confirmed. Open status and recovery again before continuing.");
            });
        });
        AlertDialog alert=builder.create();mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
    }
    private void cancelIdentityReview(){
        boolean hadFlow=mIdentityFlow!=null;
        if(hadFlow){mIdentityFlow.set(false);mIdentityFlow=null;}
        if(mIdentityExpiry!=null){mMain.removeCallbacks(mIdentityExpiry);mIdentityExpiry=null;}
        if(mIdentityDialog!=null){AlertDialog dialog=mIdentityDialog;mIdentityDialog=null;dialog.dismiss();}
        if(hadFlow)mCore.cancelPairingIdentityReview();
    }
    private void identityNotice(String message){
        if(!identityUiAvailable()||mIdentityDialog!=null)return;
        AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Remote Play").setMessage(message).setPositiveButton(android.R.string.ok,null).create();
        mIdentityDialog=alert;alert.setOnDismissListener(dialog->{if(mIdentityDialog==alert)mIdentityDialog=null;});alert.show();
    }
    public void suspend(){
        if(mPairingStartupOwner!=null){mPairingStartupOwner.set(false);mPairingStartupOwner=null;mPairingStartupChecked=false;}
        cancelIdentityReview();
        if(mClosed)return;
        mCore.setForeground(false);
        if(mChannel!=null&&owns(mChannel)){
            if(mSuspension!=null)return;
            mSuspension=UUID.randomUUID().toString();
            try{send(mChannel,new JSONObject().put("event","native_suspend").put("suspension_id",mSuspension));}catch(Exception ignored){}
            mMain.postDelayed(mPauseFallback,15000);
        }else mCore.suspend();
    }
    private void disconnect(){
        if(mPairingStartupOwner!=null){mPairingStartupOwner.set(false);mPairingStartupOwner=null;mPairingStartupChecked=false;}
        cancelIdentityReview();
        mCore.setForeground(false);
        ++mHandshake;
        Channel previous=mChannel;mChannel=null;
        if(previous!=null){previous.closed=true;previous.port.close();for(Runnable expiry:previous.expiries.values())mMain.removeCallbacks(expiry);previous.expiries.clear();previous.requests.clear();mCore.detach();}
        if(mDocuments!=null){mDocuments.close();mDocuments=null;}
        if(!mClosed&&mServer!=null)mDocuments=new NativeDocuments(mActivity,mActivity.getWindowAndroid(),mCore);
        mMain.removeCallbacks(mPauseFallback);mSuspension=null;
    }
    private void showUnavailable(){mOpening=false;if(!mActivity.isFinishing()&&!mClosed)new AlertDialog.Builder(mActivity).setTitle("OmniPlay").setMessage("Game Hub could not open. Check available storage and the installed application components, then retry.").setPositiveButton(android.R.string.ok,null).show();}
    @Override public void close(){if(mClosed)return;mClosed=true;disconnect();mCore.suspend();if(mObserver!=null){mObserver.observe(null);mObserver=null;}if(mTab!=null){mTab.removeObserver(mTabObserver);mTab=null;}if(mServer!=null){mServer.close();mServer=null;}if(sActive==this)sActive=null;}
}
