#!/usr/bin/env bash
# Builds an Android example (examples/android-hello, or android-gallery with --app gallery) into the
# APK of examples/android-host, installs it on the device or emulator adb sees, and starts it.
#
#   scripts/android-run.sh [--app hello|gallery] [--release] [--target aarch64-linux-android|x86_64-linux-android] [--build-only]
#
# Needs ANDROID_NDK_HOME (or ANDROID_NDK_ROOT), ANDROID_HOME with platform 36, JAVA_HOME (17+)
# and Gradle 9 on PATH. The target defaults to the connected device's ABI, else arm64.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
profile=debug
example=hello
target=
build_only=
while [ $# -gt 0 ]; do
  case "$1" in
    --app) example="$2"; shift ;;
    --release) profile=release ;;
    --target) target="$2"; shift ;;
    --build-only) build_only=1 ;;
    *) echo "usage: $0 [--app hello|gallery] [--release] [--target TRIPLE] [--build-only]" >&2; exit 2 ;;
  esac
  shift
done

ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
[ -n "$ndk" ] || { echo "set ANDROID_NDK_HOME to an NDK (r27 or later)" >&2; exit 1; }
adb="${ANDROID_HOME:+$ANDROID_HOME/platform-tools/}adb"

if [ -z "$target" ]; then
  case "$("$adb" shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r')" in
    x86_64) target=x86_64-linux-android ;;
    *) target=aarch64-linux-android ;;
  esac
fi
case "$target" in
  aarch64-linux-android) abi=arm64-v8a ;;
  x86_64-linux-android) abi=x86_64 ;;
  *) echo "unsupported target $target" >&2; exit 2 ;;
esac

case "$(uname -s)" in
  Linux) host=linux-x86_64; ext= ;;
  Darwin) host=darwin-x86_64; ext= ;;
  *) host=windows-x86_64; ext=.cmd ;;
esac
bin="$ndk/toolchains/llvm/prebuilt/$host/bin"
var="${target//-/_}"
export "CC_$var=$bin/${target}24-clang$ext"
export "CXX_$var=$bin/${target}24-clang++$ext"
export "AR_$var=$bin/llvm-ar"
export "CARGO_TARGET_${var^^}_LINKER=$bin/${target}24-clang$ext"

flag=
[ "$profile" = release ] && flag=--release
cargo build -p "android-$example" --target "$target" $flag
out="${CARGO_TARGET_DIR:-$root/target}/$target/$profile/libmain.so"
app="$root/examples/android-host"
mkdir -p "$app/build/jniLibs/$abi"
cp "$out" "$app/build/jniLibs/$abi/libmain.so"
(cd "$app" && gradle --quiet assembleDebug)
apk="$(ls "$app"/build/outputs/apk/debug/*.apk | head -n 1)"
echo "$apk"
[ -n "$build_only" ] && exit 0

"$adb" install -r "$apk"
"$adb" shell am start -n ly.anoma.rune.example/ly.anoma.rune.RuneActivity
