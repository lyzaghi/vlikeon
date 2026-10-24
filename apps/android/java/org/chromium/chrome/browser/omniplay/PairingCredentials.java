package org.chromium.chrome.browser.omniplay;

import android.app.KeyguardManager;
import android.content.Context;
import android.os.Build;
import android.os.UserManager;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.system.Os;
import android.system.OsConstants;
import java.io.File;
import java.io.FileDescriptor;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.NoSuchFileException;
import java.nio.file.StandardOpenOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.security.Key;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.UUID;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/** Native-only Remote Play wrapping; never a WebView/JavaScript API.
 * JNI address encoding: four 16-byte UUIDs then a BIG-ENDIAN u32 kind.
 * This differs from blindly copying the native C struct's integer bytes.
 */
final class PairingCredentials {
    private static final int SECRET_BYTES = 137;
    private static final int RECORD_BYTES = 8 + 12 + SECRET_BYTES + 16;
    private static final int RETIREMENT_BYTES = 8 + 68;
    private static final byte[] MAGIC = "VLKRPW01".getBytes(StandardCharsets.US_ASCII);
    private static final byte[] SECRET_MAGIC = "VLKRP001".getBytes(StandardCharsets.US_ASCII);
    private static final byte[] RETIREMENT_MAGIC = "VLKRPF01".getBytes(StandardCharsets.US_ASCII);
    private static final byte[] DOMAIN = "vlikeon.remote-play.wrap.v1\0".getBytes(StandardCharsets.US_ASCII);
    private final Context mContext;
    private final File mNoBackup;
    private final File mDirectory;
    private final byte[] mProfile;
    private final byte[] mDevice;

