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
`--app gallery` `examples/android-gallery` (the rune-kit gallery), or with `--app fields`
`examples/android-fields` (a field of each kind, to check its keyboard), for the ABI of the device or
emulator `adb` sees, packs it into the APK of `examples/android-host` with Gradle, installs it and
starts it.

```text
usage: android-run.sh [--app hello|gallery|fields] [--release | --profile NAME] [--target aarch64-linux-android|x86_64-linux-android] [--build-only]
```

Take numbers from `--profile perf`, as the other benchmarks do. `adb shell dumpsys SurfaceFlinger
--latency` on the app's `SurfaceView` layer gives each frame's present time. An emulator's
numbers say nothing of a device's.

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

The macro works in a crate that has `#![forbid(unsafe_code)]`. An app that turns on Rune's `avif`
feature links its x86_64 library (an emulator's) with `-Wl,-Bsymbolic`, from its build script
(`cargo::rustc-link-arg=-Wl,-Bsymbolic`): the decoder's x86 assembly addresses its exported
tables directly, which a shared library otherwise refuses (`relocation R_X86_64_PC32 cannot be
used against symbol`).

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
  just far enough to show its caret 16 points above it (so the field's padding and border show
  too), as on the web; an app pads its content by
  `Cx::safe_area` so a field at the page's end has room to move.
- **Appearance**: the system's dark mode at launch and on change.
- **Touch**: every finger of a `MotionEvent` (with the samples batched since the last frame, for
  a fling's velocity) goes to Rune's touch handling: a tap clicks, a drag scrolls and flings, a
  finger held for the system's long-press delay (`ViewConfiguration.getLongPressTimeout()`, which
  the accessibility Touch & hold delay raises) is a secondary click. As on every touch host, a
  finger moves focus and selection when it lifts as a tap, so a scroll leaves a field its focus
  and the keyboard up. `Cx::touch_controls` and
  `Cx::hover_none` are true. `Tx::popup_menu` has no native menu and answers `None`, so an app
  draws its own (rune-kit's menus do).
- **Text selection**: a long press on selectable text or in a field selects the word under it,
  as Android's text does. `RuneView` hangs the system's handles on the selection's ends and shows
  the text's own menu (Cut, Copy, Paste, Select all, in the system's words) as the floating
  toolbar, even where the app has `App::on_field_menu`. A second tap's word gets them too; a long
  press in the blank beside or below a field's text places the caret there and offers Paste. A
  handle drags its end as far as the other, with the system's magnifier over the line (Android
  9+); held at the top or bottom of the viewport around selectable text or a field of several
  lines, it scrolls it. A tap in a field
  puts a handle under its caret for four seconds: it drags the caret, and a tap on it shows the
  toolbar. A finger changes the selection when it lifts as a tap, so a scroll under selected text
  keeps it; its toolbar goes while the scroll lasts. A field with its own `on_secondary_press`
  shows that on a long press instead. A one-line field's handles don't scroll it sideways, the handles keep their
  left-to-right shapes in right-to-left text, and a selection reaching into rows a virtual list
  has unmounted keeps its ends.
- **Text input**: when a field takes text, `RuneView` offers the input method an
  `InputConnection` and shows the keyboard (and hides it when the field lets go): committed text,
  the composition (`setComposingText`, `finishComposingText`), deletions, keys and the editor
  action reach the field. The input method reads the caret's paragraph (`getTextBeforeCursor`,
  `getTextAfterCursor`, `getSelectedText`, `getExtractedText`) in UTF-16 offsets, follows the
  selection (`updateSelection`), takes text back to recompose it (`setComposingRegion`) and deletes
  around the caret by range. A keyboard's image (`commitContent`: a sticker, a GIF's first frame) is
  decoded off the UI thread and pasted as PNG. A hardware keyboard's keys arrive as Rune keys, Ctrl
  shortcuts included. A tap on a field brings the keyboard back when the user had put it away
  (Back hides it and leaves the field its focus). The keyboard is told what the field is
  (`EditorInfo`) and restarted when
  focus goes to another kind: a one-line field has an action key, which submits as Return does,
  where a longer one has Return; a secret field (`El::secure`) gets the keyboard as a password's,
  with no correction, suggestion or learning, and its text goes in as typed keys since it takes no
  composition. A field that says what it holds (`El::input_mode`, HTML's `inputmode`: an e-mail or web address, a
  number, a decimal, a phone number, a search) gets the keys for it, a secret number a PIN's,
  and one with `InputMode::None` no keyboard at all.
- **Haptics**: what handlers ask for with `Tx::haptic` is played after the frame with
  `View.performHapticFeedback` (the table in `crates/rune/src/haptics.rs`), which needs no
  permission and follows the system's touch feedback setting. The view adds what Android's own
  text gives: a long press that brings the text's menu is felt, and a dragged selection handle
  ticks as its end moves.
- **Fonts**: a character the app's fonts lack draws with the system's font for its script, from
  `/system/etc/fonts.xml` and `/system/fonts` (CJK, Arabic, Indic scripts, colour emoji, symbols),
  loaded the first time a text needs it.
- **Clipboard**: text and one image through `ClipboardManager`; a copied image is served by
  `RuneClipboardProvider`, which the manifest declares with the authority
  `${applicationId}.rune.clipboard`, `exported="false"` and `grantUriPermissions="true"`: the
  clipboard grants the app that pastes a read of the image, and no other app can open it. A secret
  copy is marked sensitive. `clipboard::revision` counts the system's clipboard changes.
- **Links**: `Tx::open_url` starts `Intent.ACTION_VIEW`; `Tx::open_url_then` reports a failure when no
  app takes the link.
- **Files**: `Tx::pick_file`, `Tx::pick_files` and `Tx::pick_save_file` show the system's
  document picker (`Intent.ACTION_OPEN_DOCUMENT` with the filters' extensions as MIME types,
  `ACTION_CREATE_DOCUMENT` to save). It answers with `content://` URIs, which have no path: each
  is a `FileHandle` with the document's name and `path()` `None`, from a `FileHandleProvider`
  (`picker.rs`) that opens it through the `ContentResolver` (`RuneServices.openDocument`) for
  each read or write, with nothing copied.
  `RuneActivity` starts the picker; a host with its own activity sets `RuneView.filePicker` and
  calls `filesPicked`. The dialogs that answer with a path (`Tx::open_file`, `Tx::open_files`,
  `Tx::save_file`) and `Tx::open_folder` are refused with an error.
- **Accessibility**: TalkBack reads Rune's AccessKit tree through an `AccessibilityNodeProvider` on
  `RuneView` (`accesskit_android`, with the `accessibility` feature): roles, names, states, focus,
  explore by touch and actions. The view is a passive root and each text a stop of its own
  (Android 9+). Nothing is built until a service asks, and while a viewport scrolls the tree's
  changes go at most every 100 ms.
- **Back**: when something has focus, back sends Escape (closing what's open or leaving a
  field); otherwise the activity goes back.

Not yet: file pickers.
