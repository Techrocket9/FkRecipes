use alloc::string::String;
use alloc::vec::Vec;

use crate::value::Value;

/// The one question a SETTINGS plan asks.
///
/// [`Lib::plan_settings`](crate::Lib::plan_settings) takes this rather than
/// the whole `World`, because it needs the mod name and nothing else: a
/// consumer holding their own settings plan up to the light in a host test
/// implements ONE method instead of ten. `World` requires it, so anything that
/// implements `World` implements this and the emit layer passes the same value
/// to both planners.
pub trait Named {
    /// The packaged mod's name, the sole source of the prefix.
    fn mod_name(&self) -> String;
}

/// Everything the planner is allowed to know about the game outside its own
/// plan. The emit layer implements it over fkdata; host tests implement it
/// over fixtures, which is the whole point of the trait.
///
/// Every method is a QUESTION, never a write: a planner that could mutate
/// could not be replayed, and the plan is what the two languages compare.
///
/// IT GROWS ADDITIVELY, from now on. A new question lands as a DEFAULT method
/// whose body panics naming itself, never as a new required method: the pilot
/// measured what the other shape costs, where adding `entity_exists` broke
/// every consumer's host fixture at once and each of them had to write a stub
/// for a question their tests never ask. A fixture keeps compiling until the
/// declaration it holds up to the light actually reaches the new probe, and
/// the panic then says which method to write, by name. The Go mirror gets the
/// same property from an embeddable `UnimplementedWorld`.
pub trait World: Named {
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

    /// The same question for [`ItemSpec::place_result`](crate::ItemSpec). The
    /// engine's failure for an item naming an entity that is not there is an
    /// assignID abort naming the item, so this probe is what turns that into a
    /// sentence naming the declaration instead.
    fn entity_exists(&self, name: &str) -> bool;

    /// Asked about the plan's OWN recipe names: a plan that would overwrite
    /// an existing prototype is refused, which is also what keeps every
    /// planned name new for the cycle overlay.
    fn recipe_exists(&self, name: &str) -> bool;

    /// The presence question for a FLUID ingredient, asked of
    /// `data.raw.fluid` alone. Items and fluids are separate namespaces and a
    /// name can be in both (base Factorio has none, but an overhaul pack
    /// may), so the two probes stay two.
    ///
    /// A DEFAULT THAT PANICS, see the trait's note: a fixture that never
    /// reaches a fluid never has to answer, and one that does gets told which
    /// method to write.
    fn fluid_exists(&self, _name: &str) -> bool {
        panic!("fkrecipes: World::fluid_exists is not implemented by this fixture")
    }

