package ly.anoma.rune

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import android.window.OnBackInvokedDispatcher

/**
 * An activity showing one [RuneView]. It loads the native library named by the
 * `ly.anoma.rune.lib_name` meta-data (default `main`), whose `rune::android_main!` function
 * starts the app. The manifest should declare the configuration changes Rune handles itself
 * (`orientation|screenSize|screenLayout|smallestScreenSize|keyboardHidden|uiMode|density`), so
 * a rotation doesn't recreate the activity.
 */
open class RuneActivity : Activity() {
    protected lateinit var view: RuneView
    /** The file dialog the document picker is open for. */
    private var picking = 0L

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        System.loadLibrary(libraryName())
        // The keyboard covers the window instead of resizing it; Rune raises its bottom safe area.
        window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) window.setDecorFitsSystemWindows(false)
        view = RuneView(this)
        view.filePicker = { id, intent ->
            picking = id
            try {
                @Suppress("DEPRECATION")
                startActivityForResult(intent, PICK_REQUEST)
                true
            } catch (_: ActivityNotFoundException) {
                false
            }
        }
        setContentView(view)
        view.requestFocus()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT) { goBack() }
        }
    }

    @Deprecated("Rune handles back through OnBackInvokedDispatcher on API 33 and later")
    override fun onBackPressed() = goBack()

    private fun goBack() {
        if (view.back()) return
        if (isTaskRoot) moveTaskToBack(true) else finish()
    }

    /** The document picker closed: its documents, one or several, or none when backed out of. */
    @Deprecated("Rune's picker goes through startActivityForResult, which needs no AndroidX")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != PICK_REQUEST) return
        val uris = if (resultCode != RESULT_OK || data == null) null else
            data.clipData?.let { clip -> List(clip.itemCount) { clip.getItemAt(it).uri } } ?: listOfNotNull(data.data)
        view.filesPicked(picking, uris?.takeIf { it.isNotEmpty() })
    }

    override fun onResume() {
        super.onResume()
        view.resume()
    }

    override fun onPause() {
        view.pause()
        super.onPause()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        view.configurationChanged(newConfig)
    }

    override fun onDestroy() {
        view.destroy()
        super.onDestroy()
    }

    private fun libraryName(): String {
        val info = packageManager.getActivityInfo(componentName, PackageManager.GET_META_DATA)
        return info.metaData?.getString("ly.anoma.rune.lib_name") ?: "main"
    }

    private companion object {
        const val PICK_REQUEST = 0x5255
    }
}
