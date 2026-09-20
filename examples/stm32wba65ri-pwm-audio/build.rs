//! Linker arguments for a `cortex-m-rt` firmware image.
//!
//! `memory.x` is provided by `embassy-stm32`'s `memory-x` feature (generated
//! from the `stm32-metapac` chip metadata for `stm32wba65ri`), which also adds
//! its `OUT_DIR` to the linker search path.

fn main() {
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
