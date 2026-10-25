package org.chromium.chrome.browser.omniplay;

import android.content.Context;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract.Document;
import android.provider.DocumentsContract.Root;
import android.provider.DocumentsProvider;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/**
 * Storage Access Framework (SAF) DocumentsProvider for OmniPlay game saves and exports.
 * Provides isolated access to user saves and exported archives while strictly protecting
 * internal databases, cryptographic keys, and profile tokens.
 */
public final class OmniPlayDocumentsProvider extends DocumentsProvider {

    public static final String AUTHORITY = "org.chromium.chrome.browser.omniplay.documents";

    private static final String ROOT_ID = "omniplay_root";
    private static final String DOC_ID_ROOT = "root";
    private static final String DOC_ID_SAVES = "saves";
    private static final String DOC_ID_EXPORTS = "exports";

    private static final String[] DEFAULT_ROOT_PROJECTION = new String[]{
        Root.COLUMN_ROOT_ID,
        Root.COLUMN_MIME_TYPES,
        Root.COLUMN_FLAGS,
        Root.COLUMN_ICON,
        Root.COLUMN_TITLE,
        Root.COLUMN_SUMMARY,
        Root.COLUMN_DOCUMENT_ID,
        Root.COLUMN_AVAILABLE_BYTES
    };

    private static final String[] DEFAULT_DOCUMENT_PROJECTION = new String[]{
        Document.COLUMN_DOCUMENT_ID,
        Document.COLUMN_MIME_TYPE,
        Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_LAST_MODIFIED,
        Document.COLUMN_FLAGS,
        Document.COLUMN_SIZE
    };

    private File mBaseDir;

    @Override
    public boolean onCreate() {
        Context context = getContext();
        if (context == null) return false;
        mBaseDir = new File(context.getNoBackupFilesDir(), "OmniPlay");
        return true;
    }

    @Override
    public Cursor queryRoots(String[] projection) {
        MatrixCursor result = new MatrixCursor(projection != null ? projection : DEFAULT_ROOT_PROJECTION);
        File savesDir = new File(mBaseDir, "Saves");
        long freeBytes = savesDir.getFreeSpace();

        MatrixCursor.RowBuilder row = result.newRow();
        row.add(Root.COLUMN_ROOT_ID, ROOT_ID);
        row.add(Root.COLUMN_DOCUMENT_ID, DOC_ID_ROOT);
        row.add(Root.COLUMN_TITLE, "OmniPlay Storage");
        row.add(Root.COLUMN_SUMMARY, "Game saves and exported backups");
        row.add(Root.COLUMN_FLAGS, Root.FLAG_SUPPORTS_IS_CHILD | Root.FLAG_SUPPORTS_SEARCH);
        row.add(Root.COLUMN_MIME_TYPES, "*/*");
        row.add(Root.COLUMN_AVAILABLE_BYTES, freeBytes);
        return result;
    }

    @Override
    public Cursor queryDocument(String documentId, String[] projection) throws FileNotFoundException {
        MatrixCursor result = new MatrixCursor(projection != null ? projection : DEFAULT_DOCUMENT_PROJECTION);
        includeDocument(result, documentId);
        return result;
    }

    @Override
    public Cursor queryChildDocuments(String parentDocumentId, String[] projection, String sortOrder)
            throws FileNotFoundException {
        MatrixCursor result = new MatrixCursor(projection != null ? projection : DEFAULT_DOCUMENT_PROJECTION);

        if (DOC_ID_ROOT.equals(parentDocumentId)) {
            includeDirectory(result, DOC_ID_SAVES, "Saves");
            includeDirectory(result, DOC_ID_EXPORTS, "Exports");
        } else if (DOC_ID_SAVES.equals(parentDocumentId)) {
            populateChildren(result, new File(mBaseDir, "Saves"), "saves/");
        } else if (DOC_ID_EXPORTS.equals(parentDocumentId)) {
            populateChildren(result, new File(mBaseDir, "NativeExports"), "exports/");
        }
        return result;
    }

    private void populateChildren(MatrixCursor cursor, File parentDir, String prefix) {
        if (!parentDir.exists() || !parentDir.isDirectory()) return;
        File[] files = parentDir.listFiles();
        if (files == null) return;

        for (File f : files) {
            if (f.getName().startsWith(".")) continue; // skip hidden files
            if (f.isFile()) {
                MatrixCursor.RowBuilder row = cursor.newRow();
                row.add(Document.COLUMN_DOCUMENT_ID, prefix + f.getName());
                row.add(Document.COLUMN_DISPLAY_NAME, f.getName());
                row.add(Document.COLUMN_SIZE, f.length());
                row.add(Document.COLUMN_MIME_TYPE, getMimeTypeForFile(f));
                row.add(Document.COLUMN_LAST_MODIFIED, f.lastModified());
                row.add(Document.COLUMN_FLAGS, Document.FLAG_SUPPORTS_DELETE | Document.FLAG_SUPPORTS_WRITE);
            }
        }
    }