    PairingCredentials(Context context, UUID profile, UUID device) throws Exception {
        if (profile.equals(new UUID(0, 0)) || device.equals(new UUID(0, 0))) throw new IllegalArgumentException();
        mContext = context.getApplicationContext();
        if (mContext.isDeviceProtectedStorage()) throw new IllegalArgumentException();
        mNoBackup = mContext.getNoBackupFilesDir().getCanonicalFile();
        mDirectory = new File(mNoBackup, "VlikeonRemotePlayCredentialsV1");
        mProfile = uuidBytes(profile); mDevice = uuidBytes(device);
    }
    private static byte[] uuidBytes(UUID id) {
        return ByteBuffer.allocate(16).putLong(id.getMostSignificantBits()).putLong(id.getLeastSignificantBits()).array();
    }
    private boolean owns(byte[] address) {
        if (address == null || address.length != 68) return false;
        if (!MessageDigest.isEqual(mProfile, Arrays.copyOfRange(address, 0, 16)) ||
                !MessageDigest.isEqual(mDevice, Arrays.copyOfRange(address, 16, 32))) return false;
        for (int offset = 0; offset < 64; offset += 16) {
            int present = 0; for (int i = offset; i < offset + 16; ++i) present |= address[i];
            if (present == 0) return false;
        }
        int kind = ByteBuffer.wrap(address, 64, 4).getInt();
        return (kind == 1 || kind == 2) && (kind != 1 || MessageDigest.isEqual(mDevice, Arrays.copyOfRange(address, 32, 48)));
    }
    private static boolean envelope(byte[] address, byte[] secret) {
        if (secret == null || secret.length != SECRET_BYTES) return false;
        for (int i = 0; i < 8; ++i) if (secret[i] != SECRET_MAGIC[i]) return false;
        if (secret[8] != ByteBuffer.wrap(address, 64, 4).getInt()) return false;
        for (int i = 0; i < 64; ++i) if (secret[9 + i] != address[i]) return false;
        return true;
    }
    private boolean unlocked() {
        UserManager user = mContext.getSystemService(UserManager.class);
        KeyguardManager guard = mContext.getSystemService(KeyguardManager.class);
        return user != null && guard != null && user.isUserUnlocked() && !guard.isDeviceLocked();
    }
    private static boolean present(File file) throws Exception {
        try { Files.readAttributes(file.toPath(), BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS); return true; }
        catch (NoSuchFileException absent) { return false; }
    }
    private static void ordinary(File file, boolean directory) throws Exception {
        BasicFileAttributes metadata = Files.readAttributes(file.toPath(), BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        if (metadata.isSymbolicLink() || (directory ? !metadata.isDirectory() : !metadata.isRegularFile()) ||
                !file.getCanonicalFile().equals(file.getAbsoluteFile())) throw new IllegalStateException();
    }
    private void directory() throws Exception {
        ordinary(mNoBackup, true);
        if (!present(mDirectory)) {
            try { Files.createDirectory(mDirectory.toPath()); }
            catch (java.nio.file.FileAlreadyExistsException concurrent) { /* Validate winner below. */ }
            syncDirectory(mNoBackup);
        }
        ordinary(mDirectory, true); Os.chmod(mDirectory.getAbsolutePath(), 0700);
    }
    private static String token(byte[] address) throws Exception {
        byte[] hash = MessageDigest.getInstance("SHA-256").digest(address);
        StringBuilder out = new StringBuilder(64);
        for (byte value : hash) { out.append(Character.forDigit((value & 255) >>> 4, 16)); out.append(Character.forDigit(value & 15, 16)); }
        return out.toString();
    }
    private File record(String token) { return new File(mDirectory, token + ".pairing"); }
    private File retirement(String token) { return new File(mDirectory, token + ".retired"); }
    private static byte[] retirementBytes(byte[] address) { return ByteBuffer.allocate(RETIREMENT_BYTES).put(RETIREMENT_MAGIC).put(address).array(); }
    private static String alias(String token) { return "org.vlikeon.remote-play.wrap.v1." + token; }
    private static KeyStore keys() throws Exception { KeyStore store = KeyStore.getInstance("AndroidKeyStore"); store.load(null); return store; }
    private static SecretKey existing(KeyStore store, String alias) throws Exception {
        if (!store.containsAlias(alias)) throw new IllegalStateException();
        Key key = store.getKey(alias, null);
        if (!(key instanceof SecretKey) || !"AES".equals(key.getAlgorithm())) throw new IllegalStateException();
        return (SecretKey) key;
    }
    private static SecretKey create(KeyStore store, String alias) throws Exception {
        if (store.containsAlias(alias)) throw new IllegalStateException();
        KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        KeyGenParameterSpec.Builder spec = new KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256).setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).setRandomizedEncryptionRequired(true);
        // Android documents credential-loss/unlock bugs through API 34. Earlier
        // versions retain explicit app lock checks; no hardware-lock claim.
        if (Build.VERSION.SDK_INT >= 35) spec.setUnlockedDeviceRequired(true);
        generator.init(spec.build()); return generator.generateKey();
    }
    private static byte[] aad(byte[] address) { return ByteBuffer.allocate(DOMAIN.length + 68 + 8).put(DOMAIN).put(address).put(MAGIC).array(); }
    private static void syncDirectory(File path) throws Exception {
        FileDescriptor fd = Os.open(path.getAbsolutePath(), OsConstants.O_RDONLY | OsConstants.O_DIRECTORY | OsConstants.O_CLOEXEC, 0);
        try { Os.fsync(fd); } finally { Os.close(fd); }
    }
    private static final class Lease implements AutoCloseable {
        private final FileChannel channel; private FileLock lock;
        Lease(File path) throws Exception {
            channel = FileChannel.open(path.toPath(), StandardOpenOption.READ, StandardOpenOption.WRITE, StandardOpenOption.CREATE, LinkOption.NOFOLLOW_LINKS);
            try { ordinary(path, false); if (channel.size() != 0) throw new IllegalStateException(); lock = channel.tryLock();
                if (lock == null) throw new IllegalStateException(); Os.chmod(path.getAbsolutePath(), 0600); }
            catch (Exception error) { try { if (lock != null) lock.release(); } finally { channel.close(); } throw error; }
        }
        public void close() throws Exception { try { if (lock != null) lock.release(); } finally { channel.close(); } }
    }
    private Lease lease(String token) throws Exception { return new Lease(new File(mDirectory, token + ".lease")); }
    private static byte[] readRecord(File path) throws Exception {
        return readExact(path,RECORD_BYTES);
    }
    private static byte[] readExact(File path,int expected) throws Exception {
        if(expected!=RECORD_BYTES&&expected!=RETIREMENT_BYTES)throw new IllegalArgumentException();
        ordinary(path, false); byte[] bytes = new byte[expected + 1];
        try (FileChannel channel = FileChannel.open(path.toPath(), StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)) {
            if (channel.size() != expected) throw new IllegalStateException();
            ByteBuffer target = ByteBuffer.wrap(bytes); while (target.hasRemaining()) { int n = channel.read(target); if (n < 0) break; if (n == 0) throw new IllegalStateException(); }
            if (target.position() != expected || channel.size() != expected) throw new IllegalStateException();
            return Arrays.copyOf(bytes, expected);
        } finally { Arrays.fill(bytes, (byte) 0); }
    }
    private static void verifyRetirement(File path,byte[] address) throws Exception {
        byte[] observed=readExact(path,RETIREMENT_BYTES);
        try{if(!MessageDigest.isEqual(observed,retirementBytes(address)))throw new IllegalStateException();}
        finally{Arrays.fill(observed,(byte)0);}
    }
    /** Retain the marker permanently. Explicit cleanup may finish a matching
     * public prefix after a crash; wrong bytes are never overwritten. Holds Lease. */
    private void sealRetirement(String token,byte[] address) throws Exception {
        File path=retirement(token);FileChannel opened;
        try{opened=FileChannel.open(path.toPath(),StandardOpenOption.READ,StandardOpenOption.WRITE,StandardOpenOption.CREATE_NEW,LinkOption.NOFOLLOW_LINKS);}
        catch(java.nio.file.FileAlreadyExistsException exists){ordinary(path,false);opened=FileChannel.open(path.toPath(),StandardOpenOption.READ,StandardOpenOption.WRITE,LinkOption.NOFOLLOW_LINKS);}
        try(FileChannel channel=opened){
            ordinary(path,false);Os.chmod(path.getAbsolutePath(),0600);long length=channel.size();
            if(length<0||length>RETIREMENT_BYTES)throw new IllegalStateException();
            byte[] intended=retirementBytes(address),prefix=new byte[(int)length];ByteBuffer input=ByteBuffer.wrap(prefix);
            while(input.hasRemaining())if(channel.read(input)<=0)throw new IllegalStateException();
            if(channel.size()!=length)throw new IllegalStateException();
            for(int i=0;i<prefix.length;i++)if(prefix[i]!=intended[i])throw new IllegalStateException();
            // Append only: no truncate/unlink window where a candidate could
            // mistake this generation for reusable. Empty interrupted create is
            // a valid public prefix; unrelated/corrupt bytes remain refused.
            channel.position(length);ByteBuffer tail=ByteBuffer.wrap(intended,(int)length,RETIREMENT_BYTES-(int)length);
            while(tail.hasRemaining())if(channel.write(tail)<=0)throw new IllegalStateException();channel.force(true);
        }
        verifyRetirement(path,address);syncDirectory(mDirectory);verifyRetirement(path,address);
    }
    public int check(byte[] address) {
        if (!owns(address)) return -3;
        try { return unlocked() ? 0 : -2; } catch (RuntimeException failure) { return -3; }
    }
    public synchronized int read(byte[] address, byte[] output) {
        if (!owns(address) || output == null || output.length != SECRET_BYTES) return -3;
        Arrays.fill(output, (byte) 0); byte[] raw = null, plaintext = null; boolean delivered = false;
        try {
            if (!unlocked()) return -2; directory(); String token = token(address);
            try (Lease owned = lease(token)) {
                boolean retired=present(retirement(token));if(retired)verifyRetirement(retirement(token),address);
                directory(); File path = record(token); KeyStore store = keys(); boolean hasFile = present(path), hasKey = store.containsAlias(alias(token));
                if (!hasFile && !hasKey) return unlocked() ? -1 : -2;
                // A barrier with remaining ciphertext/alias is incomplete
                // cleanup, not permission to decrypt or report missing.
                if(retired)return -3;
                if (!hasFile || !hasKey) return -3;
                raw = readRecord(path); for (int i = 0; i < 8; ++i) if (raw[i] != MAGIC[i]) return -3;
                Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
                cipher.init(Cipher.DECRYPT_MODE, existing(store, alias(token)), new GCMParameterSpec(128, raw, 8, 12));
                cipher.updateAAD(aad(address)); plaintext = cipher.doFinal(raw, 20, raw.length - 20);
                if (!envelope(address, plaintext)) return -3; if (!unlocked()) return -2;
                System.arraycopy(plaintext, 0, output, 0, SECRET_BYTES); if (!unlocked()) return -2;
                delivered = true; return SECRET_BYTES;
            }
        } catch (Exception failure) { Arrays.fill(output, (byte) 0); return -3; }
        finally { if (!delivered) Arrays.fill(output, (byte) 0); if (raw != null) Arrays.fill(raw, (byte) 0); if (plaintext != null) Arrays.fill(plaintext, (byte) 0); }
    }
    public synchronized int write(byte[] address, byte[] secret) {
        if (!owns(address) || !envelope(address, secret)) return -3;
        byte[] ciphertext = null, raw = null, observed = null;
        try {
            if (!unlocked()) return -2; directory(); String token = token(address);
            try (Lease owned = lease(token)) {
                if(present(retirement(token)))return -3;
                directory(); File path = record(token); KeyStore store = keys();
                // Missing-key ciphertext and orphan aliases require explicit
                // generation cleanup, never replacement or overwriting here.
                if (present(path) || store.containsAlias(alias(token))) return -3;
                Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
                cipher.init(Cipher.ENCRYPT_MODE, create(store, alias(token))); cipher.updateAAD(aad(address));
                byte[] iv = cipher.getIV(); if (iv == null || iv.length != 12) return -3;
                ciphertext = cipher.doFinal(secret); if (ciphertext.length != SECRET_BYTES + 16) return -3;
                raw = ByteBuffer.allocate(RECORD_BYTES).put(MAGIC).put(iv).put(ciphertext).array(); if (!unlocked()) return -2;
                if(present(retirement(token)))return -3;
                try (FileChannel channel = FileChannel.open(path.toPath(), StandardOpenOption.WRITE, StandardOpenOption.CREATE_NEW, LinkOption.NOFOLLOW_LINKS)) {
                    Os.chmod(path.getAbsolutePath(), 0600); ByteBuffer input = ByteBuffer.wrap(raw);
                    while (input.hasRemaining()) if (channel.write(input) <= 0) throw new IllegalStateException(); channel.force(true);
                }
                directory(); observed = readRecord(path); if (!MessageDigest.isEqual(raw, observed)) return -3;
                syncDirectory(mDirectory); return unlocked() ? 0 : -2;
            }
        } catch (Exception failure) { return -3; }
        finally { if (ciphertext != null) Arrays.fill(ciphertext, (byte) 0); if (raw != null) Arrays.fill(raw, (byte) 0); if (observed != null) Arrays.fill(observed, (byte) 0); }
    }
    public synchronized int remove(byte[] address) {
        if (!owns(address)) return -3;
        try {
            if (!unlocked()) return -2; directory(); String token = token(address);
            try (Lease owned = lease(token)) {
                // Persist/read back the no-reuse barrier before deleting any
                // wrapped material. Never remove this public marker on retry.
                sealRetirement(token,address);if(!unlocked())return -2;
                directory(); KeyStore store = keys(); File path = record(token);
                if (present(path)) ordinary(path, false);
                if (store.containsAlias(alias(token))) store.deleteEntry(alias(token));
                if (present(path)) Files.delete(path.toPath());
                if (present(path) || store.containsAlias(alias(token))) return -3;
                // Keep the zero-byte lock file named: unlinking it while another
                // process opened it would split the exclusion namespace.
                syncDirectory(mDirectory);verifyRetirement(retirement(token),address);return unlocked() ? 0 : -2;
            }
        } catch (Exception failure) { return -3; }
    }
}
