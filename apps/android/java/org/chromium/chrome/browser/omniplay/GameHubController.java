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
                .setItems(new String[]{"Continue to library","Remote Play identity","Remote Play status and recovery","Pair with a remote host"},(dialog,index)->{mIdentityDialog=null;if(index==1)beginIdentityReview();else if(index==2)beginPairingRecovery();else if(index==3)beginClientPairing();})
                .setNegativeButton(android.R.string.cancel,(dialog,which)->mIdentityDialog=null).create();
        mIdentityDialog=menu;menu.setOnDismissListener(dialog->{if(mIdentityDialog==menu)mIdentityDialog=null;});menu.show();
    }
    private void beginClientPairing(){
        if(!identityUiAvailable()||mIdentityFlow!=null)return;
        java.util.concurrent.atomic.AtomicBoolean owner=new java.util.concurrent.atomic.AtomicBoolean(true);mIdentityFlow=owner;
        mCore.clientPairingSetup(owner::get,(setup,error)->{
            if(mIdentityFlow!=owner||!owner.get())return;
            if(!identityUiAvailable()||setup==null){cancelIdentityReview();if(identityUiAvailable())identityNotice(error);return;}
            android.widget.LinearLayout form=new android.widget.LinearLayout(mActivity);form.setOrientation(android.widget.LinearLayout.VERTICAL);
            android.widget.EditText code=new android.widget.EditText(mActivity),host=new android.widget.EditText(mActivity),label=new android.widget.EditText(mActivity);
            code.setHint("Invitation code");code.setSingleLine(true);code.setInputType(android.text.InputType.TYPE_CLASS_TEXT|android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD);
            code.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(8192)});code.setSaveEnabled(false);
            host.setHint("Independently verified host public key");host.setSingleLine(true);host.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(43)});host.setSaveEnabled(false);
            label.setHint("This device's label");label.setSingleLine(true);label.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(64)});label.setSaveEnabled(false);
            if(android.os.Build.VERSION.SDK_INT>=26){code.setImportantForAutofill(android.view.View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);host.setImportantForAutofill(android.view.View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);}
            android.widget.Spinner relay=new android.widget.Spinner(mActivity);
            relay.setAdapter(new android.widget.ArrayAdapter<>(mActivity,android.R.layout.simple_spinner_dropdown_item,setup.relayOrigins));
            android.widget.CheckBox verified=new android.widget.CheckBox(mActivity);verified.setText("I verified the host key independently of the invitation code.");
            form.addView(code);form.addView(host);form.addView(label);form.addView(relay);form.addView(verified);
            AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Pair with a remote host")
                    .setMessage("Use a host invitation and its independently verified public key. Setup expires in 30 seconds; reopen if needed.")
                    .setView(form).setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview()).setPositiveButton("Prepare pairing",null).create();
            mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());
            alert.setOnDismissListener(dialog->code.setText(""));alert.show();
            alert.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(button->{
                if(!verified.isChecked()||!host.getText().toString().matches("[A-Za-z0-9_-]{42}[AEIMQUYcgkosw048]")||code.length()==0||label.length()==0){identityFormError(alert);return;}
                if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=setup.expires){cancelIdentityReview();return;}
                String invitation=code.getText().toString(),hostKey=host.getText().toString(),deviceLabel=label.getText().toString(),origin=(String)relay.getSelectedItem();
                code.setText("");alert.dismiss();mIdentityDialog=null;
                if(mIdentityExpiry!=null){mMain.removeCallbacks(mIdentityExpiry);mIdentityExpiry=null;}
                mCore.beginPairingClient(setup,invitation,hostKey,origin,deviceLabel,owner::get,response->{
                    if(mIdentityFlow!=owner||!owner.get())return;
                    if(!identityUiAvailable()){cancelIdentityReview();return;}
                    try{
                        if(!"ok".equals(response.getString("status")))throw new IllegalStateException();
                        JSONObject value=response.getJSONObject("result");String ticket=value.getString("ticket");
                        long remaining=Math.min(300_000,value.getLong("expires_at_ms")-System.currentTimeMillis());
                        if(remaining<=0)throw new IllegalStateException();
                        mIdentityExpiry=()->{if(mIdentityFlow==owner)cancelIdentityReview();};mMain.postDelayed(mIdentityExpiry,remaining);
                        showClientJoin(ticket,value.getString("comparison_code"),owner);
                    }catch(Exception failure){cancelIdentityReview();identityNotice("Pairing preparation was not confirmed. Inspect status before retrying.");}
                });
            });
            mIdentityExpiry=()->{if(mIdentityFlow==owner)cancelIdentityReview();};mMain.postDelayed(mIdentityExpiry,Math.max(0,setup.expires-android.os.SystemClock.elapsedRealtime()));
        });
    }
    private void identityFormError(AlertDialog alert){alert.setMessage("Enter an invitation, device label and canonical host key, then confirm independent host verification.");}
    private void showClientJoin(String ticket,String comparison,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
        AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Send pairing request?")
                .setMessage("Comparison code: "+comparison+"\n\nSend this request to the configured service, then compare this code on the host. Sending does not approve the peer or input control.")
                .setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview())
                .setPositiveButton("Send request",(dialog,which)->{
                    mIdentityDialog=null;
                    if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
                    try{
                        byte[] request=new JSONObject().put("ticket",ticket).toString().getBytes(StandardCharsets.UTF_8);
                        mCore.pairingClient(request,1,owner::get,response->{
                            if(mIdentityFlow!=owner||!owner.get())return;
                            if("ok".equals(response.optString("status"))&&identityUiAvailable())showClientWaiting(ticket,owner);
                            else{cancelIdentityReview();identityNotice("Request result is uncertain. Inspect status before continuing.");}
                        });
                    }catch(Exception error){cancelIdentityReview();identityNotice("Request could not be sent. Inspect status.");}
                }).create();
        mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
    }
    private void showClientWaiting(String ticket,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
        AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Waiting for host approval")
                .setMessage("No current approval has been confirmed here. Approve on the host after comparing codes, then check again, or cancel locally. Checks run only when you request them.")
                .setNegativeButton("Cancel locally",(dialog,which)->{mIdentityDialog=null;cancelPendingPeer(ticket,owner);})
                .setPositiveButton("Check approval",(dialog,which)->{mIdentityDialog=null;showPeerApproval(ticket,owner);}).create();
        mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
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
            if(snapshot.records.isEmpty()&&snapshot.pendingClientTicket==null&&snapshot.pendingHostTicket==null){cancelIdentityReview();identityNotice("This device has no Remote Play identity or pair records.");return;}
            String[] titles=new String[snapshot.records.size()+(snapshot.pendingClientTicket==null?0:2)+(snapshot.pendingHostTicket==null?0:1)];
            for(int i=0;i<snapshot.records.size();i++){NativeCore.RecoveryRecord record=snapshot.records.get(i);titles[i]=(record.isIdentity()?"Identity · ":"Device pair · ")+record.generation;}
            if(snapshot.pendingClientTicket!=null){titles[snapshot.records.size()]="Review pending host approval";titles[snapshot.records.size()+1]="Cancel pending pairing locally";}
            if(snapshot.pendingHostTicket!=null)titles[titles.length-1]="Close this device's host invitation";
            AlertDialog list=new AlertDialog.Builder(mActivity).setTitle("Remote Play status")
                    .setItems(titles,(dialog,index)->{
                        mIdentityDialog=null;
                        if(index<snapshot.records.size())showPairingRecovery(snapshot,snapshot.records.get(index),owner);
                        else if(snapshot.pendingClientTicket!=null&&index==snapshot.records.size())showPeerApproval(snapshot.pendingClientTicket,owner);
                        else if(snapshot.pendingClientTicket!=null&&index==snapshot.records.size()+1)cancelPendingPeer(snapshot.pendingClientTicket,owner);
                        else closeHostInvitation(snapshot.pendingHostTicket,owner);
                    })
                    .setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview()).create();
            mIdentityDialog=list;list.setOnCancelListener(dialog->cancelIdentityReview());
            mIdentityExpiry=()->{if(mIdentityFlow==owner)cancelIdentityReview();};mMain.postDelayed(mIdentityExpiry,remaining);list.show();
        });
    }
    private void closeHostInvitation(String ticket,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
        AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Close host invitation?")
                .setMessage("Stop sharing and approving this invitation on this device, then request its closure on the configured service if registration was attempted. Existing pairs are not revoked. A failed result requires checking status again.")
                .setNegativeButton("Later",(dialog,which)->cancelIdentityReview())
                .setPositiveButton("Close invitation",(dialog,which)->{
                    mIdentityDialog=null;
                    if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
                    mCore.pairingHostTicket(ticket,3,owner::get,response->{
                        if(mIdentityFlow!=owner||!owner.get())return;cancelIdentityReview();
                        if(identityUiAvailable())identityNotice("ok".equals(response.optString("status"))?"Host invitation closed. Existing pairs were not revoked.":"Closure could not be confirmed. Reopen status before continuing.");
                    });
                }).create();
        mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
    }
    private void cancelPendingPeer(String ticket,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
        AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Cancel pending pairing?")
                .setMessage("Discard this device's pending exchange. This does not revoke a pair already approved on the connection service or delete a saved pairing.")
                .setNegativeButton("Close",(dialog,which)->cancelIdentityReview())
                .setPositiveButton("Cancel locally",(dialog,which)->{
                    mIdentityDialog=null;
                    if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
                    try{
                        byte[] request=new JSONObject().put("ticket",ticket).toString().getBytes(StandardCharsets.UTF_8);
                        mCore.pairingClient(request,4,owner::get,response->{
                            if(mIdentityFlow!=owner||!owner.get())return;cancelIdentityReview();
                            if(identityUiAvailable())identityNotice("ok".equals(response.optString("status"))?"Pending pairing cancelled locally. Remote approvals were not revoked.":"Cancellation could not be confirmed. Reopen status before continuing.");
                        });
                    }catch(Exception error){cancelIdentityReview();identityNotice("Cancellation could not be queued. Reopen status.");}
                }).create();
        mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
    }
    private void showPeerApproval(String ticket,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
        mCore.reviewPairingPeer(ticket,owner::get,(review,error)->{
            if(mIdentityFlow!=owner||!owner.get())return;
            if(!identityUiAvailable()){cancelIdentityReview();return;}
            if(review==null){showClientWaiting(ticket,owner);return;}
            if(mIdentityExpiry!=null)mMain.removeCallbacks(mIdentityExpiry);
            long remaining=review.expires-android.os.SystemClock.elapsedRealtime();
            if(remaining<=0){cancelIdentityReview();return;}
            android.widget.LinearLayout checks=new android.widget.LinearLayout(mActivity);checks.setOrientation(android.widget.LinearLayout.VERTICAL);
            android.widget.CheckBox peer=new android.widget.CheckBox(mActivity),permission=new android.widget.CheckBox(mActivity);
            peer.setText("I independently verified this host and matched the comparison code on both devices.");
            permission.setText(review.inputAllowed?"I approve the input-control permission shown for this pair.":"I approve this pair without input-control permission.");
            checks.addView(peer);checks.addView(permission);
            AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Confirm Remote Play peer")
                    .setMessage("Host key:\n"+review.hostPublicKey+"\nFingerprint:\n"+review.hostFingerprint+"\nComparison code: "+review.comparisonCode+"\nPair: "+review.pairId+"\nInput control: "+(review.inputAllowed?"Allowed":"Not allowed")+"\nPermission expires: "+java.text.DateFormat.getDateTimeInstance().format(new java.util.Date(review.grantExpiresAt))+"\n\nSave this verified pairing. Activation is a separate step. This review closes within one minute.")
                    .setView(checks).setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview())
                    .setPositiveButton("Save verified pairing",null).create();
            mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
            android.widget.Button confirm=alert.getButton(AlertDialog.BUTTON_POSITIVE);confirm.setEnabled(false);
            android.widget.CompoundButton.OnCheckedChangeListener changed=(button,checked)->confirm.setEnabled(peer.isChecked()&&permission.isChecked());
            peer.setOnCheckedChangeListener(changed);permission.setOnCheckedChangeListener(changed);
            confirm.setOnClickListener(button->{
                if(!peer.isChecked()||!permission.isChecked()||mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=review.expires){cancelIdentityReview();return;}
                confirm.setEnabled(false);alert.dismiss();mIdentityDialog=null;
                mCore.confirmPairingPeer(review,owner::get,response->{
                    if(mIdentityFlow!=owner||!owner.get())return;
                    if(!identityUiAvailable()){cancelIdentityReview();return;}
                    try{showPeerActivation(mCore.activationForPeer(review,response),owner);}
                    catch(Exception error){cancelIdentityReview();identityNotice("Pairing result is uncertain or its review expired. Open status before continuing.");}
                });
            });
            mIdentityExpiry=()->{if(mIdentityFlow==owner)cancelIdentityReview();};mMain.postDelayed(mIdentityExpiry,remaining);
        });
    }
    private void showPeerActivation(NativeCore.PeerActivation activation,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=activation.expires){cancelIdentityReview();return;}
        NativeCore.PeerApproval peer=activation.peer;
        AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Activate verified pairing?")
                .setMessage("Host: "+peer.hostFingerprint+"\nPair: "+peer.pairId+"\nGeneration: "+activation.generation+"\nInput control: "+(peer.inputAllowed?"Allowed":"Not allowed")+"\nPermission expires: "+java.text.DateFormat.getDateTimeInstance().format(new java.util.Date(peer.grantExpiresAt))+"\n\nConfirm current service approval and activate this saved pairing. This does not start a connection.")
                .setNegativeButton("Later",(dialog,which)->cancelIdentityReview())
                .setPositiveButton("Activate pairing",(dialog,which)->{
                    mIdentityDialog=null;
                    if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
                    mCore.activatePairingPeer(activation,owner::get,response->{
                        if(mIdentityFlow!=owner||!owner.get())return;cancelIdentityReview();
                        if(identityUiAvailable())identityNotice("ok".equals(response.optString("status"))?"Pairing activated. No connection was started. Open status to inspect it.":"Activation could not be confirmed. Inspect status before retrying.");
                    });
                }).create();
        mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
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
            case "wait_for_pair_cleanup":return "Waiting for device pair or host invitation cleanup.";
            case "revoke_relay":return "Waiting for relay revocation confirmation.";
            case "remove_material":return "Waiting for key removal and vault verification.";
            case "finalize_retirement":return "Finish cleanup.";
            default:return "Recovery status must be reloaded.";
        }
    }
    private void showPairingRecovery(NativeCore.RecoverySnapshot snapshot,NativeCore.RecoveryRecord record,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=snapshot.expires){cancelIdentityReview();return;}
        String message="Device: "+record.device+"\nGeneration: "+record.generation+"\nStatus: "+recoveryPhase(record.phase)+"\nPublic key:\n"+record.key+"\n\n"+recoveryStep(record.nextStep);
        if("active".equals(record.phase)||"fence_generation".equals(record.nextStep))message+="\n\nDisabling this generation is permanent. Disabling an identity also disables its pairs. This does not confirm that connections stopped or keys were deleted.";
        if("remove_material".equals(record.nextStep))message+="\n\nPermanently delete only this generation's key from the device vault. This action cannot restore it. Removal is allowed only after all stop/revocation confirmations already exist.";
        if("revoke_relay".equals(record.nextStep))message+=snapshot.relayRetirementAvailable?"\n\nRequest revocation of this pair from the configured connection service. Only the exact pair acknowledgment completes this step; device keys are not deleted.":"\n\nThis version has no configured connection service for completing revocation.";
        if(record.sessionRecoveryRequired)message+="\n\nAn earlier connection has not been confirmed stopped. Complete recovery before starting another session. Restarting alone does not clear it.";
        if(snapshot.namespaceProvenanceMissing)message+="\n\nThis profile lacks verified credential storage provenance. Reviewed profile recovery is required before changing keys or lifecycle state.";
        AlertDialog.Builder builder=new AlertDialog.Builder(mActivity).setTitle("Remote Play record").setMessage(message)
                .setNegativeButton(android.R.string.cancel,(dialog,which)->cancelIdentityReview());
        String actionTitle="active".equals(record.phase)?"Disable this generation":("revoke_relay".equals(record.nextStep)?"Revoke on service":("remove_material".equals(record.nextStep)?"Delete this generation's key":("fence_generation".equals(record.nextStep)?"Disable this generation":"Finish cleanup")));
        if(!snapshot.namespaceProvenanceMissing&&record.canAdvance()&&(!"revoke_relay".equals(record.nextStep)||snapshot.relayRetirementAvailable))builder.setPositiveButton(actionTitle,(dialog,which)->{
            mIdentityDialog=null;
            if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=snapshot.expires){cancelIdentityReview();return;}
            mCore.advancePairingRecovery(snapshot,record,owner::get,response->{
                if(mIdentityFlow!=owner||!owner.get())return;cancelIdentityReview();
                if(!identityUiAvailable())return;
                identityNotice("ok".equals(response.optString("status"))?"Open status and recovery again to inspect the current result and next step.":"The result could not be confirmed. Open status and recovery again before continuing.");
            });
        });
        if(!snapshot.namespaceProvenanceMissing&&!record.isIdentity()&&"material_ready".equals(record.phase)&&!record.sessionRecoveryRequired){
            builder.setNeutralButton("Review saved pairing",(dialog,which)->{mIdentityDialog=null;reviewSavedPeer(snapshot,record,owner);});
        }
        AlertDialog alert=builder.create();mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
    }
    private void reviewSavedPeer(NativeCore.RecoverySnapshot snapshot,NativeCore.RecoveryRecord record,java.util.concurrent.atomic.AtomicBoolean owner){
        if(mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()){cancelIdentityReview();return;}
        mCore.reviewSavedPeer(snapshot,record,owner::get,(review,error)->{
            if(mIdentityFlow!=owner||!owner.get())return;
            if(!identityUiAvailable()||review==null){cancelIdentityReview();if(identityUiAvailable())identityNotice(error);return;}
            if(mIdentityExpiry!=null)mMain.removeCallbacks(mIdentityExpiry);
            long remaining=review.expires-android.os.SystemClock.elapsedRealtime();
            if(remaining<=0){cancelIdentityReview();return;}
            android.widget.LinearLayout checks=new android.widget.LinearLayout(mActivity);checks.setOrientation(android.widget.LinearLayout.VERTICAL);
            android.widget.CheckBox host=new android.widget.CheckBox(mActivity),permission=new android.widget.CheckBox(mActivity);
            host.setText("I independently verified this host key again.");
            permission.setText(review.inputAllowed?"I approve input control for this saved pairing.":"I approve this saved pairing without input control.");checks.addView(host);checks.addView(permission);
            AlertDialog alert=new AlertDialog.Builder(mActivity).setTitle("Review saved pairing")
                    .setMessage("Host key:\n"+review.hostPublicKey+"\nFingerprint:\n"+review.hostFingerprint+"\nService: "+review.audience+"\nPair: "+review.pairId+"\nGeneration: "+review.generation+"\nInput control: "+(review.inputAllowed?"Allowed":"Not allowed")+"\nPermission expires: "+java.text.DateFormat.getDateTimeInstance().format(new java.util.Date(review.grantExpiresAt))+"\n\nThe original comparison code is unavailable. Verify this host independently again. Activation checks current service approval and does not start a connection.")
                    .setView(checks).setNegativeButton("Later",(dialog,which)->cancelIdentityReview()).setPositiveButton("Activate verified pairing",null).create();
            mIdentityDialog=alert;alert.setOnCancelListener(dialog->cancelIdentityReview());alert.show();
            android.widget.Button confirm=alert.getButton(AlertDialog.BUTTON_POSITIVE);confirm.setEnabled(false);
            android.widget.CompoundButton.OnCheckedChangeListener changed=(button,checked)->confirm.setEnabled(host.isChecked()&&permission.isChecked());host.setOnCheckedChangeListener(changed);permission.setOnCheckedChangeListener(changed);
            confirm.setOnClickListener(button->{
                if(!host.isChecked()||!permission.isChecked()||mIdentityFlow!=owner||!owner.get()||!identityUiAvailable()||android.os.SystemClock.elapsedRealtime()>=review.expires){cancelIdentityReview();return;}
                confirm.setEnabled(false);alert.dismiss();mIdentityDialog=null;
                mCore.activateSavedPeer(review,owner::get,response->{
                    if(mIdentityFlow!=owner||!owner.get())return;cancelIdentityReview();
                    if(identityUiAvailable())identityNotice("ok".equals(response.optString("status"))?"Saved pairing activated. No connection was started. Open status to inspect it.":"Activation could not be confirmed. Read status before retrying.");
                });
            });
            mIdentityExpiry=()->{if(mIdentityFlow==owner)cancelIdentityReview();};mMain.postDelayed(mIdentityExpiry,remaining);
        });
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
