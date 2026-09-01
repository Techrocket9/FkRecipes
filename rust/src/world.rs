use alloc::string::String;
use alloc::vec::Vec;

use crate::value::Value;

/// Everything the planner is allowed to know about the game outside its own
/// plan. The emit layer implements it over fkdata; host tests implement it
/// over fixtures, which is the whole point of the trait.
///
/// Every method is a QUESTION, never a write: a planner that could mutate
/// could not be replayed, and the plan is what the two languages compare.
pub trait World {
    /// The packaged mod's name, the sole source of the prefix.
    fn mod_name(&self) -> String;

    /// Reads a startup setting by its FULL, prefixed name. `None` is a
    /// setting that is not readable, which the planner degrades to the
    /// declared default plus a log line.
    fn startup_setting(&self, name: &str) -> Option<Value>;

    /// Every technology in the game, SORTED. The caller guarantees the sort;
    /// the cycle walk's determinism rests on it.
    fn tech_names(&self) -> Vec<String>;

    /// One technology's prerequisite list, in its own order.
    ///
    /// STRING ENTRIES ONLY. An entry that is not a string is invisible to this
    /// library, so a splice that rewrites the list drops it. The engine
    /// refuses a non-string prerequisite anyway, so such an entry is somebody
    /// else's load failure already, not one this rewrite introduces.
    fn tech_prereqs(&self, name: &str) -> Vec<String>;

    /// A technology's whole unit, copied verbatim by `cost_of`. Read the
    /// whole map and check the result: 32 of 275 base technologies are
    /// research_trigger technologies with no unit at all.
    fn tech_unit(&self, name: &str) -> Option<Value>;

    /// A technology's max_level, which lives on the TECHNOLOGY and not in its
    /// unit: a verbatim unit copy carries count_formula but cannot carry the
    /// level cap, so `cost_of` reads it from here. A `Value` because the
    /// engine's field is either a number or the string "infinite".
    fn tech_max_level(&self, name: &str) -> Option<Value>;

    /// Reports the research_trigger case directly, so `cost_of` can refuse
    /// with a message instead of copying an absent unit.
    fn tech_has_research_trigger(&self, name: &str) -> bool;

    /// The prerequisite-presence question. Every dangling prerequisite is a
    /// hard load failure naming the CONSUMER's mod, so the planner asks
    /// before it names.
    fn tech_exists(&self, name: &str) -> bool;

    /// The same question for ingredients and science packs.
    fn item_exists(&self, name: &str) -> bool;

    /// Asked about the plan's OWN recipe names: a plan that would overwrite
    /// an existing prototype is refused, which is also what keeps every
    /// planned name new for the cycle overlay.
    fn recipe_exists(&self, name: &str) -> bool;
}

/// Which of this library's two plans a stage calls for.
///
/// There is no "neither" variant: `stage_kind` returns `None` for a stage this
/// library does not plan for, which keeps "not one of ours" out of the type
/// that names the two plans.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StageKind {
    /// The settings stage, which plans setting prototypes.
    Settings,
    /// Any data-family stage, which plans everything else.
    Data,
}

/// Maps a stage NAME to the plan it calls for, `None` for a stage this library
/// does not plan for.
///
/// WHY THIS IS A PURE FUNCTION AND NOT AN `if` IN `emit`. The emit module sits
/// behind `cfg(target_family = "wasm")`, so a dispatch written there is a
/// decision no host test can reach; this one is testable with plain
/// `cargo test`, and the mapping is where the interesting judgement lives.
///
/// THE DEFAULT IS REFUSAL, NOT "PLAN DATA". `emit`'s first shape read
/// "settings plans settings, EVERYTHING ELSE plans data", which is correct for
/// the four stages fkdata names today and quietly wrong for any fifth. FkLua
/// leaves settings-updates and settings-final-fixes unwired on purpose; if
/// they are ever wired, the old shape would run `plan_data` at a settings
/// stage, where data.raw does not exist, and the failure would be a confusing
/// probe error rather than a sentence naming the cause. Mapping by name with
/// an explicit `None` means that day is a one-line change here plus a decision
/// about what the settings family should do, rather than a silent misroute.
///
/// "unknown" is fkdata's own name for a stage id it does not recognise, so it
/// is spelled out here alongside the two unwired ones: all three are the same
/// answer.
pub fn stage_kind(name: &str) -> Option<StageKind> {
    match name {
        "settings" => Some(StageKind::Settings),
        "data" | "data-updates" | "data-final-fixes" => Some(StageKind::Data),
        _ => None,
    }
}
