package ly.anoma.rune

import android.graphics.Rect
import android.graphics.drawable.Drawable
import android.os.Build
import android.os.SystemClock
import android.view.ActionMode
import android.view.Gravity
import android.view.Menu
import android.view.MenuItem
import android.view.MotionEvent
import android.view.View
import android.view.ViewConfiguration
import android.view.WindowManager
import android.widget.ImageView
import android.widget.Magnifier
import android.widget.PopupWindow
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * What [RuneView] shows over selected text, as Android's own text does: the system's handles on
 * the selection's ends (or under a field's caret after a tap), the floating toolbar with the
 * text's menu and, while a handle is dragged, the magnifier.
 */
internal class RuneSelection(private val view: View, private val events: Events) {
    interface Events {
        /**
         * A handle's finger: [phase] 0 down, 1 moved, 2 up, 3 up with no toolbar to bring; where its
         * end goes and the finger's own height, in the view's pixels.
         */
        fun handle(which: Int, phase: Int, x: Float, y: Float, fingerY: Float)

        /** The toolbar closed by itself, with the item at [index] picked or none (-1). */
        fun picked(index: Int)
    }

    private val start = Handle(START, android.R.attr.textSelectHandleLeft, 0.75f)
    private val end = Handle(END, android.R.attr.textSelectHandleRight, 0.25f)
    private val caret = Handle(CARET, android.R.attr.textSelectHandle, 0.5f)
    private val content = Rect()
    private var ends: FloatArray? = null
    private var toolbar: ActionMode? = null
    private var magnifier: Magnifier? = null
    /** Until when the caret's handle shows (`uptimeMillis`): a tap brings it, typing takes it away. */
    private var caretUntil = 0L
    private val caretOff = Runnable { ends?.let(::place) }
    private val slop = ViewConfiguration.get(view.context).scaledTouchSlop

    /** The ends moved: the kind (0 none, 1 caret, 2 range), then each end's x, top, bottom, shown. */
    fun place(a: FloatArray) {
        ends = a
        val kind = a[0].toInt()
        start.show(kind == RANGE && a[4] != 0f, a[1], a[2], a[3])
        end.show(kind == RANGE && a[8] != 0f, a[5], a[6], a[7])
        caret.show(kind == POINT && a[4] != 0f && (caret.held || SystemClock.uptimeMillis() < caretUntil), a[1], a[2], a[3])
        // One line: its span. More: the view's width, as the lines between run to its edges.
        val oneLine = a[6] < a[3]
        content.set(
            if (oneLine) min(a[1], a[5]).roundToInt() else 0, a[2].roundToInt(),
            if (oneLine) max(a[1], a[5]).roundToInt() else view.width, a[7].roundToInt() + end.height,
        )
        toolbar?.invalidateContentRect()
        val held = arrayOf(start, end, caret).firstOrNull { it.held }
        if (held != null && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            @Suppress("DEPRECATION")
            val glass = magnifier ?: Magnifier(view).also { magnifier = it }
            glass.show(held.x, held.middle)
        }
    }

    /** A tap in the view: a field's caret gets its handle for a while. */
    fun tapped() {
        caretUntil = SystemClock.uptimeMillis() + CARET_MS
        view.removeCallbacks(caretOff)
        view.postDelayed(caretOff, CARET_MS)
        ends?.let(::place)
    }

    fun typed() {
        if (caretUntil == 0L) return
        caretUntil = 0L
        ends?.let(::place)
    }