    private void includeDirectory(MatrixCursor cursor, String docId, String displayName) {
        MatrixCursor.RowBuilder row = cursor.newRow();
        row.add(Document.COLUMN_DOCUMENT_ID, docId);
        row.add(Document.COLUMN_DISPLAY_NAME, displayName);
        row.add(Document.COLUMN_SIZE, 0);
        row.add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR);
        row.add(Document.COLUMN_LAST_MODIFIED, System.currentTimeMillis());
        row.add(Document.COLUMN_FLAGS, 0);
    }

    private void includeDocument(MatrixCursor cursor, String documentId) throws FileNotFoundException {
        MatrixCursor.RowBuilder row = cursor.newRow();
        row.add(Document.COLUMN_DOCUMENT_ID, documentId);

        if (DOC_ID_ROOT.equals(documentId)) {
            row.add(Document.COLUMN_DISPLAY_NAME, "OmniPlay");
            row.add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR);
            row.add(Document.COLUMN_FLAGS, 0);
        } else if (DOC_ID_SAVES.equals(documentId)) {
            row.add(Document.COLUMN_DISPLAY_NAME, "Saves");
            row.add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR);
            row.add(Document.COLUMN_FLAGS, 0);
        } else if (DOC_ID_EXPORTS.equals(documentId)) {
            row.add(Document.COLUMN_DISPLAY_NAME, "Exports");
            row.add(Document.COLUMN_MIME_TYPE, Document.MIME_TYPE_DIR);
            row.add(Document.COLUMN_FLAGS, 0);
        } else {
            File target = resolveDocumentFile(documentId);
            if (!target.exists() || !target.isFile()) throw new FileNotFoundException("Document not found: " + documentId);
            row.add(Document.COLUMN_DISPLAY_NAME, target.getName());
            row.add(Document.COLUMN_SIZE, target.length());
            row.add(Document.COLUMN_MIME_TYPE, getMimeTypeForFile(target));
            row.add(Document.COLUMN_LAST_MODIFIED, target.lastModified());
            row.add(Document.COLUMN_FLAGS, Document.FLAG_SUPPORTS_DELETE | Document.FLAG_SUPPORTS_WRITE);
        }
    }

    private File resolveDocumentFile(String documentId) throws FileNotFoundException {
        if (documentId == null || documentId.contains("..")) {
            throw new FileNotFoundException("Invalid document ID");
        }
        final File allowedDir;
        final String name;
        if (documentId.startsWith("saves/")) {
            allowedDir = new File(mBaseDir, "Saves");
            name = documentId.substring("saves/".length());
        } else if (documentId.startsWith("exports/")) {
            allowedDir = new File(mBaseDir, "NativeExports");
            name = documentId.substring("exports/".length());
        } else {
            throw new FileNotFoundException("Unrecognized document: " + documentId);
        }

        if (name.isEmpty() || name.startsWith(".") || name.contains("/") || name.contains("\\")) {
            throw new FileNotFoundException("Rejected unsafe document filename: " + documentId);
        }

        File target = new File(allowedDir, name);
        try {
            String canonicalAllowed = allowedDir.getCanonicalPath() + File.separator;
            String canonicalTarget = target.getCanonicalPath();
            if (!canonicalTarget.startsWith(canonicalAllowed)) {
                throw new FileNotFoundException("Document escaped sandbox boundary: " + documentId);
            }
        } catch (IOException e) {
            throw new FileNotFoundException("Failed to canonicalize document path: " + documentId);
        }
        return target;
    }

    @Override
    public ParcelFileDescriptor openDocument(String documentId, String mode, CancellationSignal signal)
            throws FileNotFoundException {
        File file = resolveDocumentFile(documentId);
        int accessMode = ParcelFileDescriptor.parseMode(mode);
        return ParcelFileDescriptor.open(file, accessMode);
    }

    @Override
    public void deleteDocument(String documentId) throws FileNotFoundException {
        File file = resolveDocumentFile(documentId);
        if (!file.delete()) {
            throw new FileNotFoundException("Failed to delete document: " + documentId);
        }
    }

    @Override
    public String getDocumentType(String documentId) throws FileNotFoundException {
        if (DOC_ID_ROOT.equals(documentId) || DOC_ID_SAVES.equals(documentId) || DOC_ID_EXPORTS.equals(documentId)) {
            return Document.MIME_TYPE_DIR;
        }
        File file = resolveDocumentFile(documentId);
        return getMimeTypeForFile(file);
    }

    private static String getMimeTypeForFile(File file) {
        String name = file.getName().toLowerCase();
        if (name.endsWith(".sav") || name.endsWith(".srm")) return "application/x-game-save";
        if (name.endsWith(".json")) return "application/json";
        if (name.endsWith(".zip") || name.endsWith(".tar") || name.endsWith(".gz")) return "application/zip";
        return "application/octet-stream";
    }
}