    /// The presence question for a SCIENCE PACK, and THE NAME IS HISTORICAL.
    /// What it asks is "is this a science pack the running engine's research
    /// units accept", which is not the same probe on the two engines this
    /// library has been measured on; the method kept its name because a
    /// consumer's fixture `World` implements it.
    ///
    /// MEASURED (Factorio 2.0.77 build 84539): a research unit takes tool-type
    /// items and nothing else, and an item ingredient refuses with "Invalid
    /// research unit (iron-plate). Research unit(s) can only be tool type
    /// items at the moment."
    ///
    /// MEASURED (Factorio 2.1.17 build 87315): `data.raw.tool` DOES NOT EXIST,
    /// base's science packs are `data.raw.item` entries with subgroup
    /// "science-pack", and `technology.logistics.unit.ingredients` names one
    /// of those items. So the 2.0 sentence is a 2.0 rule. The emit layer
    /// answers this on both engines; see [`research_unit_takes_items`] for the
    /// key and for what the 2.1 engine really gates on.
    ///
    /// A pack list resolves through this and never through `item_exists`,
    /// either way.
    ///
    /// A DEFAULT THAT PANICS, see [`World::fluid_exists`].
    fn tool_exists(&self, _name: &str) -> bool {
        panic!("fkrecipes: World::tool_exists is not implemented by this fixture")
    }
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

/// The item subgroup a science pack carries on an engine whose research units
/// take items rather than tools.
///
/// MEASURED (Factorio 2.1.17 build 87315, base plus its bundled DLC data,
/// `factorio -c <a private config> --mod-directory <a probe mod> --dump-data`):
/// every one of base's seven packs is a `data.raw.item` entry with subgroup
/// "science-pack". The same walk over `data.raw` at the DATA stage finds two
/// other items in that subgroup, coin and science, which are not science
/// packs, so this probe is a NAMED APPROXIMATION and not the engine's own
/// gate.
///
/// THE ENGINE'S OWN GATE IS LAB COVERAGE, and it is not askable here. Measured
/// on the same engine, a technology whose unit names iron-plate refuses with
///
/// ```text
/// Technology probe-item-unit: there is no lab that will accept all of the science packs this technology requires.
/// Science packs: iron-plate
/// ```
///
/// and the same refusal comes for a freshly declared item whose subgroup IS
/// "science-pack" and which no lab lists, while adding iron-plate to
/// `data.raw.lab.lab.inputs` makes a unit naming iron-plate load with exit 0.
/// So the gate is the lab's inputs and the subgroup has nothing to do with it.
/// A lab-keyed probe is still the wrong probe for this library, because the
/// data stage cannot see the answer: measured in the same run, at the data
/// stage base's `lab` lists seven inputs, space-age's five packs are not items
/// yet and its biolab does not exist, and all of them arrive by
/// data-final-fixes. A lab-keyed probe would therefore drop every pack of any
/// modpack that assembles its labs after the data stage, which is a free
/// research per technology; the subgroup is visible at the data stage and
/// selects exactly base's seven. The 2.0 probe was an approximation of the
/// same kind: it asked whether a name was the right TYPE, not whether a lab
/// would take it.
///
/// COMPILED WHERE IT IS READ AND NOWHERE ELSE, the gate `value::not_text`
/// explains: the only caller is behind `cfg(target_family = "wasm")` and the
/// only witness is behind `cfg(test)`.
#[cfg(any(target_family = "wasm", test))]
pub(crate) const PACK_SUBGROUP: &str = "science-pack";

/// Which of the two MEASURED ENGINES this is, from base's own version. It is a
/// pure function for the reason [`stage_kind`] is: the emit layer sits behind
/// the wasm build gate, so a decision written there is one no host test can
/// reach.
///
/// WHY THE VERSION AND NOT THE PRESENCE OF `data.raw.tool`. "Ask
/// `data.raw.tool` where that table exists and `data.raw.item` where it does
/// not" is the obvious key and it is MEASURABLY WRONG. The tool prototype type
/// still exists on 2.1 (`defines.prototypes.item` still lists "tool" among its
/// 21 keys, measured), and a mod that declares a tool-type prototype loads on
/// 2.1 with exit 0 while a technology beside it prices itself in an ITEM, also
/// measured. One ported mod still shipping a legacy tool-typed pack would put
/// a table there and take every base science pack away from this library,
/// which is the free-research outcome this fix exists to close. base's version
/// is the engine's, is one env read, and says which engine is running whatever
/// the mod set did.
///
/// UNREADABLE MEANS THE CURRENT ENGINE. base is always installed, so this arm
/// is for a host that answered something this parser cannot read.
///
/// WHAT A WRONG ANSWER COSTS, per fact rather than as one claim. On the SCIENCE
/// PACK neither way can stop a load: the tool branch on a 2.1 engine and the
/// item branch on a 2.0 engine both DROP packs, a free research and disclosed,
/// because a 2.0 science pack is not in `data.raw.item` at all. On the RECIPE
/// CATEGORY the wrong answer is worse and is UNMEASURED: emitting `categories`
/// to a 2.0 engine either has it ignore an unknown key, which silently puts the
/// recipe in `crafting` and refuses a fluid ingredient there, or has it refuse
/// the key outright. The 2.0 binary was gone from the machine that found this,
/// so neither outcome was probed. That is an argument for keying on something
/// always readable, which base's version is, and not a claim that the arm is
/// harmless.
#[cfg(any(target_family = "wasm", test))]
pub(crate) fn research_unit_takes_items(base_version: &str) -> bool {
    match major_minor(base_version) {
        None => true,
        Some((major, minor)) => major > 2 || (major == 2 && minor >= 1),
    }
}

/// The leading `<major>.<minor>` of a version string. A version with no minor
/// part reads as minor 0, and anything with no leading digit at all is not a
/// version this library will key on.
///
/// HAND-ROLLED RATHER THAN `str::parse`, because the whole input is two small
/// unsigned numbers and an overflow on a hostile string would be a silent
/// wrong answer rather than an error: a run of digits longer than four is
/// refused here instead.
#[cfg(any(target_family = "wasm", test))]
pub(crate) fn major_minor(v: &str) -> Option<(u32, u32)> {
    let (major, rest) = leading_number(v)?;
    match rest.strip_prefix('.') {
        None => Some((major, 0)),
        Some(after) => match leading_number(after) {
            None => Some((major, 0)),
            Some((minor, _)) => Some((major, minor)),
        },
    }
}

/// The run of digits at the front of `s`, and what follows it.
#[cfg(any(target_family = "wasm", test))]
fn leading_number(s: &str) -> Option<(u32, &str)> {
    let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 || digits > 4 {
        return None;
    }
    let mut value: u32 = 0;
    for b in s.as_bytes()[..digits].iter() {
        value = value * 10 + u32::from(b - b'0');
    }
    Some((value, &s[digits..]))
}

/// The SECOND engine-keyed fact, keyed by the same function for the same
/// reason: the emit layer cannot make this call where a host test could see it.
///
/// MEASURED (Factorio 2.1.17 build 87315): a recipe prototype carrying
/// `category` refuses the load outright, with
///
/// ```text
/// Error while loading recipe prototype "<name>" (recipe): In RecipePrototype, `category` and `additional_categories` got merged into `categories` table. Please use that instead.
/// ```
///
/// while the same recipe carrying `categories = {"crafting-with-fluid"}` loads
/// with exit 0, and base's own sulfuric-acid reads `categories = {"chemistry"}`
/// with no `category` at all. That is not a degradation, it is the WHOLE MOD
/// failing to load, so it is the more severe of this round's two findings and
/// the one that made a 2.1 golden row impossible until it was answered.
///
/// THE SPELLING IS THE EMIT LAYER'S AND NOT THE PLAN'S.
/// [`respell_recipe_category`] rewrites the pair on the way out, so the `Op`
/// stream a consumer's host test asserts on says `category` on every engine
/// and does not move under them. The alternative was a question on [`World`],
/// which is a trait a consumer's own fixture implements: a new method there
/// would have to carry a default, and a default is exactly the silent wrong
/// answer this key exists to avoid.
#[cfg(any(target_family = "wasm", test))]
pub(crate) fn recipe_categories_are_a_list(base_version: &str) -> bool {
    research_unit_takes_items(base_version)
}

/// Rewrites a recipe prototype's `category` pair into the `categories` pair a
/// 2.1 engine wants, holding that one name.
///
/// IT TOUCHES A RECIPE AND NOTHING ELSE. The type is read off the prototype
/// itself rather than assumed from the caller, because the same `Op` stream
/// carries items, technologies and four kinds of setting, and "category" is a
/// field name a future prototype of another kind could carry with a meaning of
/// its own.
///
/// A PROTOTYPE WITH NO CATEGORY COMES BACK UNTOUCHED: the library omits the
/// field entirely for a recipe that declared no category, because an absent
/// field is the engine's own default and "crafting" spelled out would be this
/// library inventing a value the author never wrote.
///
/// `None` IS "UNTOUCHED" AND NOT "NOTHING", which is what keeps the caller
/// from cloning a prototype it is not rewriting: every item, technology and
/// setting in the stream reaches this function on a 2.1 engine, and a `Value`
/// return would deep-copy each of them to change nothing, into an allocator
/// whose free is a no-op. The Go twin returns its input by identity, which is
/// the same property in a language that has it for free.
#[cfg(any(target_family = "wasm", test))]
pub(crate) fn respell_recipe_category(proto: &crate::value::Value) -> Option<crate::value::Value> {
    use crate::value::{kv, Value};
    let pairs = match proto {
        Value::Map(pairs) => pairs,
        _ => return None,
    };
    let is_recipe = pairs
        .iter()
        .find(|(k, _)| k == "type")
        .map(|(_, v)| matches!(v, Value::Str(s) if s == "recipe"))
        .unwrap_or(false);
    if !is_recipe {
        return None;
    }
    let mut out = alloc::vec::Vec::with_capacity(pairs.len());
    let mut changed = false;
    for (key, val) in pairs.iter() {
        match (key.as_str(), val) {
            ("category", Value::Str(name)) => {
                out.push(kv(
                    "categories",
                    Value::arr(alloc::vec![Value::string(name)]),
                ));
                changed = true;
            }
            _ => out.push((key.clone(), val.clone())),
        }
    }
    if !changed {
        return None;
    }
    Some(Value::Map(out))
}
