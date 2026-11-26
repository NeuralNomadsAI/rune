# Run a Rune app on Android

Rune's experimental Android host is a Kotlin `RuneView` (a `SurfaceView` paced by
`Choreographer`) inside `RuneActivity`, in `crates/rune/android/`. Rune draws into the view's
`ANativeWindow` through the same `LayerHost` iOS uses, over Vulkan (or GLES where Vulkan is
missing). winit isn't used on Android. The app's state lives as long as the activity: losing
the surface (Home, the app switcher, a rotation) keeps it, and the next surface picks it up.

## Build the example

Install the Android targets, an NDK (r27 or later), the SDK with platform 36, JDK 17 or newer
and Gradle 9:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
export ANDROID_NDK_HOME=/path/to/ndk ANDROID_HOME=/path/to/sdk
scripts/android-run.sh
```

The script builds `examples/android-hello` (`examples/hello` as `libmain.so`), or with
`--app gallery` `examples/android-gallery` (the rune-kit gallery), for the ABI of the device or
emulator `adb` sees, packs it into the APK of `examples/android-host` with Gradle, installs it and
starts it.

```text
usage: android-run.sh [--app hello|gallery] [--release] [--target aarch64-linux-android|x86_64-linux-android] [--build-only]
```

## Your app

Build a `cdylib` named `main` and export the function that calls `rune::run`:

```rust
rune::android_main!(main);

pub fn main() {
    rune::run(rune::AppSpec::new("Hello", (320.0, 200.0), State::default, view));
}
```

On Android `run` returns at once: `RuneView` starts the app in its first surface. Declare
`ly.anoma.rune.RuneActivity` in the manifest with the configuration changes Rune handles
(`orientation|screenSize|screenLayout|smallestScreenSize|keyboardHidden|uiMode|density`), so a
rotation resizes the surface instead of recreating the activity; `examples/android-host` is a
complete project. The `ly.anoma.rune.lib_name` meta-data names another library.

## What the host does

- **Frames**: one `Choreographer` frame per animation tick, a timed wake for the next timer, and
  none while idle. Data from another thread (the app's waker) asks for the next frame.
- **Lifecycle**: frames run only while the activity is resumed and a surface exists
  (`platform/android/lifecycle.rs`, tested on the host).
- **Density and safe area**: points are device pixels over `DisplayMetrics.density`.
  `Cx::safe_area` holds the system bars and the display cutout.
- **Keyboard**: the activity asks for `adjustNothing`, so the keyboard covers the window
  instead of resizing it, and only raises the bottom safe area. A keyboard opening in portrait
  never reads as a wide, short window. When it opens, the focused field's scroll viewports move
  just far enough to show its caret above it, as on the web; an app pads its content by
  `Cx::safe_area` so a field at the page's end has room to move.
- **Appearance**: the system's dark mode at launch and on change.
- **Touch**: every finger of a `MotionEvent` (with the samples batched since the last frame, for
  a fling's velocity) goes to Rune's touch handling: a tap clicks, a drag scrolls and flings, a
  finger held for the system's long-press delay (`ViewConfiguration.getLongPressTimeout()`, which
  the accessibility Touch & hold delay raises) is a secondary click. `Cx::touch_controls` and
  `Cx::hover_none` are true. `Tx::popup_menu` has no native menu yet and answers `None`, so an
  app draws its own (rune-kit's menus do).
- **Text input**: when a field takes text, `RuneView` offers the input method an
  `InputConnection` and shows the keyboard (and hides it when the field lets go): committed text,
  the composition (`setComposingText`, `finishComposingText`), deletions, keys and the editor
  action reach the field. The input method reads the caret's paragraph (`getTextBeforeCursor`,
  `getTextAfterCursor`, `getSelectedText`, `getExtractedText`) in UTF-16 offsets, follows the
  selection (`updateSelection`), takes text back to recompose it (`setComposingRegion`) and deletes
  around the caret by range. A keyboard's image (`commitContent`: a sticker, a GIF's first frame) is
  decoded off the UI thread and pasted as PNG. A hardware keyboard's keys arrive as Rune keys, Ctrl
  shortcuts included.
- **Back**: when something has focus, back sends Escape (closing what's open or leaving a
  field); otherwise the activity goes back.

Not yet: accessibility (TalkBack), the clipboard,
links, file pickers and system fonts.
