//! The HOST side of the emit layer: the same three methods, present so that a
//! consumer's ordinary toolchain sees a complete crate.
//!
//! WHY THESE EXIST, measured by the pilot. The real `emit` is behind
//! `cfg(target_family = "wasm")`, because fkdata's imports do not exist
//! off-target. Without a host counterpart the methods simply do not exist on a
//! host build, so a consumer's `cargo check` fails on their own data guest and
//! they have to name the target on every invocation. The Go half had exactly
//! that problem and BetterBeltBalancer's make check carries the workaround on
//! one line. A guest is written once and checked constantly, so the target
//! belongs on the build that produces the wasm, not on every check.
//!
//! THEY PANIC RATHER THAN RETURN. There is no fkdata on the host: no data.raw
//! to read, no prototype to write, and nothing a caller could do with a silent
//! no-op except ship a mod that emits nothing. A panic naming the cause is the
//! honest answer to a call that cannot be served, and it keeps the host build
//! from ever being mistaken for a working guest.
//!
//! A consumer's own host tests do not come through here.
//! [`Lib::plan_settings`] and [`Lib::plan_data`] are pure and public for
//! exactly that purpose: they take a `World` (or, for settings, a `Named`) the
//! test supplies, and assert on the `Op` stream.

use crate::plan::Lib;

/// The one message, so all three read identically wherever a consumer meets
/// one. Byte-identical to the Go half's.
const HOST_ONLY: &str = "fkrecipes: Emit runs only inside a wasm guest; this build is for the host";

impl Lib {
    /// Panics on the host. See [`Lib::plan_settings`] and [`Lib::plan_data`]
    /// for the pure seams a host test uses instead.
    pub fn emit(&self) -> ! {
        panic!("{}", HOST_ONLY)
    }

    /// Panics on the host, like [`Lib::emit`].
    pub fn emit_settings(&self) -> ! {
        panic!("{}", HOST_ONLY)
    }

    /// Panics on the host, like [`Lib::emit`].
    pub fn emit_data(&self) -> ! {
        panic!("{}", HOST_ONLY)
    }
}