    /** Opens the toolbar with [items] (`id UNIT title` joined by `RECORD`), or shuts it when empty. */
    fun toolbar(items: String) {
        toolbar.also { toolbar = null }?.finish()
        if (items.isEmpty()) return
        val list = items.split(RECORD).map { it.split(UNIT, limit = 2) }.filter { it.size == 2 }
        toolbar = view.startActionMode(object : ActionMode.Callback2() {
            override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
                list.forEachIndexed { i, (id, title) ->
                    menu.add(Menu.NONE, i, i, systemTitle(id) ?: title).setShowAsAction(MenuItem.SHOW_AS_ACTION_IF_ROOM)
                }
                return true
            }

            override fun onPrepareActionMode(mode: ActionMode, menu: Menu) = false

            override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean {
                toolbar = null
                events.picked(item.itemId)
                mode.finish()
                return true
            }

            /** Shut by the system (back, the window losing focus): Rune's own shuts clear [toolbar] first. */
            override fun onDestroyActionMode(mode: ActionMode) {
                if (toolbar !== mode) return
                toolbar = null
                events.picked(-1)
            }

            override fun onGetContentRect(mode: ActionMode, view: View, outRect: Rect) = outRect.set(content)
        }, ActionMode.TYPE_FLOATING)
        // Refused (a view without a window): the menu is answered, not left open.
        if (toolbar == null) view.post { events.picked(-1) }
    }

    /** The view lost its surface or went to the background: nothing stays over it until [show]. */
    fun hide() {
        view.removeCallbacks(caretOff)
        arrayOf(start, end, caret).forEach { it.release() }
        toolbar.also { toolbar = null }?.let {
            it.finish()
            events.picked(-1)
        }
    }

    /** The view is back: the handles return to the selection it kept. */
    fun show() {
        ends?.let(::place)
    }

    /** Android's own word for a standard item, in the system's language. */
    private fun systemTitle(id: String): CharSequence? = when (id) {
        "cut" -> android.R.string.cut
        "copy" -> android.R.string.copy
        "paste" -> android.R.string.paste
        "select-all" -> android.R.string.selectAll
        else -> null
    }?.let(view.context::getText)

    private fun themed(attr: Int): Drawable? {
        val a = view.context.theme.obtainStyledAttributes(intArrayOf(attr))
        try { return a.getDrawable(0) } finally { a.recycle() }
    }

    /**
     * One handle: the theme's drawable in a window of its own, whose [hot] fraction of the width
     * points at the end. The drawable is some 22dp of ink; the window around it is a finger's
     * target, as the system's own handles have.
     */
    private inner class Handle(private val which: Int, attr: Int, private val hot: Float) {
        private val icon = themed(attr)
        private val width = icon?.intrinsicWidth ?: 0
        val height = icon?.intrinsicHeight ?: 0
        private val target = (TARGET_DP * view.resources.displayMetrics.density).roundToInt()
        /** Blank on each side of the drawable, and below it. */
        private val side = max(0, target - width) / 2
        private val below = max(0, target - height)
        private val window = PopupWindow(ImageView(view.context).apply {
            setImageDrawable(icon)
            scaleType = ImageView.ScaleType.CENTER
            setPadding(side, 0, side, below)
            setOnTouchListener { _, event -> touch(event) }
        }, width + 2 * side, height + below).apply {
            isClippingEnabled = false
            windowLayoutType = WindowManager.LayoutParams.TYPE_APPLICATION_SUB_PANEL
        }
        var held = false
            private set
        /** The end's place in the view: its x and the middle of its line. */
        var x = 0f
            private set
        var middle = 0f
            private set
        /** From the end's place to the finger that took the handle. */
        private var dx = 0f
        private var dy = 0f
        /** Where the finger went down on the screen, and whether it has left the touch slop since. */
        private var downX = 0f
        private var downY = 0f
        private var moved = false

        fun show(shown: Boolean, x: Float, top: Float, bottom: Float) {
            this.x = x
            middle = (top + bottom) / 2
            // A held handle keeps its window, which is what its finger's moves come through.
            if (!shown && held) return
            if (!shown || icon == null || !view.isAttachedToWindow) return window.dismiss()
            val at = IntArray(2).also(view::getLocationInWindow)
            val (left, y) = at[0] + (x - width * hot).roundToInt() - side to at[1] + bottom.roundToInt()
            if (window.isShowing) window.update(left, y, -1, -1) else window.showAtLocation(view, Gravity.NO_GRAVITY, left, y)
        }

        fun release() {
            held = false
            window.dismiss()
        }

        private fun touch(event: MotionEvent): Boolean {
            val at = IntArray(2).also(view::getLocationOnScreen)
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    held = true
                    moved = false
                    downX = event.rawX
                    downY = event.rawY
                    dx = event.rawX - at[0] - x
                    dy = event.rawY - at[1] - middle
                    events.handle(which, 0, x, middle, event.rawY - at[1])
                }
                MotionEvent.ACTION_MOVE -> {
                    moved = moved || hypot(event.rawX - downX, event.rawY - downY) > slop
                    events.handle(which, 1, event.rawX - at[0] - dx, event.rawY - at[1] - dy, event.rawY - at[1])
                }
                MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    held = false
                    magnifier.also { magnifier = null }?.dismiss()
                    if (which == CARET) tapped()
                    // A caret's handle shows its toolbar when tapped, not after a drag.
                    events.handle(which, if (which == CARET && moved) 3 else 2, 0f, 0f, 0f)
                }
            }
            return true
        }
    }

    private companion object {
        const val START = 0
        const val END = 1
        const val CARET = 2
        /** The kinds of ends: a field's caret alone, or a selection's two. */
        const val POINT = 1
        const val RANGE = 2
        const val CARET_MS = 4000L
        /** The least width and height of a handle's touch target. */
        const val TARGET_DP = 48
        const val UNIT = '\u001f'
        const val RECORD = '\u001e'
    }
}
