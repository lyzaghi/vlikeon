package org.chromium.chrome.browser.omniplay;

import android.content.ContentResolver;
import android.content.res.AssetFileDescriptor;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.CancellationSignal;
import android.os.SystemClock;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;
import android.system.ErrnoException;
import android.system.Os;
import android.system.OsConstants;
import android.system.StructPollfd;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.FileDescriptor;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.UUID;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.Semaphore;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;

/** Native-only, transient SAF subtree. Paths are relative document names, never URIs from IPC. */
@android.annotation.TargetApi(30)
public final class SelectedDiscTree {
    private static final long MAX_BYTES=4L*1024*1024*1024;
    private static final long OPERATION_MS=30_000, SELECTION_MS=300_000, IMPORT_MS=1_800_000;
    private static final String[] COLUMNS={Document.COLUMN_DOCUMENT_ID,Document.COLUMN_DISPLAY_NAME,Document.COLUMN_MIME_TYPE,Document.COLUMN_SIZE,Document.COLUMN_FLAGS};
    private static final ScheduledThreadPoolExecutor CLOCK=new ScheduledThreadPoolExecutor(2,r->{Thread thread=new Thread(r,"OmniPlayDocumentDeadline");thread.setDaemon(true);return thread;});
    // A stalled provider cleanup keeps its permit. New selections then fail closed
    // instead of growing threads, retained URI trees or a cleanup queue forever.
    private static final Semaphore PROVIDERS=new Semaphore(4);
    private static final ThreadPoolExecutor CLEANUP=new ThreadPoolExecutor(2,2,0,TimeUnit.MILLISECONDS,new ArrayBlockingQueue<>(4),r->{Thread thread=new Thread(r,"OmniPlayDocumentClose");thread.setDaemon(true);return thread;});
    static {CLOCK.setRemoveOnCancelPolicy(true);}
    private static final class Failure extends Exception {final int code;Failure(int value){code=value;}}
    private static final class Node {
        final String id,name;final boolean directory,virtual;final long size;
        Node(String identity,String display,boolean folder,boolean transformed,long bytes){id=identity;name=display;directory=folder;virtual=transformed;size=bytes;}
    }
    private static final class Stream {
        final Node node;long seen,declared=-1;boolean closed;
        AssetFileDescriptor asset;
        Stream(Node value){node=value;}
    }
    private final class Operation implements AutoCloseable {
        final CancellationSignal signal=new CancellationSignal();final long deadline;final ScheduledFuture<?> timer;
        Operation()throws Failure{
            synchronized(mState){checkLocked();if(mOperation!=null)throw new Failure(-5);deadline=Math.min(mDeadline,SystemClock.elapsedRealtime()+OPERATION_MS);mOperation=this;}
            timer=CLOCK.schedule(()->stop(-4,this,0),Math.max(1,deadline-SystemClock.elapsedRealtime()),TimeUnit.MILLISECONDS);
        }
        void check()throws Failure{SelectedDiscTree.this.check();if(SystemClock.elapsedRealtime()>=deadline){stop(-4);throw new Failure(-4);}}
        @Override public void close(){synchronized(mState){if(mOperation==this)mOperation=null;}timer.cancel(false);}
    }
    private final ContentResolver mResolver;
    private final Uri mTree;
    private final String mRootId;
    private final byte[] mProfile;
    private final Consumer<SelectedDiscTree> mReleased;
    private final Object mState=new Object(),mIo=new Object();
    private final Map<Long,Stream> mStreams=new HashMap<>();
    // Only mIo reads/writes these caches. Cached rows are bounded across the entire tree.
    private final Map<String,Map<String,Node>> mDirectories=new HashMap<>();
    private int mRows,mMetadataBytes;
    private long mNextToken=1,mDeadline;
    private int mStopped;
    private boolean mPrepared,mStarted,mNativeOwned,mDidRelease,mCleanupDone,mBudgetReleased;
    private Operation mOperation;
    private ScheduledFuture<?> mExpiry;

