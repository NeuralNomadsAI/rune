package ly.anoma.rune

import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.view.Choreographer
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.WindowInsets

/**
 * A Rune app's view: a [SurfaceView] Rune draws into, paced by [Choreographer]. Rune keeps the
 * app's state across surfaces, so pausing, rotating or leaving for the launcher loses nothing.
 * The app's library must already be loaded (see [RuneActivity]).
 */
class RuneView(context: Context) : SurfaceView(context), SurfaceHolder.Callback, Choreographer.FrameCallback {
    private var handle = 0L
    private var hasSurface = false
    private var frameQueued = false
    private val choreographer = Choreographer.getInstance()
    private val main = Handler(Looper.getMainLooper())
    private val later = Runnable { schedule(0) }

    init {
        handle = nativeNew(resources.displayMetrics.density, isDark(resources.configuration))
        check(handle != 0L) { "rune: the app didn't start (see logcat)" }
        holder.addCallback(this)
        isFocusable = true
        isFocusableInTouchMode = true
        setOnApplyWindowInsetsListener { _, insets -> applyInsets(insets); insets }
    }

    override fun surfaceCreated(holder: SurfaceHolder) {}

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        if (!hasSurface) {
            hasSurface = true
            schedule(nativeSurfaceCreated(handle, holder.surface, width, height))
        } else {
            schedule(nativeSurfaceChanged(handle, width, height))
        }
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        hasSurface = false
        cancelFrames()
        nativeSurfaceDestroyed(handle)
    }

    override fun doFrame(frameTimeNanos: Long) {
        frameQueued = false
        schedule(nativeFrame(handle))
    }

    fun resume() = schedule(nativeResumed(handle))

    fun pause() {
        cancelFrames()
        nativePaused(handle)
    }

    /** The system back gesture; false when Rune had nothing to close or leave. */
    fun back(): Boolean = nativeBack(handle).also { if (it) schedule(0) }

    fun configurationChanged(config: Configuration) = schedule(nativeDark(handle, isDark(config)))

    /** Frees the app; the view is unusable afterwards. */
    fun destroy() {
        cancelFrames()
        nativeDrop(handle)
        handle = 0L
    }

    /** New data from any thread (Rune's waker): draw on the next frame. */
    @Suppress("unused")
    fun requestFrame() {
        post { schedule(0) }
    }

    private fun applyInsets(insets: WindowInsets) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
            val ime = insets.getInsets(WindowInsets.Type.ime())
            schedule(nativeInsets(handle,
                bars.top.toFloat(), bars.right.toFloat(), bars.bottom.toFloat(), bars.left.toFloat(),
                ime.top.toFloat(), ime.right.toFloat(), ime.bottom.toFloat(), ime.left.toFloat()))
        } else {
            @Suppress("DEPRECATION")
            schedule(nativeInsets(handle,
                insets.systemWindowInsetTop.toFloat(), insets.systemWindowInsetRight.toFloat(),
                insets.systemWindowInsetBottom.toFloat(), insets.systemWindowInsetLeft.toFloat(),
                0f, 0f, 0f, 0f))
        }
    }

    private fun schedule(next: Long) {
        if (handle == 0L) return
        when {
            next == 0L -> if (!frameQueued) {
                frameQueued = true
                choreographer.postFrameCallback(this)
            }
            next > 0L -> {
                main.removeCallbacks(later)
                main.postDelayed(later, next)
            }
        }
    }

    private fun cancelFrames() {
        choreographer.removeFrameCallback(this)
        frameQueued = false
        main.removeCallbacks(later)
    }

    private fun isDark(config: Configuration) =
        config.uiMode and Configuration.UI_MODE_NIGHT_MASK == Configuration.UI_MODE_NIGHT_YES

    private external fun nativeNew(density: Float, dark: Boolean): Long
    private external fun nativeSurfaceCreated(handle: Long, surface: android.view.Surface, width: Int, height: Int): Long
    private external fun nativeSurfaceChanged(handle: Long, width: Int, height: Int): Long
    private external fun nativeSurfaceDestroyed(handle: Long)
    private external fun nativeResumed(handle: Long): Long
    private external fun nativePaused(handle: Long)
    private external fun nativeFrame(handle: Long): Long
    private external fun nativeInsets(
        handle: Long,
        barTop: Float, barRight: Float, barBottom: Float, barLeft: Float,
        imeTop: Float, imeRight: Float, imeBottom: Float, imeLeft: Float,
    ): Long
    private external fun nativeDark(handle: Long, dark: Boolean): Long
    private external fun nativeBack(handle: Long): Boolean
    private external fun nativeDrop(handle: Long)
}
