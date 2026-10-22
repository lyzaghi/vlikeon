package org.chromium.chrome.browser.omniplay;

import android.app.KeyguardManager;
import android.content.Context;
import android.os.Build;
import android.os.UserManager;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.system.Os;
import android.system.OsConstants;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardOpenOption;
import java.security.Key;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.UUID;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;

/** Native-only device credential wrapping, separate from renderer file grants. */
final class SecureCredentials {
    private static final int MAX_SECRET = 4096;
    private static final int MAX_RECORD = 8 + 12 + MAX_SECRET + 16;
    private static final byte[] MAGIC = "OPCR0001".getBytes(StandardCharsets.US_ASCII);
    private static final byte[] DOMAIN = "omniplay.sync.device.v1\0".getBytes(StandardCharsets.US_ASCII);
    private final Context mContext;
    private final File mNoBackup;
    private final File mDirectory;
    private final File mRecord;
    private final byte[] mProfile;
    private final String mAlias;

    SecureCredentials(Context context, UUID profile) throws Exception {
        mContext = context.getApplicationContext();
        mNoBackup = mContext.getNoBackupFilesDir().getCanonicalFile();
        mDirectory = new File(mNoBackup, "OmniPlaySecureCredentials");
        mRecord = new File(mDirectory, profile.toString() + ".credential");
        mProfile = ByteBuffer.allocate(16).putLong(profile.getMostSignificantBits()).putLong(profile.getLeastSignificantBits()).array();
        mAlias = "org.omniplay.sync.wrap.v1." + profile.toString();
    }
    private boolean owns(byte[] profile) { return profile != null && profile.length == 16 && MessageDigest.isEqual(profile, mProfile); }
    private boolean unlocked() {
        UserManager user = mContext.getSystemService(UserManager.class);
        KeyguardManager keyguard = mContext.getSystemService(KeyguardManager.class);
        return user != null && keyguard != null && user.isUserUnlocked() && !keyguard.isDeviceLocked();
    }
    private static boolean exists(File value) { return Files.exists(value.toPath(), LinkOption.NOFOLLOW_LINKS); }
    private void directory(boolean create) throws Exception {
        directory(create, false);
    }
    private void directory(boolean create, boolean removing) throws Exception {
        if (!Files.isDirectory(mNoBackup.toPath(), LinkOption.NOFOLLOW_LINKS) || !mNoBackup.getCanonicalFile().equals(mNoBackup.getAbsoluteFile())) throw new IllegalStateException();
        if (!exists(mDirectory) && create) {
            try { Files.createDirectory(mDirectory.toPath()); }
            catch (java.nio.file.FileAlreadyExistsException concurrentOwner) { /* Validate the winning native directory below. */ }
            syncDirectory(mNoBackup);
        }
        if (!Files.isDirectory(mDirectory.toPath(), LinkOption.NOFOLLOW_LINKS) || !mDirectory.getCanonicalFile().equals(mDirectory.getAbsoluteFile())) throw new IllegalStateException();
        Os.chmod(mDirectory.getAbsolutePath(), 0700);
        for (File entry : records()) if (exists(entry) && (!Files.isRegularFile(entry.toPath(), LinkOption.NOFOLLOW_LINKS) || !removing && entry.length() > MAX_RECORD)) throw new IllegalStateException();
    }
    private File[] records() { return new File[] {mRecord, new File(mRecord.getPath() + ".new"), new File(mRecord.getPath() + ".bak")}; }
    private static final class RecordLock implements AutoCloseable {
        private final FileChannel channel;
        private FileLock lock;
        RecordLock(File path) throws Exception {
            channel = FileChannel.open(path.toPath(), StandardOpenOption.READ, StandardOpenOption.WRITE, StandardOpenOption.CREATE, LinkOption.NOFOLLOW_LINKS);
            try { if (channel.size() != 0) throw new IllegalStateException(); lock = channel.tryLock(); if (lock == null) throw new IllegalStateException(); Os.chmod(path.getAbsolutePath(), 0600); }
            catch (Exception error) { try { if (lock != null) lock.release(); } finally { channel.close(); } throw error; }
        }
        public void close() throws Exception { try { if (lock != null) lock.release(); } finally { channel.close(); } }
    }
    private RecordLock lockRecord() throws Exception { return new RecordLock(new File(mRecord.getPath() + ".lease")); }
    private boolean anyRecord() { for (File entry : records()) if (exists(entry)) return true; return false; }
    private static KeyStore keys() throws Exception { KeyStore store = KeyStore.getInstance("AndroidKeyStore"); store.load(null); return store; }
    private SecretKey key(KeyStore store, boolean create) throws Exception {
        if (store.containsAlias(mAlias)) {
            Key value = store.getKey(mAlias, null);
            if (!(value instanceof SecretKey) || !"AES".equals(value.getAlgorithm())) throw new IllegalStateException();
            return (SecretKey) value;
        }
        // Never silently replace an invalidated/missing wrapping key for retained
        // ciphertext. Recovery/disconnect must make that decision explicitly.
        if (!create || anyRecord()) throw new IllegalStateException();
        KeyGenerator generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore");
        KeyGenParameterSpec.Builder spec = new KeyGenParameterSpec.Builder(mAlias, KeyProperties.PURPOSE_ENCRYPT | KeyProperties.PURPOSE_DECRYPT)
                .setKeySize(256).setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).setRandomizedEncryptionRequired(true);
        // Android documents credential-loss/unlock bugs on API 34 and earlier.
        // Those versions keep explicit app lock checks without a hardware claim.
        if (Build.VERSION.SDK_INT >= 35) spec.setUnlockedDeviceRequired(true);
        generator.init(spec.build()); return generator.generateKey();
    }
    private byte[] associatedData() { return ByteBuffer.allocate(DOMAIN.length + 16 + MAGIC.length).put(DOMAIN).put(mProfile).put(MAGIC).array(); }
    private byte[] readRecord() throws Exception {
        byte[] raw = new byte[MAX_RECORD + 1]; int count = 0;
        try (FileInputStream input = new AtomicFile(mRecord).openRead()) {
            while (count < raw.length) { int size = input.read(raw, count, raw.length - count); if (size < 0) break; if (size == 0) throw new IllegalStateException(); count += size; }
            if (count < MAGIC.length + 12 + 16 + 1 || count > MAX_RECORD) throw new IllegalStateException();
            return Arrays.copyOf(raw, count);
        } finally { Arrays.fill(raw, (byte) 0); }
    }
    private static void syncDirectory(File directory) throws Exception {
        FileDescriptor fd = Os.open(directory.getAbsolutePath(), OsConstants.O_RDONLY | OsConstants.O_DIRECTORY | OsConstants.O_CLOEXEC, 0);
        try { Os.fsync(fd); } finally { Os.close(fd); }
    }
    // Called only through a retained JNI object; never registered as a JS API.
    public int check(byte[] profile) {
        if (!owns(profile)) return -3;
        try { return unlocked() ? 0 : -2; } catch (RuntimeException error) { return -3; }
    }
    public synchronized int read(byte[] profile, byte[] output) {
        if (!owns(profile) || output == null || output.length != MAX_SECRET) return -3;
        Arrays.fill(output, (byte) 0); byte[] plaintext = null, record = null;
        try {
            if (!unlocked()) return -2;
            directory(true);
            try (RecordLock owned = lockRecord()) {
            directory(false); if (!anyRecord()) return -1;
            record = readRecord(); for (int i = 0; i < MAGIC.length; ++i) if (record[i] != MAGIC[i]) return -3;
            Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.DECRYPT_MODE, key(keys(), false), new GCMParameterSpec(128, record, MAGIC.length, 12));
            cipher.updateAAD(associatedData()); plaintext = cipher.doFinal(record, MAGIC.length + 12, record.length - MAGIC.length - 12);
            if (plaintext.length < 1 || plaintext.length > MAX_SECRET) return -3;
            if (!unlocked()) return -2;
            System.arraycopy(plaintext, 0, output, 0, plaintext.length); return plaintext.length;
            }
        } catch (Exception error) { return -3; }
        finally { if (plaintext != null) Arrays.fill(plaintext, (byte) 0); if (record != null) Arrays.fill(record, (byte) 0); }
    }
    public synchronized int write(byte[] profile, byte[] secret) {
        if (!owns(profile) || secret == null || secret.length < 1 || secret.length > MAX_SECRET) return -3;
        byte[] record = null, observed = null; FileOutputStream pending = null; AtomicFile atomic = null;
        try {
            if (!unlocked()) return -2;
            directory(true);
            try (RecordLock owned = lockRecord()) {
            try {
            directory(false); Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
            cipher.init(Cipher.ENCRYPT_MODE, key(keys(), true)); cipher.updateAAD(associatedData());
            byte[] iv = cipher.getIV(); if (iv == null || iv.length != 12) return -3;
            byte[] ciphertext = cipher.doFinal(secret);
            record = ByteBuffer.allocate(MAGIC.length + iv.length + ciphertext.length).put(MAGIC).put(iv).put(ciphertext).array();
            Arrays.fill(ciphertext, (byte) 0); if (record.length > MAX_RECORD) return -3; if (!unlocked()) return -2;
            atomic = new AtomicFile(mRecord); pending = atomic.startWrite(); Os.fchmod(pending.getFD(), 0600);
            pending.write(record); pending.getFD().sync(); atomic.finishWrite(pending); pending = null;
            // AtomicFile's finishWrite can log instead of throwing; observe the
            // selected ciphertext and sync its parent before acknowledging.
            directory(false); observed = readRecord(); if (!MessageDigest.isEqual(record, observed)) return -3;
            syncDirectory(mDirectory); return unlocked() ? 0 : -2;
            } finally { if (pending != null && atomic != null) { atomic.failWrite(pending); pending = null; } }
            }
        } catch (Exception error) { return -3; }
        finally { if (record != null) Arrays.fill(record, (byte) 0); if (observed != null) Arrays.fill(observed, (byte) 0); }
    }
    public synchronized int remove(byte[] profile) {
        if (!owns(profile)) return -3;
        try {
            if (!unlocked()) return -2;
            directory(true, true);
            try (RecordLock owned = lockRecord()) {
            directory(false, true); KeyStore store = keys();
            // Delete the exact wrapping key first: retained bytes after a storage
            // failure remain unreadable, and a later retry resumes exact cleanup.
            if (store.containsAlias(mAlias)) store.deleteEntry(mAlias);
            directory(false, true); new AtomicFile(mRecord).delete(); if (anyRecord()) return -3;
            syncDirectory(mDirectory); return 0;
            }
        } catch (Exception error) { return -3; }
    }
}
