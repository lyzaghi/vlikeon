package org.chromium.chrome.browser.omniplay;

import android.app.Activity;
import android.content.ContentResolver;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.CancellationSignal;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import org.chromium.ui.base.WindowAndroid;
import org.json.JSONObject;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** SAF-only access. Single files use private staging; disc references stream through a scoped native tree. */
public final class NativeDocuments implements AutoCloseable {
    private static final class Export {
        final File file; final long expiry;
        Export(File value) { file=value;expiry=System.currentTimeMillis()+300_000; }
    }
    private final Activity mActivity;
    private final WindowAndroid mWindow;
    private final NativeCore mCore;
    private final Handler mMain=new Handler(Looper.getMainLooper());
    private final ExecutorService mIo=Executors.newSingleThreadExecutor();
    private final Map<String,Export> mExports=new HashMap<>();
    private final java.util.Set<SelectedDiscTree> mTrees=java.util.concurrent.ConcurrentHashMap.newKeySet();
    private volatile CancellationSignal mActiveCancellation;
    private volatile boolean mClosed;
    private boolean mSelecting;
    private boolean mCopying;
    private int mPicker=WindowAndroid.START_INTENT_FAILURE;
    public NativeDocuments(Activity activity,WindowAndroid window,NativeCore core) {mActivity=activity;mWindow=window;mCore=core;}
    private static JSONObject fail(JSONObject request,String code,String message){return NativeCore.failure(request.optString("id"),code,message);}
    private boolean alive(JSONObject request){return !mClosed&&request.optLong("deadline_ms")>System.currentTimeMillis();}
    public boolean intercept(JSONObject request,NativeCore.Reply reply) {
        prune();JSONObject command=request.optJSONObject("command");if(command==null)return false;
        JSONObject params=command.optJSONObject("params");String method=command.optString("method");
        if(method.equals("file_select_import")||method.equals("firmware_import_selected")) {selectImport(request,params,method.equals("firmware_import_selected"),reply);return true;}
        if(method.equals("disc_select_folder")) {selectDisc(request,reply);return true;}
        if(method.equals("file_select_export")) {createExport(request,params,reply);return true;}
        return false;
    }
    private void selectDisc(JSONObject request,NativeCore.Reply reply){
        if(!SelectedDiscTree.supported()){reply.accept(fail(request,"unsupported_capability","Disc folder import requires Android 11 or newer."));return;}
        if(mSelecting||mCopying||!alive(request)){reply.accept(fail(request,"busy","Finish the current document selection first."));return;}
        Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        mSelecting=true;
        mPicker=mWindow.showCancelableIntent(intent,(code,data)->{
            mSelecting=false;mPicker=WindowAndroid.START_INTENT_FAILURE;
            if(code!=Activity.RESULT_OK||data==null||data.getData()==null||!alive(request)){reply.accept(fail(request,"cancelled","Disc folder selection was cancelled or expired."));return;}
            Uri treeUri=data.getData();
            if(!"content".equals(treeUri.getScheme())||(data.getFlags()&Intent.FLAG_GRANT_READ_URI_PERMISSION)==0){reply.accept(fail(request,"permission_denied","Choose a readable folder using the system picker."));return;}
            mCopying=true;
            mCore.storage(root->{
                if(!alive(request)){mCopying=false;reply.accept(fail(request,"cancelled","Disc folder selection expired."));return;}
                SelectedDiscTree tree=null;
                try{
                    tree=new SelectedDiscTree(mActivity.getApplicationContext().getContentResolver(),mCore.profileForDocuments(),treeUri,request.optLong("deadline_ms"),mTrees::remove);mTrees.add(tree);
                    final SelectedDiscTree selected=tree;
                    mIo.execute(()->prepareDisc(selected,request,reply));
                }catch(Exception error){if(tree!=null)tree.abandon();mCopying=false;reply.accept(fail(request,"storage_failure","The selected folder is unavailable. Finish pending imports or choose it again."));}
            },()->{mCopying=false;reply.accept(fail(request,"storage_failure","Local application storage is unavailable."));});
        },null);
        if(mPicker==WindowAndroid.START_INTENT_FAILURE){mSelecting=false;reply.accept(fail(request,"runtime_unavailable","No document folder picker is available."));}
    }
    private void prepareDisc(SelectedDiscTree tree,JSONObject request,NativeCore.Reply reply){
        try{
            if(!alive(request))throw new IllegalStateException();JSONObject metadata=tree.prepare();
            if(!alive(request))throw new IllegalStateException();
            mCore.work(()->{if(!alive(request)){tree.abandon();return fail(request,"cancelled","Disc folder selection expired.");}return mCore.authorizeMedia(metadata,tree);},value->{
                mCopying=false;
                if(!alive(request)){tree.abandon();reply.accept(fail(request,"cancelled","Disc folder selection expired."));return;}
                if(!"ok".equals(value.optString("status"))||value.optJSONObject("result")==null){tree.abandon();reply.accept(fail(request,"storage_failure","The folder could not be authorized. Choose the folder again."));return;}
                reply.accept(NativeCore.success(request,value.optJSONObject("result")));
            });
        }catch(Exception error){tree.abandon();mMain.post(()->{mCopying=false;reply.accept(fail(request,"storage_failure","Choose a readable folder containing at most 64 CUE, M3U, CHD, ISO or PBP entry points. Ambiguous names, loading providers or oversized directories cannot be imported."));});}
    }
    public boolean finishExport(JSONObject request,JSONObject response,NativeCore.Reply reply) {
        JSONObject command=request.optJSONObject("command");if(command==null)return false;
        String method=command.optString("method");
        if(!("save_export".equals(method)||"save_export_upload_commit".equals(method)||"diagnostics_export".equals(method)||"performance_run_export".equals(method))||!"ok".equals(response.optString("status")))return false;
        JSONObject params=command.optJSONObject("params");Export export=params==null?null:mExports.remove(params.optString("destination_grant_id"));
        if(export==null){reply.accept(fail(request,"permission_denied","The native export selection expired."));return true;}
        if(!alive(request)||mSelecting||mCopying){cleanup(export.file);reply.accept(fail(request,"busy","Finish the current document selection, then export again."));return true;}
        Intent intent=new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("application/octet-stream").putExtra(Intent.EXTRA_TITLE,export.file.getName());
        mSelecting=true;
        mPicker=mWindow.showCancelableIntent(intent,(code,data)->{
            mSelecting=false;mPicker=WindowAndroid.START_INTENT_FAILURE;
            if(code!=Activity.RESULT_OK||data==null||data.getData()==null||!alive(request)){cleanup(export.file);reply.accept(fail(request,"cancelled","Export was cancelled or its deadline expired."));return;}
            final Uri uri=data.getData();
            mCopying=true;
            mIo.execute(()->{
                boolean complete=false;
                CancellationSignal cancellation=new CancellationSignal();mActiveCancellation=cancellation;
                Runnable cancel=()->cancellation.cancel();mMain.postDelayed(cancel,Math.max(1,request.optLong("deadline_ms")-System.currentTimeMillis()));
                try {
                    if(!alive(request)||!"content".equals(uri.getScheme()))throw new IllegalArgumentException();
                    ContentResolver resolver=mActivity.getContentResolver();
                    try(ParcelFileDescriptor descriptor=resolver.openFileDescriptor(uri,"wt",cancellation)){
                        if(descriptor==null)throw new IllegalStateException();
                        try(FileInputStream input=new FileInputStream(export.file);FileOutputStream output=new ParcelFileDescriptor.AutoCloseOutputStream(descriptor)){
                            copy(input,output,300L*1024*1024,request,null);output.flush();
                        }
                    }
                    complete=true;
                }catch(Exception ignored){}finally{cleanup(export.file);mMain.removeCallbacks(cancel);mActiveCancellation=null;}
                final boolean saved=complete;mMain.post(()->{mCopying=false;if(saved){try{response.getJSONObject("result").put("delivery","document_provider");}catch(Exception ignored){}}reply.accept(saved?response:fail(request,"storage_failure","The selected destination could not be completely written. Retry the export."));});
            });
        },null);
        if(mPicker==WindowAndroid.START_INTENT_FAILURE){mSelecting=false;cleanup(export.file);reply.accept(fail(request,"runtime_unavailable","No document destination is available."));}
        return true;
    }
    private void selectImport(JSONObject request,JSONObject params,boolean firmware,NativeCore.Reply reply) {
        if(mSelecting||mCopying||!alive(request)){reply.accept(fail(request,"busy","Finish the current document selection first."));return;}
        if(firmware&&(params==null||params.optString("system").isEmpty()||params.optString("role").isEmpty())){reply.accept(fail(request,"invalid_request","Choose the firmware system and role."));return;}
        Intent intent=new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*");
        mSelecting=true;
        mPicker=mWindow.showCancelableIntent(intent,(code,data)->{
            mSelecting=false;mPicker=WindowAndroid.START_INTENT_FAILURE;
            if(code!=Activity.RESULT_OK||data==null||data.getData()==null||!alive(request)){reply.accept(fail(request,"cancelled","Document selection was cancelled or its deadline expired."));return;}
            Uri uri=data.getData();
            if(!"content".equals(uri.getScheme())){reply.accept(fail(request,"permission_denied","Choose a document using the system picker."));return;}
            mCopying=true;
            NativeCore.Reply completed=value->{mCopying=false;reply.accept(value);};
            mCore.storage(root->{if(!alive(request)){completed.accept(fail(request,"cancelled","Document import expired."));return;}mIo.execute(()->stage(uri,root,request,params,firmware,completed));},()->completed.accept(fail(request,"storage_failure","Local application storage is unavailable.")));
        },null);
        if(mPicker==WindowAndroid.START_INTENT_FAILURE){mSelecting=false;reply.accept(fail(request,"runtime_unavailable","No document picker is available."));}
    }
    private void stage(Uri uri,File root,JSONObject request,JSONObject params,boolean firmware,NativeCore.Reply reply) {
        File target=null;
        CancellationSignal cancellation=new CancellationSignal();mActiveCancellation=cancellation;
        Runnable cancel=()->cancellation.cancel();mMain.postDelayed(cancel,Math.max(1,request.optLong("deadline_ms")-System.currentTimeMillis()));
        try {
            if(!alive(request))throw new IllegalStateException();
            String display="Game";ContentResolver resolver=mActivity.getContentResolver();
            try(Cursor cursor=resolver.query(uri,new String[]{OpenableColumns.DISPLAY_NAME},null,null,null,cancellation)){
                if(cursor!=null&&cursor.moveToFirst()){String name=cursor.getString(0);if(name!=null)display=name.substring(0,Math.min(256,name.length()));}
            }
            int dot=display.lastIndexOf('.');String extension=dot<0?"":display.substring(dot+1).toLowerCase(java.util.Locale.ROOT);
            if(!extension.matches("[a-z0-9]{1,12}"))extension="bin";
            File inbox=new File(root,"NativeInbox");if(!inbox.isDirectory()&&!inbox.mkdirs())throw new IllegalStateException();
            target=new File(inbox,UUID.randomUUID()+"."+extension);
            if(!target.createNewFile())throw new IllegalStateException();
            try(ParcelFileDescriptor descriptor=resolver.openFileDescriptor(uri,"r",cancellation)){
                if(descriptor==null)throw new IllegalStateException();
                long size=descriptor.getStatSize(),maximum=firmware?128L*1024*1024:4L*1024*1024*1024;
                if(size>maximum||size==0||size>0&&size+64L*1024*1024>inbox.getUsableSpace())throw new IllegalStateException();
                try(FileInputStream input=new ParcelFileDescriptor.AutoCloseInputStream(descriptor);FileOutputStream output=new FileOutputStream(target)) {
                    long count=copy(input,output,maximum,request,inbox);if(count==0||size>=0&&count!=size)throw new IllegalStateException();output.getFD().sync();
                }
            }
            final File staged=target;final String title=(dot>0?display.substring(0,dot):display).replaceAll("[\\p{Cntrl}]","");
            mMain.post(()->{
                if(!alive(request)){cleanup(staged);reply.accept(fail(request,"cancelled","Document import expired."));return;}
                mCore.work(()->{
                    try {if(!alive(request))throw new IllegalStateException();JSONObject authorization=new JSONObject().put("kind",firmware?"firmware":"import").put("path",staged.getAbsolutePath());
                        if(firmware)authorization.put("system",params.getString("system")).put("role",params.getString("role"));
                        return mCore.authorize(authorization);
                    }finally{cleanup(staged);}
                },value->{
                    if(!"ok".equals(value.optString("status"))){reply.accept(fail(request,"storage_failure","The selected document could not be imported."));return;}
                    JSONObject result=value.optJSONObject("result");try{if(!firmware&&result!=null)result.put("suggested_title",title.substring(0,Math.min(120,title.length())));}catch(Exception ignored){}
                    reply.accept(NativeCore.success(request,result));
                });
            });
        }catch(Exception error){if(target!=null)cleanup(target);mMain.post(()->reply.accept(fail(request,"storage_failure","The selected document could not be read within its size, storage or time limit.")));}
        finally{mMain.removeCallbacks(cancel);mActiveCancellation=null;}
    }
    private long copy(FileInputStream input,FileOutputStream output,long maximum,JSONObject request,File storage)throws Exception {
        byte[] block=new byte[256*1024];long total=0;int count;
        // A provider may return a pipe. Closing the descriptors also bounds blocked IO.
        Runnable abort=()->{try{input.close();}catch(Exception ignored){}try{output.close();}catch(Exception ignored){}};
        mMain.postDelayed(abort,Math.max(1,request.optLong("deadline_ms")-System.currentTimeMillis()));
        try {while((count=input.read(block))!=-1){total+=count;if(!alive(request)||total>maximum||storage!=null&&storage.getUsableSpace()<16L*1024*1024)throw new IllegalStateException();output.write(block,0,count);}return total;}
        finally{mMain.removeCallbacks(abort);}
    }
    private void createExport(JSONObject request,JSONObject params,NativeCore.Reply reply) {
        String name=params==null?"":params.optString("suggested_name");
        if(name.isEmpty()||name.length()>128||name.equals(".")||name.equals("..")||!name.matches("[A-Za-z0-9_.-]+")||mExports.size()>=16){reply.accept(fail(request,"invalid_request","Choose a valid export filename and finish pending exports."));return;}
        mCore.work(()->{
            if(!alive(request))return fail(request,"cancelled","Export selection expired.");
            File directory=new File(new File(mCore.root(),"NativeExports"),UUID.randomUUID().toString());if(!directory.mkdirs())throw new IllegalStateException();
            File path=new File(directory,name);JSONObject authorized=mCore.authorize(new JSONObject().put("kind","export").put("path",path.getAbsolutePath()));
            JSONObject result=authorized.optJSONObject("result");
            if(!"ok".equals(authorized.optString("status"))||result==null){cleanup(path);return fail(request,"storage_failure","Private export staging is unavailable.");}
            // Registration occurs on the main thread before the renderer sees the grant.
            mMain.post(()->{if(alive(request)){mExports.put(result.optString("grant_id"),new Export(path));reply.accept(NativeCore.success(request,result));}else{cleanup(path);reply.accept(fail(request,"cancelled","Export selection expired."));}});
            return new JSONObject().put("native_deferred",true);
        },value->{if(!value.optBoolean("native_deferred"))reply.accept(value);});
    }
    private void prune(){java.util.Iterator<Map.Entry<String,Export>> items=mExports.entrySet().iterator();while(items.hasNext()){Export value=items.next().getValue();if(value.expiry<System.currentTimeMillis()){cleanup(value.file);items.remove();}}}
    private static void cleanup(File file){file.delete();File parent=file.getParentFile();if(parent!=null&&parent.getParentFile()!=null&&parent.getParentFile().getName().equals("NativeExports"))parent.delete();}
    @Override public void close(){mClosed=true;for(SelectedDiscTree tree:mTrees)tree.cancel();mTrees.clear();CancellationSignal active=mActiveCancellation;if(active!=null)active.cancel();if(mPicker!=WindowAndroid.START_INTENT_FAILURE)mWindow.cancelIntent(mPicker);for(Export value:mExports.values())cleanup(value.file);mExports.clear();mIo.shutdownNow();}
}
