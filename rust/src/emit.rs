//! The emit layer: the only code in this crate that touches fkdata, gated so
//! the pure half stays host-testable (the host has no wasm imports to bind).
//!
//! IT IS DELIBERATELY THIN. Every decision was made in the pure half and is
//! already in the [`Op`] stream by the time anything here runs; this module
//! gathers what the World asks for, hands the plan back, and executes what
//! comes out. Nothing here may branch on a value it read, because a branch
//! here is a branch no host test can reach.
//!
//! NO HOOKS ARE EXPORTED. A wasm module has one export per name, so a
//! `#[no_mangle] fk_data` here would take that name away from the consuming
//! mod. The consumer owns the four stage exports and calls `emit` from them.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::op::{Op, PathEl};
use crate::plan::Lib;
use crate::value::{kv, Value};
use crate::world::World;

impl Lib {
    /// Plans and then writes. It is the one call a consumer makes:
    ///
    /// ```ignore
    /// #[no_mangle]
    /// pub extern "C" fn fk_data() { lib.emit(); }
    /// ```
    ///
    /// ROUTE `fk_settings` AND EXACTLY ONE DATA-FAMILY HOOK INTO IT. data,
    /// updates and final-fixes share one Lua state and one data.raw, so a
    /// second data-family call would find the first pass's prototypes already
    /// there and refuse as an overwrite. Which one is the consumer's choice:
    /// `fk_data` for content of their own, `fk_data_updates` to sit after
    /// other mods.
    ///
    /// The stage decides which plan runs, and the plan is rebuilt every time:
    /// the module is instantiated fresh per stage, so the consumer's
    /// declarations run again and nothing carries across a stage boundary.
    pub fn emit(&self) {
        let w = DataWorld::new();

        let planned = if fkdata::stage() == fkdata::StageId::Settings {
            self.plan_settings(&w)
        } else {
            self.plan_data(&w)
        };
        let ops = match planned {
            Ok(ops) => ops,
            Err(message) => refuse(&message),
        };

        for op in &ops {
            match op {
                // One prototype per call. extend takes a slice, but a plan's
                // prototypes are independent and a failure names the offending
                // one better when it is the only one in the call.
                Op::Extend(proto) => fkdata::extend(&[to_v(proto)]),
                Op::Set(path, val) => {
                    // Borrowed straight out of the op, which outlives the
                    // call: fkdata::P holds a &str and nothing here needs an
                    // owned copy.
                    let p: Vec<fkdata::P> = path
                        .iter()
                        .map(|el| match el {
                            PathEl::Str(s) => fkdata::P::S(s.as_str()),
                            PathEl::Num(n) => fkdata::P::N(*n),
                        })
                        .collect();
                    // A nil VALUE here would DELETE the key rather than write
                    // one. The planner never puts a Nil in a Set op and a
                    // pure-half test asserts it, which is what keeps this call
                    // a write.
                    fkdata::set(&to_v(val), &p);
                }
                Op::Log(line) => fkdata::log(line),
            }
        }
    }
}

/// Stops the load with the planner's message.
///
/// WHAT THE PLAYER SEES, from reading FkLua's runtime rather than from hope.
/// `fk_data.lua`'s `run()` calls the guest's stage export with NO pcall, so a
/// trap propagates out as a Lua error and Factorio reports a failed data stage
/// naming the consumer's mod. But a trap carries only a code: `fk_rt.lua`
/// builds it as `{fk_trap = "unreachable"}` whose `__tostring` is
/// `"fklua trap: unreachable"`, so the panic's own message reaches NOBODY.
/// That is why the message goes through `log` FIRST: the refusal lands in
/// factorio-current.log in full, and the trap that follows is what actually
/// stops the load. A player gets a load failure naming the mod, and the mod
/// author gets the sentence that says which declaration to fix, one line above
/// it in the log.
fn refuse(message: &str) -> ! {
    fkdata::log(message);
    panic!("{}", message)
}

