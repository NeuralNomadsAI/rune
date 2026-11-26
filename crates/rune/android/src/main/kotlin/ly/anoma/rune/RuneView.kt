package ly.anoma.rune

import android.content.Context
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Build
import android.os.Bundle
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
import android.view.accessibility.AccessibilityManager
import android.view.accessibility.AccessibilityNodeInfo
import android.view.accessibility.AccessibilityNodeProvider
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.ExtractedText
import android.view.inputmethod.ExtractedTextRequest
import android.view.inputmethod.InputContentInfo
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
    private val selectionUi = RuneSelection(this, object : RuneSelection.Events {
        override fun handle(which: Int, phase: Int, x: Float, y: Float, fingerY: Float) = schedule(nativeHandle(handle, which, phase, x, y, fingerY))
        override fun picked(index: Int) = schedule(nativeToolbarPicked(handle, index))
    })

    init {
        RuneServices.install(context)
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
        selectionUi.hide()
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
        if (event.actionMasked == MotionEvent.ACTION_UP && event.eventTime - event.downTime < ViewConfiguration.getLongPressTimeout()) selectionUi.tapped()
        return true
    }

    /** TalkBack's view of the app: Rune's accessibility tree, one virtual node per element. */
    private val nodes = object : AccessibilityNodeProvider() {
        override fun createAccessibilityNodeInfo(virtualViewId: Int): AccessibilityNodeInfo? =
            if (handle == 0L) null else nativeA11yNodeInfo(handle, virtualViewId)?.also {
                // The root is this view, focusable for keys: TalkBack would take it for one
                // control and read its name in place of every text inside.
                if (virtualViewId == AccessibilityNodeProvider.HOST_VIEW_ID) {
                    it.isFocusable = false
                    it.removeAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_FOCUS)
                    it.removeAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK)
                } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P && it.childCount == 0 && !it.contentDescription.isNullOrEmpty()) {
                    // A text of its own, where TalkBack would read a scrolled page's as one.
                    it.isScreenReaderFocusable = true
                }
            }

        override fun findFocus(focus: Int): AccessibilityNodeInfo? =
            if (handle == 0L) null else nativeA11yFindFocus(handle, focus)

        override fun performAction(virtualViewId: Int, action: Int, arguments: Bundle?): Boolean =
            handle != 0L && nativeA11yPerformAction(handle, virtualViewId, action, arguments)
    }

    override fun getAccessibilityNodeProvider(): AccessibilityNodeProvider? =
        if (handle != 0L && nativeA11yAvailable(handle)) nodes else super.getAccessibilityNodeProvider()

    private val accessibility = context.getSystemService(AccessibilityManager::class.java)

    /**
     * Explore by touch: the node under the finger takes accessibility focus. A mouse's hover without
     * a screen reader stays out of it, as looking a node up builds the whole tree.
     */
    override fun dispatchHoverEvent(event: MotionEvent): Boolean =
        (handle != 0L && accessibility.isTouchExplorationEnabled && nativeA11yHover(handle, event.action, event.x, event.y)) ||
            super.dispatchHoverEvent(event)

    override fun onCheckIsTextEditor() = handle != 0L && nativeWantsText(handle)

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection? {
        if (!onCheckIsTextEditor()) return null
        val editor = nativeEditor(handle)
        val (secure, oneLine) = (editor and EDITOR_SECURE != 0) to (editor and EDITOR_ONE_LINE != 0)
        // A password is not corrected, suggested or learnt; a one-line field has an action key
        // (sent to Rune as Return, which submits) where a longer one has Return.
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or when {
            secure -> InputType.TYPE_TEXT_VARIATION_PASSWORD
            oneLine -> InputType.TYPE_TEXT_FLAG_CAP_SENTENCES or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
            else -> InputType.TYPE_TEXT_FLAG_MULTI_LINE or InputType.TYPE_TEXT_FLAG_CAP_SENTENCES or InputType.TYPE_TEXT_FLAG_AUTO_CORRECT
        }
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN or EditorInfo.IME_FLAG_NO_EXTRACT_UI or
            (if (secure) EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING else 0) or
            (if (oneLine) EditorInfo.IME_ACTION_DONE else EditorInfo.IME_ACTION_NONE)
        val selection = nativeImeSelection(handle)
        if (selection >= 0) {
            outAttrs.initialSelStart = (selection ushr 32).toInt()
            outAttrs.initialSelEnd = selection.toInt()
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N_MR1) outAttrs.contentMimeTypes = IMAGE_TYPES
        return RuneInput()
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent) = key(event) || super.onKeyDown(keyCode, event)

    /** A key down, from a hardware keyboard or an input method; false when Rune doesn't name it. */
    private fun key(event: KeyEvent): Boolean {
        if (event.action != KeyEvent.ACTION_DOWN) return false
        val next = nativeKey(handle, event.keyCode, event.getUnicodeChar(0), event.unicodeChar, event.metaState, event.repeatCount > 0)
        if (next == UNHANDLED) return false
        selectionUi.typed()
        schedule(next)
        return true
    }

    private fun key(code: Int) = key(KeyEvent(KeyEvent.ACTION_DOWN, code))

    /** What the input method edits: Rune's focused field, through its composition. */
    private inner class RuneInput : BaseInputConnection(this@RuneView, false) {
        override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
            selectionUi.typed()
            schedule(nativeCommit(handle, text.toString()))
            return true
        }

        override fun setComposingText(text: CharSequence, newCursorPosition: Int): Boolean {
            selectionUi.typed()
            schedule(nativeCompose(handle, text.toString()))
            return true
        }

        override fun finishComposingText(): Boolean {
            schedule(nativeFinishCompose(handle))
            return true
        }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            val next = nativeDeleteSurrounding(handle, beforeLength, afterLength)
            if (next == UNHANDLED) {
                // No text to count in (mid-composition, a secret field): keys instead.
                repeat(beforeLength) { key(KeyEvent.KEYCODE_DEL) }
                repeat(afterLength) { key(KeyEvent.KEYCODE_FORWARD_DEL) }
            } else {
                schedule(next)
            }
            return true
        }

        override fun setComposingRegion(start: Int, end: Int): Boolean {
            schedule(nativeComposingRegion(handle, start, end))
            return true
        }

        override fun getTextBeforeCursor(n: Int, flags: Int): CharSequence? {
            val text = nativeImeText(handle) ?: return null
            val start = selectionStart()
            return text.substring((start - n).coerceAtLeast(0), start)
        }

        override fun getTextAfterCursor(n: Int, flags: Int): CharSequence? {
            val text = nativeImeText(handle) ?: return null
            val end = selectionEnd()
            return text.substring(end, (end + n).coerceAtMost(text.length))
        }

        override fun getSelectedText(flags: Int): CharSequence? {
            val text = nativeImeText(handle) ?: return null
            val (start, end) = selectionStart() to selectionEnd()
            return if (start == end) null else text.substring(start, end)
        }

        override fun getExtractedText(request: ExtractedTextRequest?, flags: Int): ExtractedText? {
            val text = nativeImeText(handle) ?: return null
            return ExtractedText().apply {
                this.text = text
                startOffset = 0
                partialStartOffset = -1
                partialEndOffset = -1
                selectionStart = selectionStart()
                selectionEnd = selectionEnd()
            }
        }

        /** A keyboard's image (a sticker, a GIF's first frame): decoded off the UI thread, pasted as PNG. */
        override fun commitContent(content: InputContentInfo, flags: Int, opts: Bundle?): Boolean {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.N_MR1) return false
            if (flags and InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION != 0) {
                try { content.requestPermission() } catch (_: Exception) { return false }
            }
            Thread {
                val png = try {
                    context.contentResolver.openInputStream(content.contentUri)?.use { BitmapFactory.decodeStream(it) }?.let { bitmap ->
                        java.io.ByteArrayOutputStream().also { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
                    }
                } catch (_: Exception) {
                    null
                } finally {
                    content.releasePermission()
                }
                if (png != null) post { if (handle != 0L) schedule(nativePastePng(handle, png)) }
            }.start()
            return true
        }

        private fun selectionStart() = nativeImeSelection(handle).let { if (it < 0) 0 else (it ushr 32).toInt() }
        private fun selectionEnd() = nativeImeSelection(handle).let { if (it < 0) 0 else it.toInt() }

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
            KEYBOARD_SELECT -> nativeImeSelection(handle).let {
                if (it >= 0) keyboard.updateSelection(this, (it ushr 32).toInt(), it.toInt(), -1, -1)
            }
        }
    }

    fun resume() {
        schedule(nativeResumed(handle))
        selectionUi.show()
    }

    fun pause() {
        selectionUi.hide()
        cancelFrames()
        nativePaused(handle)
    }

    /** The system back gesture; false when Rune had nothing to close or leave. */
    fun back(): Boolean = nativeBack(handle).also { if (it) schedule(0) }

    fun configurationChanged(config: Configuration) = schedule(nativeDark(handle, isDark(config)))

    /** Frees the app; the view is unusable afterwards. */
    fun destroy() {
        selectionUi.hide()
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
        nativeTakeHandles(handle)?.let(selectionUi::place)
        nativeTakeToolbar(handle)?.let(selectionUi::toolbar)
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
    private external fun nativeComposingRegion(handle: Long, start: Int, end: Int): Long
    private external fun nativeDeleteSurrounding(handle: Long, before: Int, after: Int): Long
    private external fun nativeImeText(handle: Long): String?
    private external fun nativeImeSelection(handle: Long): Long
    private external fun nativePastePng(handle: Long, png: ByteArray): Long
    private external fun nativeEditor(handle: Long): Int
    private external fun nativeTakeKeyboard(handle: Long): Int
    private external fun nativeWantsText(handle: Long): Boolean
    private external fun nativeA11yAvailable(handle: Long): Boolean
    private external fun nativeA11yNodeInfo(handle: Long, id: Int): AccessibilityNodeInfo?
    private external fun nativeA11yFindFocus(handle: Long, focus: Int): AccessibilityNodeInfo?
    private external fun nativeA11yPerformAction(handle: Long, id: Int, action: Int, arguments: Bundle?): Boolean
    private external fun nativeA11yHover(handle: Long, action: Int, x: Float, y: Float): Boolean
    private external fun nativeTakeHandles(handle: Long): FloatArray?
    private external fun nativeTakeToolbar(handle: Long): String?
    private external fun nativeHandle(handle: Long, which: Int, phase: Int, x: Float, y: Float, fingerY: Float): Long
    private external fun nativeToolbarPicked(handle: Long, index: Int): Long
    private external fun nativeDark(handle: Long, dark: Boolean): Long
    private external fun nativeBack(handle: Long): Boolean
    private external fun nativeDrop(handle: Long)

    private companion object {
        const val UNHANDLED = -2L
        const val KEYBOARD_SHOW = 1
        const val KEYBOARD_HIDE = 2
        const val KEYBOARD_RESTART = 3
        const val KEYBOARD_SELECT = 4
        const val EDITOR_SECURE = 1
        const val EDITOR_ONE_LINE = 2
        val IMAGE_TYPES = arrayOf("image/png", "image/gif", "image/jpeg", "image/webp")
    }
}
