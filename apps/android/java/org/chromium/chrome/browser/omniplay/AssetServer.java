package org.chromium.chrome.browser.omniplay;

import android.content.res.AssetManager;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.ByteArrayOutputStream;
import java.io.Closeable;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.SynchronousQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

/** Read-only APK assets on an unguessable, per-process loopback origin/path. No RPC. */
public final class AssetServer implements Closeable {
    private static final class Asset {
        final long bytes; final String mime;
        Asset(long count, String type) { bytes=count; mime=type; }
    }
    private final AssetManager mAssets;
    private final Map<String, Asset> mInventory = new HashMap<>();
    private final ServerSocket mListener;
    private final ThreadPoolExecutor mClients = new ThreadPoolExecutor(0, 16, 30, TimeUnit.SECONDS, new SynchronousQueue<>());
    private final Set<Socket> mSockets = ConcurrentHashMap.newKeySet();
    private final java.util.concurrent.ScheduledExecutorService mDeadlines=java.util.concurrent.Executors.newSingleThreadScheduledExecutor();
    private final String mPrefix;
    private final String mOrigin;
    private String mStreamingSources="";
    private volatile boolean mClosed;

    public AssetServer(AssetManager assets) throws Exception {
        mAssets=assets;
        try (InputStream in = assets.open("omniplay/asset-index.json"); ByteArrayOutputStream out = new ByteArrayOutputStream()) {
            byte[] block=new byte[16*1024]; int count;
            while ((count=in.read(block))!=-1) { if (out.size()+count>1_048_576) throw new IllegalStateException(); out.write(block,0,count); }
            JSONObject index=new JSONObject(out.toString("UTF-8"));
            if (index.getInt("schema_version")!=1) throw new IllegalStateException();
            JSONArray origins=index.optJSONArray("streaming_origins");
            if(origins!=null){if(origins.length()>8)throw new IllegalStateException();StringBuilder allowed=new StringBuilder();for(int i=0;i<origins.length();++i){String origin=origins.getString(i);java.net.URI uri=new java.net.URI(origin);if(origin.length()>256||!origin.matches("[\\x21-\\x7e]+")||origin.indexOf('\'')>=0||origin.indexOf('"')>=0||origin.indexOf(';')>=0||!"https".equals(uri.getScheme())||uri.getHost()==null||uri.getUserInfo()!=null||!uri.getRawPath().isEmpty()||uri.getQuery()!=null||uri.getFragment()!=null||uri.getPort()==0||uri.getPort()>65535||!origin.equals("https://"+uri.getRawAuthority()))throw new IllegalStateException();allowed.append(' ').append(origin);}mStreamingSources=allowed.toString();}
            JSONArray list=index.getJSONArray("assets");
            if (list.length()>2048) throw new IllegalStateException();
            for (int i=0;i<list.length();++i) {
                JSONObject value=list.getJSONObject(i); String path=value.getString("path"),mime=value.getString("mime"); long length=value.getLong("bytes");
                if (!safePath(path)||length<0||length>512L*1024*1024||mime.length()>100||mime.indexOf('\r')>=0||mime.indexOf('\n')>=0||mInventory.containsKey(path)) throw new IllegalStateException();
                mInventory.put(path,new Asset(length,mime));
            }
        }
        if (!mInventory.containsKey("index.html")) throw new IllegalStateException();
        byte[] token=new byte[24];new SecureRandom().nextBytes(token);StringBuilder hex=new StringBuilder();
        for (byte value:token) hex.append(String.format(java.util.Locale.ROOT,"%02x",value&255));
        mPrefix="/h/"+hex+"/";
        mListener=new ServerSocket(0,16,InetAddress.getByName("127.0.0.1"));
        mOrigin="http://127.0.0.1:"+mListener.getLocalPort();
        Thread accept=new Thread(this::accept,"OmniPlayAssets");accept.setDaemon(true);accept.start();
    }
    private static boolean safePath(String path) {
        if (path.isEmpty()||path.length()>512||path.startsWith("/")||path.contains("\\"))return false;
        for (String segment:path.split("/",-1)) if (segment.isEmpty()||segment.equals(".")||segment.equals("..")||!segment.matches("[A-Za-z0-9_.-]+"))return false;
        return true;
    }
    public String origin() { return mOrigin; }
    public String entryURL() { return mOrigin+mPrefix+"index.html"; }
    public boolean owns(String url) {if(url==null)return false;int fragment=url.indexOf('#');return (fragment<0?url:url.substring(0,fragment)).equals(entryURL());}
    private void accept() {
        while (!mClosed) {
            Socket client=null;
            try { client=mListener.accept();client.setSoTimeout(15000);mSockets.add(client);final Socket selected=client;mClients.execute(()->serve(selected)); }
            catch (Exception error) { if (client!=null) {mSockets.remove(client);try{client.close();}catch(Exception ignored){}} }
        }
    }
    private static String line(InputStream in,int[] remaining) throws Exception {
        ByteArrayOutputStream out=new ByteArrayOutputStream();
        while (remaining[0]-->0) {
            int value=in.read();if(value<0)throw new IllegalStateException();
            if (value=='\n') {byte[] bytes=out.toByteArray();if(bytes.length==0||bytes[bytes.length-1]!='\r')throw new IllegalStateException();return new String(bytes,0,bytes.length-1,StandardCharsets.US_ASCII);}
            if (value<32&&value!='\r'||value>126||out.size()>=4096)throw new IllegalStateException();out.write(value);
        }
        throw new IllegalStateException();
    }
    private void serve(Socket client) {
        long started=System.nanoTime();
        java.util.concurrent.ScheduledFuture<?> deadline=mDeadlines.schedule(()->{try{client.close();}catch(Exception ignored){}},90,TimeUnit.SECONDS);
        try (Socket socket=client; InputStream input=socket.getInputStream(); OutputStream output=socket.getOutputStream()) {
            if (!socket.getInetAddress().isLoopbackAddress())return;
            int[] remaining={16384};String[] request=line(input,remaining).split(" ",-1);
            if(request.length!=3||!request[2].equals("HTTP/1.1")||!(request[0].equals("GET")||request[0].equals("HEAD"))) {reject(output,405);return;}
            String host=null,header;boolean invalid=false;int headers=0;
            while (!(header=line(input,remaining)).isEmpty()) {
                if (++headers>64)throw new IllegalStateException();int split=header.indexOf(':');if(split<=0)throw new IllegalStateException();
                String name=header.substring(0,split).toLowerCase(java.util.Locale.ROOT),value=header.substring(split+1).trim();
                if(name.equals("host")){if(host!=null)invalid=true;host=value;}
                if(name.equals("content-length")&&!value.equals("0")||name.equals("transfer-encoding"))invalid=true;
            }
            String target=request[1];
            if(invalid||!("127.0.0.1:"+mListener.getLocalPort()).equals(host)||!target.startsWith(mPrefix)){reject(output,404);return;}
            String relative=target.substring(mPrefix.length());Asset asset=mInventory.get(relative);
            if(!safePath(relative)||asset==null){reject(output,404);return;}
            String policy="default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self'"+mStreamingSources+"; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; frame-ancestors 'none'";
            String response="HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: "+asset.bytes+"\r\nContent-Type: "+asset.mime+"\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\nContent-Security-Policy: "+policy+"\r\n\r\n";
            output.write(response.getBytes(StandardCharsets.US_ASCII));
            if(request[0].equals("HEAD"))return;
            try(InputStream resource=mAssets.open("omniplay/"+relative)) {
                byte[] bytes=new byte[128*1024];long sent=0;int count;
                while((count=resource.read(bytes))!=-1) {
                    sent+=count;if(sent>asset.bytes||mClosed||TimeUnit.NANOSECONDS.toSeconds(System.nanoTime()-started)>90)throw new IllegalStateException();
                    output.write(bytes,0,count);
                }
                if(sent!=asset.bytes)throw new IllegalStateException();
            }
        } catch (Exception ignored) { /* No private paths, content names or tokens in logs. */ }
        finally {deadline.cancel(false);mSockets.remove(client);}
    }
    private static void reject(OutputStream output,int status)throws Exception {output.write(("HTTP/1.1 "+status+" Unavailable\r\nConnection: close\r\nContent-Length: 0\r\nCache-Control: no-store\r\n\r\n").getBytes(StandardCharsets.US_ASCII));}
    @Override public void close() {mClosed=true;try{mListener.close();}catch(Exception ignored){}for(Socket socket:mSockets)try{socket.close();}catch(Exception ignored){}mClients.shutdownNow();mDeadlines.shutdownNow();}
}