/// Answers the planner's questions out of data.raw.
///
/// It carries two caches, both scoped to one `emit` and both there because a
/// probe is not free: every fkdata call marshals through the boundary, and the
/// planner asks about the same ingredient names once per recipe that mentions
/// them. The memo is behind a `RefCell` because the World trait answers
/// through `&self`, which is the right shape for a set of questions.
struct DataWorld {
    /// The engine's item types, read ONCE. env(5) cannot change during a
    /// stage, and fkdata rebuilds its answer per call.
    item_types: Vec<String>,

    /// Answers already given, as a vector of pairs scanned linearly. Not a
    /// hash map: this crate does not iterate one, and a plan's ingredient set
    /// is small enough that a scan is cheaper than the probe it saves.
    item_answers: RefCell<Vec<(String, bool)>>,
}

impl DataWorld {
    fn new() -> DataWorld {
        DataWorld {
            item_types: fkdata::derived_types("item"),
            item_answers: RefCell::new(Vec::new()),
        }
    }

    fn probe_item(&self, name: &str) -> bool {
        if name_leaf_exists("item", name) {
            return true;
        }
        for typ in &self.item_types {
            if typ == "item" {
                continue;
            }
            if name_leaf_exists(typ, name) {
                return true;
            }
        }
        false
    }
}

impl World for DataWorld {
    fn mod_name(&self) -> String {
        fkdata::mod_name()
    }

    fn stage_name(&self) -> String {
        String::from(fkdata::stage().name())
    }

    fn startup_setting(&self, name: &str) -> Option<Value> {
        fkdata::startup_setting(name).map(|v| from_v(&v))
    }

    /// `keys` is returned SORTED at every path. That is what the World
    /// contract asks of this method, so the guarantee is the host shim's
    /// rather than something this layer has to arrange.
    fn tech_names(&self) -> Vec<String> {
        fkdata::keys(&[fkdata::P::S("technology")])
    }

    fn tech_prereqs(&self, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(fkdata::V::Arr(items)) = fkdata::get(&[
            fkdata::P::S("technology"),
            fkdata::P::S(name),
            fkdata::P::S("prerequisites"),
        ]) {
            for p in &items {
                if let fkdata::V::Str(s) = p {
                    out.push(s.clone());
                }
            }
        }
        out
    }

    fn tech_unit(&self, name: &str) -> Option<Value> {
        fkdata::get(&[
            fkdata::P::S("technology"),
            fkdata::P::S(name),
            fkdata::P::S("unit"),
        ])
        .map(|v| from_v(&v))
    }

    fn tech_max_level(&self, name: &str) -> Option<Value> {
        fkdata::get(&[
            fkdata::P::S("technology"),
            fkdata::P::S(name),
            fkdata::P::S("max_level"),
        ])
        .map(|v| from_v(&v))
    }

    /// Asks whether the FIELD is there, and reads nothing out of it: a
    /// research_trigger technology is identified by carrying one, and its
    /// shape is the engine's business.
    fn tech_has_research_trigger(&self, name: &str) -> bool {
        fkdata::get(&[
            fkdata::P::S("technology"),
            fkdata::P::S(name),
            fkdata::P::S("research_trigger"),
        ])
        .is_some()
    }

    /// Probes the prototype's NAME LEAF rather than the prototype. "Is this
    /// defined" is a yes or no, and a get of the prototype marshals every
    /// field it has across the boundary to answer it; the leaf is one string.
    /// Measured in BetterBeltBalancer, which is where the shape comes from.
    fn tech_exists(&self, name: &str) -> bool {
        name_leaf_exists("technology", name)
    }

    fn recipe_exists(&self, name: &str) -> bool {
        name_leaf_exists("recipe", name)
    }

