//! A FkLua guest library: a crate a mod's guest imports, compiled into the
//! CONSUMER's wasm by their own `fklua mod`.
//!
//! THE COMPOSITION CONTRACT, in the order it bites:
//!
//! - ROUTE, NEVER OWN. Never export a hook (`fk_on_tick`, `fk_on_event`,
//!   `fk_alloc`, ...): a wasm module has ONE export per name, so a
//!   `#[no_mangle]` export here takes it away from the consuming mod. The
//!   consumer owns the hooks and routes in; entry points return whether this
//!   library handled the call.
//! - THE CONSUMER'S BINDINGS, NEVER YOUR OWN. Depending on `fk` alone keeps
//!   this library PIN-TRANSPARENT. If it ever needs the generated `fkapi`,
//!   depend on the CONSUMER's copy and never vendor one -- two binding sets
//!   in one module are refused at package time, and ids are pin-relative.
//! - IDS STAY INLINE, AND IN RUST THAT IS AN ATTRIBUTE. Any wrapper that
//!   carries a member/event/define id is `#[inline(always)]` -- not
//!   `#[inline]` -- or whether the id reaches the import as a constant
//!   becomes rustc's cost heuristic's decision, per call site, and the
//!   consumer silently ships the full tables.
//! - JOIN SAFETY PASSES THROUGH. Never store whether an outbound host call
//!   succeeded, never keep anything computed under a load hook, never iterate
//!   a HashMap to decide what to write. The symptom of breaking these is a
//!   multiplayer desync with the CONSUMER's name on it.
//! - ONE `fk` SOURCE. Never declare the `fk/fkgc` feature (the consumer's
//!   graph decides), and document that a consumer on a vendored checkout must
//!   `[patch]` this crate's git source onto their path -- two `fk` crates is
//!   a duplicate `#[global_allocator]` link error.
//!
//! The pure half is below and runs under plain `cargo test`; everything that
//! touches a host import lives behind `cfg(target_family = "wasm")`.

// no_std in the consumer's wasm, std under `cargo test` -- the conditional is
// what lets the ordinary host test harness link while the shipped crate stays
// allocator-disciplined.
#![cfg_attr(not(test), no_std)]

/// The library's state. Everything here lives in the guest heap and therefore
/// in every save: allocate on a schedule you chose, not per tick.
#[derive(Default)]
pub struct Lib {
    ticks: u32,
}

impl Lib {
    /// An example entry point the consumer calls from its own fk_on_tick
    /// export. Pure, so it is host-testable.
    pub fn on_tick(&mut self) -> u32 {
        self.ticks += 1;
        self.ticks
    }

    /// The routing convention: the consumer's fk_on_event calls every
    /// library it imports in turn, and the return says whether this one
    /// handled the event -- so libraries compose without owning the export.
    pub fn on_event(&mut self, _id: u32, _ptr: u32) -> bool {
        false
    }
}

/// Guest-only glue over the pure state; host builds never see it.
#[cfg(target_family = "wasm")]
impl Lib {
    pub fn announce(&self) {
        fkdata::log("fkrecipes: tick counter is running");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pure half runs on the host, which is the property the layout
    /// exists to keep.
    #[test]
    fn on_tick_counts() {
        let mut l = Lib::default();
        assert_eq!(l.on_tick(), 1);
        assert_eq!(l.on_tick(), 2);
    }
}
