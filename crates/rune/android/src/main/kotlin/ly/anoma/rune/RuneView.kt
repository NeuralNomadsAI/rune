package ly.anoma.rune

import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.view.Choreographer
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.ViewConfiguration
import android.view.WindowInsets
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager

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
    private val keyboard = context.getSystemService(InputMethodManager::class.java)

    init {
        handle = nativeNew(resources.displayMetrics.density, ViewConfiguration.getLongPressTimeout(), isDark(resources.configuration))
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

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val n = event.pointerCount
        val ids = IntArray(n) { event.getPointerId(it) }
        val xs = FloatArray(n)
        val ys = FloatArray(n)
        // Samples batched since the last frame first, so Rune measures a fling's velocity.
        if (event.actionMasked == MotionEvent.ACTION_MOVE) {
            for (h in 0 until event.historySize) {
                for (i in 0 until n) { xs[i] = event.getHistoricalX(i, h); ys[i] = event.getHistoricalY(i, h) }
                nativeTouch(handle, MotionEvent.ACTION_MOVE, 0, ids, xs, ys, event.getHistoricalEventTime(h))
            }
        }
        for (i in 0 until n) { xs[i] = event.getX(i); ys[i] = event.getY(i) }
        schedule(nativeTouch(handle, event.actionMasked, event.actionIndex, ids, xs, ys, event.eventTime))
        return true
    }

    override fun onCheckIsTextEditor() = handle != 0L && nativeWantsText(handle)

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        if (!onCheckIsTextEditor()) return null
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE or
            InputType.TYPE_TEXT_FLAG_CAP_SENTENCES or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN or EditorInfo.IME_FLAG_NO_EXTRACT_UI or
            EditorInfo.IME_ACTION_NONE
        return RuneInput()
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent) = key(event) || super.onKeyDown(keyCode, event)

    /** A key down, from a hardware keyboard or an input method; false when Rune doesn't name it. */
    private fun key(event: KeyEvent): Boolean {
        if (event.action != KeyEvent.ACTION_DOWN) return false
        val next = nativeKey(handle, event.keyCode, event.getUnicodeChar(0), event.unicodeChar, event.metaState, event.repeatCount > 0)
        if (next == UNHANDLED) return false
        schedule(next)
        return true
    }

    private fun key(code: Int) = key(KeyEvent(KeyEvent.ACTION_DOWN, code))

    /** What the input method edits: Rune's focused field, through its composition. */
    private inner class RuneInput : BaseInputConnection(this@RuneView, false) {
        override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
            schedule(nativeCommit(handle, text.toString()))
            return true
        }

        override fun setComposingText(text: CharSequence, newCursorPosition: Int): Boolean {
            schedule(nativeCompose(handle, text.toString()))
            return true
        }

        override fun finishComposingText(): Boolean {
            schedule(nativeFinishCompose(handle))
            return true
        }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            repeat(beforeLength) { key(KeyEvent.KEYCODE_DEL) }
            repeat(afterLength) { key(KeyEvent.KEYCODE_FORWARD_DEL) }
            return true
        }

        override fun sendKeyEvent(event: KeyEvent): Boolean {
            key(event)
            return true
        }

        override fun performEditorAction(actionCode: Int): Boolean = key(KeyEvent.KEYCODE_ENTER)
    }

    /** Shows, hides or restarts the keyboard as Rune's focused field asks, after a frame. */
    private fun applyKeyboard() {
        when (nativeTakeKeyboard(handle)) {
            KEYBOARD_SHOW -> {
                requestFocus()
                keyboard.restartInput(this)
                keyboard.showSoftInput(this, 0)
            }
            KEYBOARD_HIDE -> {
                keyboard.hideSoftInputFromWindow(windowToken, 0)
                keyboard.restartInput(this)
            }
            KEYBOARD_RESTART -> keyboard.restartInput(this)
        }
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
        applyKeyboard()
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

    private external fun nativeNew(density: Float, longPressMs: Int, dark: Boolean): Long
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
    private external fun nativeTouch(handle: Long, action: Int, index: Int, ids: IntArray, xs: FloatArray, ys: FloatArray, timeMs: Long): Long
    private external fun nativeKey(handle: Long, code: Int, unicode: Int, typed: Int, meta: Int, repeat: Boolean): Long
    private external fun nativeCommit(handle: Long, text: String): Long
    private external fun nativeCompose(handle: Long, text: String): Long
    private external fun nativeFinishCompose(handle: Long): Long
    private external fun nativeTakeKeyboard(handle: Long): Int
    private external fun nativeWantsText(handle: Long): Boolean
    private external fun nativeDark(handle: Long, dark: Boolean): Long
    private external fun nativeBack(handle: Long): Boolean
    private external fun nativeDrop(handle: Long)

    private companion object {
        const val UNHANDLED = -2L
        const val KEYBOARD_SHOW = 1
        const val KEYBOARD_HIDE = 2
        const val KEYBOARD_RESTART = 3
    }
}
