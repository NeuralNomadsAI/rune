// The APK for an Android example (`examples/android-hello` or `examples/android-gallery`): Rune's
// Kotlin host (`crates/rune/android`), a manifest and the example's `libmain.so`, which
// `scripts/android-run.sh` builds into build/jniLibs first.
plugins {
    id("com.android.application") version "9.5.3"
}

android {
    namespace = "ly.anoma.rune.example"
    compileSdk = 36

    defaultConfig {
        applicationId = "ly.anoma.rune.example"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.1"
    }

    sourceSets.getByName("main") {
        kotlin.directories.add("../../crates/rune/android/src/main/kotlin")
        jniLibs.directories.add("build/jniLibs")
    }
}
