//! A FkLua guest library: a crate a mod's guest imports, compiled into the
//! CONSUMER's wasm by their own `fklua mod`. It declares user-configurable
//! crafting recipes and research, validates them, and emits them at the data
//! stage through fkdata.
//!
//! THE COMPOSITION CONTRACT, in the order it bites:
//!
//! - ROUTE, NEVER OWN. Never export a hook (`fk_settings`, `fk_data`,
//!   `fk_alloc`, ...): a wasm module has ONE export per name, so a
//!   `#[no_mangle]` export here takes it away from the consuming mod. The
//!   consumer owns the stage exports and calls in.
//! - fkdata AND NOTHING ELSE. No `fkapi` under any feature, ever. That is
//!   what makes this library PIN-TRANSPARENT: no API version can break it,
//!   and it sits outside the whole stamp/lock machinery. Never declare the
//!   `fk/fkgc` feature either, and remember a consumer on a vendored checkout
//!   must `[patch]` this crate's git source onto their path: two `fk` crates
//!   is a duplicate `#[global_allocator]` link error.
//! - UNPREFIXED NAMES ARE UNREPRESENTABLE. The prefix derives from the mod
//!   name the `World` reports; there is no prefix parameter, because a
//!   parameter can drift from the packaged mod and the engine keeps the last
//!   of two same-named settings SILENTLY.
//! - DETERMINISM IS CORRECTNESS. Plans are `Vec`s in declaration order and
//!   nothing here touches a `HashMap`. Never iterate a hash map to decide
//!   what to emit: the data stage crosses sorted, and a hash walk is a
//!   per-client order. What comes out of one would be a multiplayer desync
//!   with the CONSUMER's name on it.
//! - DIAGNOSE WITH `raise`. A guest panic surfaces in the player's game as an
//!   opaque trap with the message lost in the log; `fkdata::raise` stops the
//!   load with THIS crate's diagnostic, stage-prefixed like every host
//!   failure. Because the host adds the stage, a refusal built here carries
//!   the crate's attribution and no stage of its own.
//!
//! PLAN, THEN EMIT. Everything in this crate is ordinary values: the plan is
//! built by the declaration methods on [`Lib`], turned into an [`Op`] stream
//! by [`Lib::plan_settings`] and [`Lib::plan_data`], and only the emit module
//! (behind `cfg(target_family = "wasm")`) executes that stream against
//! fkdata. That split is why the validators, the
//! ingredient ladders and the cycle walk run under plain `cargo test` with no
//! wasm target installed.
//!
//! Consumers call Emit. `plan_settings` and `plan_data` are the seam the emit
//! layer stands on, public so a consumer's own tests can assert on a plan.

// no_std in the consumer's wasm, std under `cargo test`: the pure half
// compiles into somebody else's allocator-disciplined module, so it may only
// reach for `alloc`.
#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod cycle;
mod data;
mod locale;
// The emit layer, and the only module that touches fkdata. Gated so the host
// gates compile the pure half with no wasm target and no fkdata at all.
#[cfg(target_family = "wasm")]
mod emit;
mod op;
mod plan;
mod settings;
mod value;
mod world;

#[cfg(test)]
mod tests;

pub use op::{Op, PathEl};
pub use plan::{
    BoolSettingRef, CostChoice, CostChoices, DoubleSettingRef, DropdownSettingRef, Ingredient,
    IngredientChoice, IngredientChoices, IntSettingRef, ItemRef, ItemSpec, Lib, NumericSpec, Pack,
    RecipeRef, RecipeSpec, TechRef, TechSpec, UnitSpec,
};
pub use value::{kv, Value};
pub use world::World;
