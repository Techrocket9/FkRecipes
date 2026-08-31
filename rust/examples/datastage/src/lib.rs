//! The Rust example guest. The real body lands with the mirror harness; this
//! placeholder keeps the workspace member compiling for BOTH its targets: the
//! host (empty, std) and wasm32-unknown-unknown, where linking fkdata brings
//! in fk's allocator and panic handler, without which even an empty no_std
//! cdylib refuses to build.
#![cfg_attr(target_family = "wasm", no_std)]

#[cfg(target_family = "wasm")]
use fkdata as _;
