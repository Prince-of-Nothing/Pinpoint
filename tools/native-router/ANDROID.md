# Android integration — native-router

The routing crate is pure Rust (`osmpbfreader`, `flate2`/`miniz_oxide`, `serde`)
and makes no platform calls, so it already cross-compiles to Android. This file
records the build path and what is still missing for a real APK.

**This spike does not build or ship an APK, and does not claim to.**

## 1. Cross-compile the crate

Prerequisites:

- Rust target: `rustup target add aarch64-linux-android`
- Android NDK (this machine: `%LOCALAPPDATA%\Android\Sdk\ndk\27.2.12479018`)
- A host linker for build scripts/proc macros (VS Build Tools on Windows)

```powershell
$env:ANDROID_NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\27.2.12479018"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = `
  "$env:ANDROID_NDK_HOME\toolchains\llvm\prebuilt\windows-x86_64\bin\aarch64-linux-android24-clang.cmd"

cd tools/native-router
cargo build --release --target aarch64-linux-android
```

The crate has no C dependencies, so no CMake or NDK build script is involved;
the NDK is used only as the linker/sysroot for the final artifact.

### Recorded result

Run on 2026-11-17 with NDK `27.2.12479018`, rustup 1.100.0, target
`aarch64-linux-android`:

```text
cargo build --release --target aarch64-linux-android --target-dir target-android
...
Finished `release` profile [optimized] target(s) in 1m 00s
```

`target-android/aarch64-linux-android/release/native-router` is 1,093,040 bytes,
`ELF64` / `Machine: AArch64`, `NEEDED: libdl.so, libm.so, libc.so`.
`libnative_router.rlib` is 219,888 bytes. This is **not** an APK and was not run
on a device.

Use a separate `--target-dir` when the host `cargo`/`rustc` and the rustup
toolchain differ (this machine has standalone Rust 1.99.1 first on `PATH` and
rustup 1.100.0 with the Android target); mixing them produces stale artifact
errors. Full command and output: `docs/offline/native-routing.md`.

## 2. JNI/FFI shape (next step, not in this spike)

The app should not call the CLI. Two viable wrappers, both small:

### Option A — direct JNI (recommended for the first integration)

Create a thin wrapper crate (or a `jni` feature in this crate) with:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
jni = "0.21"
native-router = { path = "../native-router" }
```

Expose one long-lived native handle and two calls:

```text
Java_…_NativeRouter_nativeLoadGraph(env, class, pbfPath) -> jlong
Java_…_NativeRouter_nativeRoute(env, class, handle, fromLat, fromLon, toLat, toLon) -> jstring (JSON)
Java_…_NativeRouter_nativeFreeGraph(env, class, handle)
```

- Build the `Graph` once and box it behind the `jlong` handle; do **not** parse
  the PBF per route.
- Copy the returned JSON into a Java `String` and parse it there, or return a
  small object with `distance_m`, `duration_s` and a `double[]` of coordinates.
- Wrap the native call in `catch_unwind` so a Rust panic becomes a Java
  exception instead of aborting the process.
- Put `libnative_router.so` under
  `android/app/src/main/jniLibs/arm64-v8a/` and load it with
  `System.loadLibrary("native_router")`.
- Package only `arm64-v8a` for the first cut; add `armeabi-v7a` and `x86_64`
  (emulator) only when needed.

### Option B — UniFFI

Generate Kotlin bindings for `load_graph(path)` and `route(handle, …)`. More
build tooling (uniffi-bindgen, Gradle task) but less hand-written JNI. Either
option keeps routing local; do not add an HTTP fallback.

## 3. What must change before shipping

1. **Persist the graph.** Parsing the 101 MB PBF at every app start is not
   acceptable on a phone (desktop measurement is in
   `docs/offline/native-routing.md`). Build the graph once when the pack is
   installed/updated and serialize it to app-private storage (`filesDir`), for
   example a versioned binary format (header + node/edge arrays) that can be
   memory-mapped. The download manager already owns pack lifecycle and SHA-256
   verification.
2. **Keep it local.** The Android path must never call an online routing
   service. Outside-coverage checks (`RouteError::OutsideCoverage`) should show
   "outside offline area" rather than silently falling back.
3. **Battery/memory.** The graph is currently `Vec`-based and fully resident.
   Measure on a mid-range arm64 device before committing to a size; a packed
   CSR with `f32` coordinates and memory mapping is the next optimization.
4. **Maneuvers.** This spike returns geometry + ETA only. Turn-by-turn needs
   edge names/refs, turn restrictions and a maneuver generator, or the Valhalla
   path from design §10.

## 4. Native Valhalla alternative

The M1 pack already contains `moldova-valhalla.tar` (510,140 nodes,
1,354,988 directed edges, car/pedestrian/bicycle). If the Rust router is not
good enough, the alternative is the design's primary path: build Valhalla from
source with the NDK/CMake toolchain for `arm64-v8a`, ship `libvalhalla.so` and
load the tar from app storage. That keeps routing local too, but costs a C++
toolchain and a much larger native artifact. The measured Rust numbers in
`docs/offline/native-routing.md` are the input to that decision.
