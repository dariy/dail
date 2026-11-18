#!/usr/bin/env bash
# The store listing's screenshots (STORE-18) on the booted emulator, into <out>/<lang>/<n>.png.
# Usage: scripts/android-store-shots.sh <out> [<width>x<height> <dpi>]
# store-screenshots.yml runs it once on a phone AVD and once on a tablet AVD. The status bar goes into SystemUI's demo
# mode (a fixed clock, full battery and signal, no notification icons), so the shots do not show the emulator's state.
set -euo pipefail

out=$1
size=${2:-}
dpi=${3:-}

if [ -n "$size" ]; then
  adb shell wm size "$size"
  adb shell wm density "$dpi"
fi

demo() { adb shell am broadcast -a com.android.systemui.demo -e command "$@" > /dev/null; }
adb shell settings put global sysui_demo_allowed 1
# A cold-booted SystemUI can drop the first broadcasts: send them twice, a little apart.
for _ in 1 2; do
  demo enter
  demo clock -e hhmm 0941
  demo battery -e level 100 -e plugged false
  demo network -e wifi show -e level 4 -e fully true
  demo network -e mobile show -e datatype none -e level 4 -e fully true
  demo notifications -e visible false
  sleep 10
done

status=0
./android/gradlew -p android connectedDemoDebugAndroidTest -Pdail.abis=x86_64 \
  -Pandroid.testInstrumentationRunnerArguments.class=net.darii.dail.StoreScreenshotsTest \
  -Pandroid.testInstrumentationRunnerArguments.store=true || status=$?

mkdir -p "$out"
adb pull /data/local/tmp/store/. "$out" || true
demo exit
if [ -n "$size" ]; then
  adb shell wm size reset
  adb shell wm density reset
fi
exit "$status"
