package ly.anoma.rune

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import java.io.File

/**
 * Serves the image Rune last copied to whoever pastes it. Declare it in the manifest with the
 * authority `${applicationId}.rune.clipboard`, `exported="false"` and
 * `grantUriPermissions="true"`: the clipboard grants the app that pastes a read of that one
 * file, and no other app can open it.
 */
class RuneClipboardProvider : ContentProvider() {
    override fun onCreate() = true

    override fun getType(uri: Uri) = "image/png"

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor? {
        val context = context ?: return null
        return ParcelFileDescriptor.open(File(context.cacheDir, FILE), ParcelFileDescriptor.MODE_READ_ONLY)
    }

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?) = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?) = 0

    companion object {
        const val FILE = "rune-clipboard.png"
        const val AUTHORITY_SUFFIX = ".rune.clipboard"
    }
}