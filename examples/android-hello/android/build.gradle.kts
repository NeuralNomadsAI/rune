// The APK around `libmain.so`: Rune's Kotlin host (`crates/rune/android`) and a manifest.
// `scripts/android-run.sh` builds the Rust library into build/jniLibs first.
plugins {
    id("com.android.application") version "9.5.3"
}

android {
    namespace = "ly.anoma.rune.hello"
    compileSdk = 36

    defaultConfig {
        applicationId = "ly.anoma.rune.hello"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.0.1"
    }

    sourceSets.getByName("main") {
        kotlin.directories.add("../../../crates/rune/android/src/main/kotlin")
        jniLibs.directories.add("build/jniLibs")
    }
}
