package ly.anoma.rune

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.PersistableBundle
import java.io.ByteArrayOutputStream
import java.io.File

/**
 * The system services Rune calls from Rust, on any thread: the clipboard, links and picked documents. One per
 * process, installed by [RuneView] before the app starts.
 */
class RuneServices private constructor(private val context: Context) {
    private val clipboard = context.getSystemService(ClipboardManager::class.java)

    init {
        clipboard.addPrimaryClipChangedListener { nativeClipboardChanged() }
        nativeInstall()
    }

    private fun item(): ClipData.Item? = try {
        clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)
    } catch (_: Exception) {
        null
    }

    @Suppress("unused")
    fun clipboardText(): String? = try {
        item()?.coerceToText(context)?.toString()
    } catch (_: Exception) {
        null
    }

    /** The clipboard's image as PNG, when it holds one. */
    @Suppress("unused")
    fun clipboardPng(): ByteArray? = try {
        val description = clipboard.primaryClipDescription
        val uri = item()?.uri
        if (uri == null || description == null || !description.hasMimeType("image/*")) {
            null
        } else {
            context.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it) }?.let { bitmap ->
                ByteArrayOutputStream().also { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
            }
        }
    } catch (_: Exception) {
        null
    }

    @Suppress("unused")
    fun setClipboardText(text: String, sensitive: Boolean): Boolean = set(ClipData.newPlainText("", text), sensitive)

    /** Copies a PNG: written to the cache and offered through [RuneClipboardProvider]. */
    @Suppress("unused")
    fun setClipboardPng(png: ByteArray): Boolean = try {
        File(context.cacheDir, RuneClipboardProvider.FILE).writeBytes(png)
        val uri = Uri.parse("content://${context.packageName}${RuneClipboardProvider.AUTHORITY_SUFFIX}/image.png")
        set(ClipData("", arrayOf("image/png"), ClipData.Item(uri)), false)
    } catch (_: Exception) {
        false
    }

    @Suppress("unused")
    fun clearClipboard(): Boolean = try {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) clipboard.clearPrimaryClip() else clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
        true
    } catch (_: Exception) {
        false
    }

    private fun set(clip: ClipData, sensitive: Boolean): Boolean = try {
        if (sensitive && Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            // "android.content.extra.IS_SENSITIVE": honoured from Android 13, ignored before.
            clip.description.extras = PersistableBundle().apply { putBoolean(IS_SENSITIVE, true) }
        }
        clipboard.setPrimaryClip(clip)
        true
    } catch (_: Exception) {
        false
    }

    /** Opens [url] in another app; null when one took it, else why none did. */
    @Suppress("unused")
    fun openUrl(url: String): String? = try {
        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        null
    } catch (_: ActivityNotFoundException) {
        "no app opens this link"
    } catch (e: Exception) {
        e.message ?: e.javaClass.simpleName
    }

    /**
     * Opens the document at [uri] to read, or to write anew, and hands its descriptor over to
     * Rust (which closes it); -1 when it can't be opened.
     */
    @Suppress("unused")
    fun openDocument(uri: String, write: Boolean): Int = try {
        context.contentResolver.openFileDescriptor(Uri.parse(uri), if (write) "wt" else "r")?.detachFd() ?: -1
    } catch (_: Exception) {
        -1
    }

    private external fun nativeInstall()
    private external fun nativeClipboardChanged()

    companion object {
        private var instance: RuneServices? = null

        /** Call after the app's native library is loaded. */
        @JvmStatic
        @Synchronized
        fun install(context: Context) {
            if (instance == null) instance = RuneServices(context.applicationContext)
        }
    }
}

private const val IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"