    /// Probes the plain "item" type FIRST and only then walks the rest.
    ///
    /// MEASURED (Factorio 2.0.77, build 84539): defines.prototypes.item has 21
    /// keys and INCLUDES "item" itself, alongside ammo, armor, capsule, gun,
    /// module, tool and the rest. data.raw has an "item" table like any other,
    /// and the overwhelming majority of the names a recipe names live in it,
    /// so trying it first answers the common case in one probe. The remaining
    /// types are walked in the sorted order `derived_types` returns, skipping
    /// the one already tried; an item name is unique across those types,
    /// because they share one namespace, so the first hit is the answer.
    ///
    /// The answer is remembered for the rest of this `emit`: the planner asks
    /// about the same names once per recipe that mentions them.
    fn item_exists(&self, name: &str) -> bool {
        for (seen, present) in self.item_answers.borrow().iter() {
            if seen.as_str() == name {
                return *present;
            }
        }
        let present = self.probe_item(name);
        self.item_answers
            .borrow_mut()
            .push((String::from(name), present));
        present
    }
}

fn name_leaf_exists(typ: &str, name: &str) -> bool {
    fkdata::get(&[fkdata::P::S(typ), fkdata::P::S(name), fkdata::P::S("name")]).is_some()
}

/// `to_v` and `from_v` are the whole boundary between the pure value model and
/// fkdata's. They are exact and total in both directions: a map's pairs keep
/// the order the planner built them in (fkdata sorts on the way out), an array
/// stays an array, and a number is an f64 on both sides because Factorio has
/// one number type.
fn to_v(v: &Value) -> fkdata::V {
    match v {
        Value::Nil => fkdata::V::Nil,
        Value::Bool(b) => fkdata::V::Bool(*b),
        Value::Num(n) => fkdata::V::Num(*n),
        Value::Str(s) => fkdata::V::Str(s.clone()),
        Value::Arr(items) => fkdata::V::Arr(items.iter().map(to_v).collect()),
        Value::Map(pairs) => fkdata::V::Map(
            pairs
                .iter()
                .map(|(k, val)| (fkdata::V::Str(k.clone()), to_v(val)))
                .collect(),
        ),
    }
}

/// Carries a read back, TOTALLY: every value either converts or becomes Nil as
/// a whole subtree, and nothing is ever partly kept.
///
/// A NUMBER-KEYED MAP REALLY DOES ARRIVE. `fk_data.lua`'s `key_rank` accepts
/// numbers (rank 1) as well as strings (rank 2) and refuses only the rest, so
/// any holed or mixed Lua table crosses as a map with numeric keys. Keeping the
/// string pairs of such a table and dropping the others would turn a copied
/// unit's ingredient list into an empty one, which is a technology researchable
/// for free. So the whole subtree becomes Nil instead, and because fkdata's own
/// write side skips nils it never delivers one, which makes a Nil here an
/// unambiguous marker: the pure half refuses any copied unit that contains one,
/// by name.
///
/// Every arm is spelled out and there is no catch-all: a variant added to
/// fkdata's V should break this build rather than arrive as a silent nil.
fn from_v(v: &fkdata::V) -> Value {
    match v {
        fkdata::V::Nil => Value::Nil,
        fkdata::V::Bool(b) => Value::Bool(*b),
        fkdata::V::Num(n) => Value::Num(*n),
        fkdata::V::Str(s) => Value::Str(s.clone()),
        // A LuaObject: the handle table is the control stage's and
        // `fk_data.lua` does not bind it, so one is not expected here. It
        // lands on nil rather than being called impossible.
        fkdata::V::Obj(_) => Value::Nil,
        fkdata::V::Arr(items) => Value::Arr(items.iter().map(from_v).collect()),
        fkdata::V::Map(pairs) => {
            let mut out = Vec::with_capacity(pairs.len());
            for (k, val) in pairs {
                match k {
                    fkdata::V::Str(key) => out.push(kv(key.as_str(), from_v(val))),
                    _ => return Value::Nil,
                }
            }
            Value::Map(out)
        }
    }
}