    public static boolean supported(){return Build.VERSION.SDK_INT>=30;}
    public SelectedDiscTree(ContentResolver resolver,UUID profile,Uri tree,long requestDeadline,Consumer<SelectedDiscTree> released)throws Exception {
        if(!supported()||tree==null||!"content".equals(tree.getScheme())||tree.toString().length()>8192||!DocumentsContract.isTreeUri(tree))throw new Failure(-3);
        mResolver=resolver;mTree=tree;mRootId=DocumentsContract.getTreeDocumentId(tree);validateId(mRootId);mReleased=released;
        mProfile=ByteBuffer.allocate(16).putLong(profile.getMostSignificantBits()).putLong(profile.getLeastSignificantBits()).array();
        if(!PROVIDERS.tryAcquire())throw new Failure(-5);
        try{synchronized(mState){armLocked(SystemClock.elapsedRealtime()+Math.max(1,Math.min(120_000,requestDeadline-System.currentTimeMillis())));}}catch(RuntimeException error){PROVIDERS.release();throw error;}
    }
    public JSONObject prepare()throws Exception {
        synchronized(mIo){
            check();if(mPrepared)throw new Failure(-3);
            Node root;
            try(Operation operation=new Operation();Cursor cursor=mResolver.query(document(mRootId),COLUMNS,null,null,null,operation.signal)){
                operation.check();if(cursor==null||!cursor.moveToFirst())throw new Failure(-1);root=node(cursor);if(!root.directory||!mRootId.equals(root.id)||cursor.moveToNext())throw new Failure(-3);operation.check();
            }
            Set<String> names=new TreeSet<>();
            for(Node child:children(mRootId).values()){
                if(child.directory||child.virtual||child.size==0)continue;int dot=child.name.lastIndexOf('.');String extension=dot<0?"":child.name.substring(dot+1).toLowerCase(java.util.Locale.ROOT);
                if(extension.equals("cue")||extension.equals("m3u")||extension.equals("chd")||extension.equals("iso")||extension.equals("pbp")){names.add(child.name);if(names.size()>64)throw new Failure(-5);}
            }
            if(names.isEmpty())throw new Failure(-1);
            StringBuilder title=new StringBuilder();root.name.codePoints().filter(value->!Character.isISOControl(value)&&!(value>=0xd800&&value<=0xdfff)).limit(120).forEach(title::appendCodePoint);
            if(title.toString().trim().isEmpty())title.append("Game");
            synchronized(mState){checkLocked();mPrepared=true;armLocked(SystemClock.elapsedRealtime()+SELECTION_MS);}
            return new JSONObject().put("entries",new JSONArray(names)).put("suggested_title",title.toString().trim());
        }
    }
    // JNI ABI: {status, nonzero token, known byte length or -1}. No native path leaves this object.
    public long[] open(byte[] profile,byte[] relative){
        synchronized(mIo){try{
            if(profile==null||!MessageDigest.isEqual(profile,mProfile)||relative==null||relative.length==0||relative.length>2048)throw new Failure(-3);
            String path=StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(relative)).toString();
            String[] components=path.split("/",-1);if(components.length==0||components.length>8)throw new Failure(-3);for(String component:components)if(!safeName(component))throw new Failure(-3);
            synchronized(mState){checkLocked();if(!mPrepared||mStreams.size()>=129)throw new Failure(-3);if(!mStarted){mStarted=true;armLocked(SystemClock.elapsedRealtime()+IMPORT_MS);}}
            String parent=mRootId;Set<String> ancestors=new HashSet<>();ancestors.add(parent);Node found=null;
            for(int i=0;i<components.length;i++){
                check();found=children(parent).get(components[i]);if(found==null)throw new Failure(-1);if(found.virtual||ancestors.contains(found.id))throw new Failure(-3);
                if(i+1<components.length){if(!found.directory)throw new Failure(-3);parent=found.id;ancestors.add(parent);}else if(found.directory||found.size==0||found.size>MAX_BYTES)throw new Failure(-3);
            }
            synchronized(mState){checkLocked();if(mStreams.size()>=129||mNextToken<=0||mNextToken==Long.MAX_VALUE)throw new Failure(-5);long token=mNextToken++;mStreams.put(token,new Stream(found));return new long[]{0,token,found.size};}
        }catch(Exception error){return new long[]{code(error),0,0};}}
    }
    public int read(long token,byte[] output,int capacity){
        synchronized(mIo){try(Operation operation=new Operation()){
            if(output==null||capacity<=0||capacity>65_536||capacity>output.length)throw new Failure(-3);
            Stream stream;synchronized(mState){checkLocked();stream=mStreams.get(token);if(stream==null||stream.closed)throw new Failure(-3);}
            FileDescriptor descriptor=descriptor(stream,operation);int maximum=capacity;
            if(stream.declared>=0){long remaining=stream.declared-stream.seen;if(remaining<0)throw new Failure(-5);if(remaining==0){operation.check();return 0;}maximum=(int)Math.min(maximum,remaining);}
            StructPollfd poll=new StructPollfd();poll.fd=descriptor;poll.events=(short)(OsConstants.POLLIN|OsConstants.POLLERR|OsConstants.POLLHUP);StructPollfd[] descriptors={poll};
            while(true){
                operation.check();int ready;
                try{ready=Os.poll(descriptors,(int)Math.max(1,Math.min(250,operation.deadline-SystemClock.elapsedRealtime())));}catch(ErrnoException error){if(error.errno==OsConstants.EINTR)continue;throw error;}
                operation.check();if(ready==0)continue;if((poll.revents&OsConstants.POLLNVAL)!=0)throw new Failure(-5);
                int count;
                try{count=Os.read(descriptor,output,0,maximum);}catch(ErrnoException error){if(error.errno==OsConstants.EAGAIN||error.errno==OsConstants.EINTR)continue;throw error;}
                operation.check();if(count<0||count>maximum)throw new Failure(-5);
                if(count==0){if(stream.declared>=0&&stream.seen!=stream.declared)throw new Failure(-5);return 0;}
                stream.seen+=count;if(stream.seen>MAX_BYTES||stream.node.size>=0&&stream.seen>stream.node.size)throw new Failure(-5);return count;
            }
        }catch(Exception error){return code(error);}}
    }
    private FileDescriptor descriptor(Stream stream,Operation operation)throws Exception {
        synchronized(mState){checkLocked();if(stream.closed)throw new Failure(-2);if(stream.asset!=null)return stream.asset.getFileDescriptor();}
        AssetFileDescriptor asset=mResolver.openAssetFileDescriptor(document(stream.node.id),"r",operation.signal);
        if(asset==null)throw new Failure(-1);boolean admitted=false;
        try{
            operation.check();long length=asset.getDeclaredLength(),offset=asset.getStartOffset();
            if(length< -1||length==0||length>MAX_BYTES||offset<0||length>=0&&stream.node.size>=0&&length!=stream.node.size)throw new Failure(-5);
            FileDescriptor descriptor=asset.getFileDescriptor();
            try{if(Os.lseek(descriptor,offset,OsConstants.SEEK_SET)!=offset)throw new Failure(-5);}catch(ErrnoException error){if(offset!=0||error.errno!=OsConstants.ESPIPE)throw error;}
            int flags=Os.fcntlInt(descriptor,OsConstants.F_GETFL,0);Os.fcntlInt(descriptor,OsConstants.F_SETFL,flags|OsConstants.O_NONBLOCK);
            synchronized(mState){checkLocked();if(stream.closed)throw new Failure(-2);stream.declared=length;stream.asset=asset;admitted=true;}
            return descriptor;
        }finally{if(!admitted)asset.close();}
    }
    private Map<String,Node> children(String parent)throws Exception {
        check();Map<String,Node> cached=mDirectories.get(parent);if(cached!=null)return cached;if(mDirectories.size()>=256)throw new Failure(-5);
        Map<String,Node> children=new HashMap<>();
        try(Operation operation=new Operation();Cursor cursor=mResolver.query(DocumentsContract.buildChildDocumentsUriUsingTree(mTree,parent),COLUMNS,null,null,null,operation.signal)){
            if(cursor==null)throw new Failure(-5);
            while(cursor.moveToNext()){
                operation.check();if(++mRows>4096)throw new Failure(-5);Node child=node(cursor);
                mMetadataBytes+=utf8(child.id)+utf8(child.name)+64;if(mMetadataBytes>2*1024*1024)throw new Failure(-5);
                if(!safeName(child.name))continue;if(children.put(child.name,child)!=null)throw new Failure(-3);
            }
            if(cursor.getExtras().getBoolean(DocumentsContract.EXTRA_LOADING,false))throw new Failure(-5);operation.check();
        }
        mDirectories.put(parent,children);return children;
    }
    private Node node(Cursor cursor)throws Exception {
        String id=cursor.getString(0),name=cursor.getString(1),mime=cursor.getString(2);validateId(id);
        if(name==null||name.length()>512||utf8(name)>2048||mime==null||mime.length()>256)throw new Failure(-3);
        long size=cursor.isNull(3)?-1:cursor.getLong(3);if(size< -1)throw new Failure(-5);
        int flags=cursor.isNull(4)?0:cursor.getInt(4);return new Node(id,name,Document.MIME_TYPE_DIR.equals(mime),(flags&Document.FLAG_VIRTUAL_DOCUMENT)!=0,size);
    }
    private Uri document(String id){return DocumentsContract.buildDocumentUriUsingTree(mTree,id);}
    private static void validateId(String value)throws Failure{if(value==null||value.isEmpty()||value.length()>4096||utf8(value)>4096||value.indexOf('\0')>=0||!StandardCharsets.UTF_8.newEncoder().canEncode(value))throw new Failure(-3);}
    private static int utf8(String value){return value.getBytes(StandardCharsets.UTF_8).length;}
    private static boolean safeName(String value){
        if(value==null||value.isEmpty()||value.equals(".")||value.equals("..")||utf8(value)>512||value.endsWith(".")||value.endsWith(" "))return false;
        for(int i=0;i<value.length();i++){char c=value.charAt(i);if(c=='/'||c=='\\'||c==':'||Character.isISOControl(c)||Character.isSurrogate(c)&&!(Character.isHighSurrogate(c)&&i+1<value.length()&&Character.isLowSurrogate(value.charAt(++i))))return false;}return true;
    }
    private void check()throws Failure{synchronized(mState){checkLocked();}}
    private void checkLocked()throws Failure{if(mStopped!=0)throw new Failure(mStopped);if(SystemClock.elapsedRealtime()>=mDeadline)throw new Failure(-4);}
    private void armLocked(long deadline){
        if(mExpiry!=null)mExpiry.cancel(false);mDeadline=deadline;
        mExpiry=CLOCK.schedule(()->stop(-4,null,deadline),Math.max(1,deadline-SystemClock.elapsedRealtime()),TimeUnit.MILLISECONDS);
    }
    private int code(Exception error){synchronized(mState){if(mStopped!=0)return mStopped;}if(error instanceof Failure)return ((Failure)error).code;if(error instanceof SecurityException)return -3;if(error instanceof java.io.FileNotFoundException)return -1;if(error instanceof android.os.OperationCanceledException)return -2;return -5;}
    /** Synchronous marking only; closing or signalling a provider never runs on the caller. */
    public void cancel(){stop(-2);}
    private void stop(int code){stop(code,null,0);}
    private void stop(int code,Operation expected,long expectedDeadline){
        final CancellationSignal signal;final ArrayList<AssetFileDescriptor> assets=new ArrayList<>();
        synchronized(mState){if(mStopped!=0||expected!=null&&mOperation!=expected||expectedDeadline!=0&&mDeadline!=expectedDeadline)return;mStopped=code;if(mExpiry!=null)mExpiry.cancel(false);signal=mOperation==null?null:mOperation.signal;for(Stream stream:mStreams.values()){stream.closed=true;if(stream.asset!=null){assets.add(stream.asset);stream.asset=null;}}}
        CLEANUP.execute(()->{try{if(signal!=null)try{signal.cancel();}catch(Exception ignored){}for(AssetFileDescriptor asset:assets)try{asset.close();}catch(Exception ignored){}}finally{synchronized(mState){mCleanupDone=true;releaseBudgetLocked();}}});
    }
    public void close(long token){
        AssetFileDescriptor asset;synchronized(mState){Stream stream=mStreams.remove(token);if(stream==null)return;stream.closed=true;asset=stream.asset;stream.asset=null;}
        if(asset!=null)try{asset.close();}catch(Exception ignored){}
    }
    public void release(){
        synchronized(mState){if(mDidRelease)return;mDidRelease=true;mNativeOwned=false;releaseBudgetLocked();}cancel();mReleased.accept(this);
    }
    /** JNI alone transfers a successfully retained native context to Rust. */
    public boolean retainForRust(){synchronized(mState){if(!mPrepared||mStopped!=0||mDidRelease||mNativeOwned||SystemClock.elapsedRealtime()>=mDeadline)return false;mNativeOwned=true;return true;}}
    /** A lost native reply cannot release Rust's outstanding ownership early. */
    public void abandon(){synchronized(mState){if(mNativeOwned)cancel();else release();}}
    private void releaseBudgetLocked(){if(mDidRelease&&mCleanupDone&&!mBudgetReleased){mBudgetReleased=true;PROVIDERS.release();}}
}
