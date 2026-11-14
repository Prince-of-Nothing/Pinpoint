//! Android 15+ 16 KB page-size support.
//!
//! On arm64 Android, the native library must have `PT_LOAD` segments aligned
//! to at least the device page size. A 4 KB-aligned library is rejected before
//! any Rust code runs on a 16 KB-page phone:
//!
//!   program alignment (4096) cannot be smaller than system page size (16384)
//!
//! `cargo-apk` sets `CARGO_ENCODED_RUSTFLAGS` for its NDK wrapper, which
//! overrides target rustflags from `.cargo/config.toml`. Emitting the linker
//! argument from the build script keeps `cargo apk build` and plain cargo
//! builds aligned without relying on shell environment variables.
//!
//! 32-bit ARM devices use 4 KB pages, so only arm64 is changed.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_os == "android" && target_arch == "aarch64" {
        println!("cargo:rustc-link-arg=-Wl,-z,max-page-size=16384");
    }
}
