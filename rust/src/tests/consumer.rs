//! The surfaces the BetterBeltBalancer pilot was blocked on: prototype names a
//! migrating mod keeps, the prototype fields it needs, and a recipe whose
//! result it did not declare.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::locale::{locale_entries, CfgEntry};
use crate::plan::{Ingredient, ItemRef, ItemSpec, Lib, RecipeRef, RecipeSpec, TechSpec};
use crate::tests::data::{LOGISTICS_2_UNIT, LOGISTICS_3_UNIT};
use crate::tests::{assert_lines, base_world, transcript};
use crate::value::{kv, Value};
use crate::world::Named;

// ---------------------------------------------------------------------------
// Legacy prototypes.
// ---------------------------------------------------------------------------

/// A NAME A SAVE ALREADY REFERENCES IS EMITTED VERBATIM. The pilot's own item
/// is the case: a hand-rolled entity names it, and the engine's answer to a
/// renamed one is an assignID abort rather than a warning.
#[test]
fn legacy_prototypes_keep_their_names() {
    let mut lib = Lib::new();
    let part = lib.legacy_item(
        "bbb-balancer-part",
        ItemSpec {
            stack_size: 50,
            ..Default::default()
        },
    );
    lib.legacy_recipe(
        part,
        "bbb-balancer-part",
        RecipeSpec {
            ingredients: vec![Ingredient::named(1, "steel-plate", &[])],
            ..Default::default()
        },
    );
    lib.legacy_technology(
        "bbb-balancer",
        TechSpec {
            cost_of: String::from("logistics-2"),
            ..Default::default()
        },
    );
    // A generated declaration beside them still takes the prefix.
    let gen = lib.item("hardened-steel-plate", ItemSpec::default());
    lib.recipe(
        gen,
        RecipeSpec {
            ingredients: vec![Ingredient::of(part, 2)],
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="bbb-balancer-part", stack_size=50}"#,
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="bbb-balancer-part", enabled=true, ingredients=[{type="item", name="steel-plate", amount=1}], results=[{type="item", name="bbb-balancer-part", amount=1}]}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="bbb-balancer-part", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
            &alloc::format!(
                r#"extend {{type="technology", name="bbb-balancer", unit={}}}"#,
                LOGISTICS_2_UNIT
            ),
        ],
    );
}

/// A LEGACY HANDLE IS AN ORDINARY HANDLE, which is what lets a mod migrate one
/// prototype at a time: the generated half references the legacy half and the
/// splices reach both.
#[test]
fn legacy_and_generated_prototypes_interlock() {
    let mut lib = Lib::new();
    let part = lib.legacy_item("bbb-balancer-part", ItemSpec::default());
    let rec = lib.legacy_recipe(
        part,
        "bbb-balancer-part",
        RecipeSpec {
            ingredients: vec![Ingredient::of(part, 1)],
            ..Default::default()
        },
    );
    let first = lib.legacy_technology(
        "bbb-balancer",
        TechSpec {
            cost_of: String::from("logistics-2"),
            after: String::from("steel-processing"),
            unlocks: vec![rec],
            ..Default::default()
        },
    );
    // A GENERATED technology anchored on a LEGACY one by handle.
    lib.technology(
        "balancer-2",
        TechSpec {
            cost_of: String::from("logistics-3"),
            after_tech: first,
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="bbb-balancer-part", stack_size=50}"#,
            r#"extend {type="recipe", name="bbb-balancer-part", enabled=false, ingredients=[{type="item", name="bbb-balancer-part", amount=1}], results=[{type="item", name="bbb-balancer-part", amount=1}]}"#,
            &alloc::format!(
                r#"extend {{type="technology", name="bbb-balancer", prerequisites=["steel-processing"], unit={}, effects=[{{type="unlock-recipe", recipe="bbb-balancer-part"}}]}}"#,
                LOGISTICS_2_UNIT
            ),
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-balancer-2", prerequisites=["bbb-balancer"], unit={}}}"#,
                LOGISTICS_3_UNIT
            ),
        ],
    );
}

#[test]
fn legacy_prototype_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
    }

    let cases = [
        Case {
            // The names differ as declared and collide as EMITTED, which is
            // the namespace the engine keeps.
            name: "a legacy item name colliding with a generated one",
            build: |l| {
                l.item("hardened-steel-plate", ItemSpec::default());
                l.legacy_item("steelworks-hardened-steel-plate", ItemSpec::default());
            },
            want: "fkrecipes: two items share the name steelworks-hardened-steel-plate; the second would overwrite the first",
        },
        Case {
            name: "a legacy technology name colliding with a generated one",
            build: |l| {
                l.technology(
                    "balancer",
                    TechSpec {
                        cost_of: String::from("logistics-2"),
                        ..Default::default()
                    },
                );
                l.legacy_technology(
                    "steelworks-balancer",
                    TechSpec {
                        cost_of: String::from("logistics-2"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: two technologies share the name steelworks-balancer; the second would overwrite the first",
        },
        Case {
            // A legacy name is checked against data.raw under the name it
            // really carries, not under a prefix it never gets.
            name: "a legacy item that already exists in data.raw",
            build: |l| {
                l.legacy_item("steel-plate", ItemSpec::default());
            },
            want: "fkrecipes: the item steel-plate already exists in data.raw; this plan would overwrite it",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        (c.build)(&mut lib);
        match lib.plan_data(&base_world()) {
            Ok(ops) => panic!(
                "{}: the plan was accepted with {} ops, want {}",
                c.name,
                ops.len(),
                c.want
            ),
            Err(got) => assert_eq!(got, c.want, "{}", c.name),
        }
    }
}

// ---------------------------------------------------------------------------
// Order, place_result and extra.
// ---------------------------------------------------------------------------

/// All four fields the pilot measured DROPPED from the dump, emitted.
#[test]
fn field_slots_reach_the_prototypes() {
    let mut lib = Lib::new();
    let part = lib.item(
        "balancer-part",
        ItemSpec {
            order: String::from("z[balancer]"),
            place_result: String::from("steel-chest"),
            extra: vec![kv("weight", Value::Num(100.0))],
            ..Default::default()
        },
    );
    lib.recipe(
        part,
        RecipeSpec {
            order: String::from("z[balancer]-a"),
            ingredients: vec![Ingredient::named(1, "steel-plate", &[])],
            extra: vec![kv("allow_productivity", Value::Bool(true))],
            ..Default::default()
        },
    );
    lib.technology(
        "balancer",
        TechSpec {
            cost_of: String::from("logistics-2"),
            order: String::from("c-b-z"),
            extra: vec![kv("upgrade", Value::Bool(false))],
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-balancer-part", stack_size=50, order="z[balancer]", place_result="steel-chest", weight=100}"#,
            r#"extend {type="recipe", name="steelworks-balancer-part", enabled=true, ingredients=[{type="item", name="steel-plate", amount=1}], results=[{type="item", name="steelworks-balancer-part", amount=1}], order="z[balancer]-a", allow_productivity=true}"#,
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-balancer", unit={}, order="c-b-z", upgrade=false}}"#,
                LOGISTICS_2_UNIT
            ),
        ],
    );
}

/// A place_result naming an entity the WORLD gained at test time, so the probe
/// is a real lookup rather than anything that could be special-cased on the
/// base fixture's one row. This is the pilot's own shape: the entity is the
/// mod's own hand-rolled neighbour, present because the mod declares it beside
/// the emit call.
#[test]
fn place_result_resolves_against_the_world() {
    let mut lib = Lib::new();
    lib.item(
        "balancer-part",
        ItemSpec {
            place_result: String::from("bbb-balancer-1-to-2"),
            ..Default::default()
        },
    );

    let w = base_world().with_entity("bbb-balancer-1-to-2");
    let ops = lib.plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-balancer-part", stack_size=50, place_result="bbb-balancer-1-to-2"}"#,
        ],
    );
}

// ---------------------------------------------------------------------------
// A recipe whose result this plan does not declare.
// ---------------------------------------------------------------------------

/// The migration case: a recipe moves onto the library before its item does,
/// or the recipe produces somebody else's item outright.
#[test]
fn recipe_producing_an_existing_item() {
    let mut lib = Lib::new();
    lib.recipe(
        ItemRef::default(),
        RecipeSpec {
            name: String::from("steel-plate-recycling"),
            result_named: String::from("steel-plate"),
            result_count: 2,
            ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="recipe", name="steelworks-steel-plate-recycling", enabled=true, ingredients=[{type="item", name="iron-plate", amount=1}], results=[{type="item", name="steel-plate", amount=2}]}"#,
        ],
    );
}

/// `ENABLED` IN EXTRA IS THE MIGRATING MOD'S OWN while its technology is still
/// hand-rolled. Nothing in the plan unlocks the recipe, so the library has no
/// claim on the field, and the consumer's value rides in THE SLOT THE
/// LIBRARY'S OWN WOULD HAVE TAKEN: between `energy_required` and
/// `ingredients`, not at the end with the rest of Extra, so the field order a
/// migrating mod's golden already saw does not move.
///
/// The technology beside it unlocks a DIFFERENT recipe, which is what makes
/// this a fact about the recipe rather than about the plan.
#[test]
fn extra_carries_enabled_when_nothing_unlocks_the_recipe() {
    let mut lib = Lib::new();
    let part = lib.item("balancer-part", ItemSpec::default());
    lib.recipe(
        part,
        RecipeSpec {
            craft_time: 2.0,
            ingredients: vec![Ingredient::named(1, "steel-plate", &[])],
            order: String::from("z"),
            // A second Extra key AFTER enabled: the passthrough takes the
            // library's slot and this one still rides at the end, so the test
            // reads the position and not just the value.
            extra: vec![
                kv("enabled", Value::Bool(false)),
                kv("hidden", Value::Bool(true)),
            ],
            ..Default::default()
        },
    );
    let frame = lib.item("balancer-frame", ItemSpec::default());
    let framing = lib.recipe(
        frame,
        RecipeSpec {
            ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
            ..Default::default()
        },
    );
    lib.technology(
        "balancer",
        TechSpec {
            cost_of: String::from("logistics-2"),
            unlocks: vec![framing],
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-balancer-part", stack_size=50}"#,
            r#"extend {type="item", name="steelworks-balancer-frame", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-balancer-part", energy_required=2, enabled=false, ingredients=[{type="item", name="steel-plate", amount=1}], results=[{type="item", name="steelworks-balancer-part", amount=1}], order="z", hidden=true}"#,
            r#"extend {type="recipe", name="steelworks-balancer-frame", enabled=false, ingredients=[{type="item", name="iron-plate", amount=1}], results=[{type="item", name="steelworks-balancer-frame", amount=1}]}"#,
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-balancer", unit={}, effects=[{{type="unlock-recipe", recipe="steelworks-balancer-frame"}}]}}"#,
                LOGISTICS_2_UNIT
            ),
        ],
    );
}

#[test]
fn consumer_surface_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
    }

    let cases = [
        Case {
            // The engine's own answer to this is an assignID ABORT naming the
            // item, which is what the pilot's scratch clone hit.
            name: "a place_result that does not exist",
            build: |l| {
                l.item(
                    "balancer-part",
                    ItemSpec {
                        place_result: String::from("bbb-balancer-1-to-2"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the item balancer-part names a place_result bbb-balancer-1-to-2 that does not exist",
        },
        Case {
            name: "an Extra key this library emits itself",
            build: |l| {
                let it = l.item("balancer-part", ItemSpec::default());
                l.recipe(
                    it,
                    RecipeSpec {
                        extra: vec![kv("ingredients", Value::Arr(Vec::new()))],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe balancer-part sets ingredients through Extra, which this library emits",
        },
        Case {
            // THE OTHER DIRECTION of the migration exemption: the technology
            // has landed, so the research is what turns the recipe on and the
            // hand-written value would be the loser of a silent race.
            name: "an Extra enabled on a recipe a plan technology unlocks",
            build: |l| {
                let it = l.item("balancer-part", ItemSpec::default());
                let rec = l.recipe(
                    it,
                    RecipeSpec {
                        extra: vec![kv("enabled", Value::Bool(false))],
                        ..Default::default()
                    },
                );
                l.technology(
                    "balancer",
                    TechSpec {
                        cost_of: String::from("logistics-2"),
                        unlocks: vec![rec],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe balancer-part puts enabled in Extra, but the technology balancer unlocks it, so the library owns that field",
        },
        Case {
            // Two unlockers name the FIRST IN DECLARATION ORDER, so the
            // sentence does not depend on a scan direction nobody promised.
            name: "an Extra enabled on a recipe two technologies unlock",
            build: |l| {
                let it = l.item("balancer-part", ItemSpec::default());
                let rec = l.recipe(
                    it,
                    RecipeSpec {
                        extra: vec![kv("enabled", Value::Bool(false))],
                        ..Default::default()
                    },
                );
                l.technology(
                    "balancer",
                    TechSpec {
                        cost_of: String::from("logistics-2"),
                        unlocks: vec![rec],
                        ..Default::default()
                    },
                );
                l.technology(
                    "balancer-again",
                    TechSpec {
                        cost_of: String::from("logistics-2"),
                        unlocks: vec![rec],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe balancer-part puts enabled in Extra, but the technology balancer unlocks it, so the library owns that field",
        },
        Case {
            // A HANDLE FROM ANOTHER PLAN is not an unlock in this one. The
            // technology loop is what refuses it, by name; the check behind
            // the enabled rule must not follow it and read past this plan's
            // recipes on the way.
            name: "an Extra enabled beside an unlock handle from another plan",
            build: |l| {
                let mut other = Lib::new();
                let elsewhere = other.item("balancer-frame", ItemSpec::default());
                let framing = other.recipe(elsewhere, RecipeSpec::default());
                let it = l.item("balancer-part", ItemSpec::default());
                l.recipe(
                    it,
                    RecipeSpec {
                        extra: vec![kv("enabled", Value::Bool(false))],
                        ..Default::default()
                    },
                );
                l.technology(
                    "balancer",
                    TechSpec {
                        cost_of: String::from("logistics-2"),
                        unlocks: vec![framing],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology balancer unlocks a recipe that this plan never declared",
        },
        Case {
            // The exemption is for ONE key. Another field the library emits is
            // refused beside an accepted enabled, and the sentence is the
            // ordinary one.
            name: "a library field beside an accepted Extra enabled",
            build: |l| {
                let it = l.item("balancer-part", ItemSpec::default());
                l.recipe(
                    it,
                    RecipeSpec {
                        extra: vec![
                            kv("enabled", Value::Bool(false)),
                            kv("results", Value::Arr(Vec::new())),
                        ],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe balancer-part sets results through Extra, which this library emits",
        },
        Case {
            // An accepted key is still refused TWICE: the exemption hands the
            // field to the consumer, not the last-writer race with it.
            name: "an accepted Extra enabled written twice",
            build: |l| {
                let it = l.item("balancer-part", ItemSpec::default());
                l.recipe(
                    it,
                    RecipeSpec {
                        extra: vec![
                            kv("enabled", Value::Bool(false)),
                            kv("enabled", Value::Bool(true)),
                        ],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe balancer-part sets enabled through Extra twice",
        },
        Case {
            name: "an Extra key set twice",
            build: |l| {
                l.item(
                    "balancer-part",
                    ItemSpec {
                        extra: vec![
                            kv("weight", Value::Num(1.0)),
                            kv("weight", Value::Num(2.0)),
                        ],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the item balancer-part sets weight through Extra twice",
        },
        Case {
            name: "an Extra key with no name",
            build: |l| {
                l.technology(
                    "balancer",
                    TechSpec {
                        cost_of: String::from("logistics-2"),
                        extra: vec![kv("", Value::Num(1.0))],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology balancer sets a field through Extra with an empty name",
        },
        Case {
            // THE PROBE ASKS THE WORLD, and the World is data.raw before this
            // plan runs, so the plan's own item is not there yet. Refusing is
            // right; refusing with "does not exist" would send the consumer
            // looking for a missing prototype they can see two lines up.
            name: "a ResultNamed item this plan declares",
            build: |l| {
                l.item("balancer-part", ItemSpec::default());
                l.recipe(
                    ItemRef::default(),
                    RecipeSpec {
                        name: String::from("assembly"),
                        result_named: String::from("steelworks-balancer-part"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe assembly produces steelworks-balancer-part through ResultNamed, which this plan declares; use the item's handle instead",
        },
        Case {
            // The same, under a LEGACY item's verbatim name: the comparison is
            // on emitted names, so both declaration shapes are covered.
            name: "a ResultNamed item this plan declares under a legacy name",
            build: |l| {
                l.legacy_item("bbb-balancer-part", ItemSpec::default());
                l.recipe(
                    ItemRef::default(),
                    RecipeSpec {
                        name: String::from("assembly"),
                        result_named: String::from("bbb-balancer-part"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe assembly produces bbb-balancer-part through ResultNamed, which this plan declares; use the item's handle instead",
        },
        Case {
            name: "a ResultNamed item that does not exist",
            build: |l| {
                l.recipe(
                    ItemRef::default(),
                    RecipeSpec {
                        name: String::from("smelting"),
                        result_named: String::from("tungsten-plate"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe smelting produces tungsten-plate, which does not exist",
        },
        Case {
            name: "a ResultNamed recipe with no name to inherit",
            build: |l| {
                l.recipe(
                    ItemRef::default(),
                    RecipeSpec {
                        result_named: String::from("steel-plate"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: a recipe producing an existing item was declared with no name; there is no declared item to take one from",
        },
        Case {
            name: "both a result handle and ResultNamed",
            build: |l| {
                let it = l.item("balancer-part", ItemSpec::default());
                l.recipe(
                    it,
                    RecipeSpec {
                        result_named: String::from("steel-plate"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe balancer-part names both a result item and ResultNamed; pick one",
        },
        Case {
            // The pre-existing refusal, unchanged: neither a handle nor a name
            // is still a recipe with no result.
            name: "neither a result handle nor a name",
            build: |l| {
                l.recipe(
                    ItemRef::default(),
                    RecipeSpec {
                        name: String::from("smelting"),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        (c.build)(&mut lib);
        match lib.plan_data(&base_world()) {
            Ok(ops) => panic!(
                "{}: the plan was accepted with {} ops, want {}",
                c.name,
                ops.len(),
                c.want
            ),
            Err(got) => assert_eq!(got, c.want, "{}", c.name),
        }
    }
}

// ---------------------------------------------------------------------------
// The narrowed settings seam, and the exported locale reader.
// ---------------------------------------------------------------------------

/// What a consumer's settings test now has to write: ONE method, where the
/// full World is ten. That this type satisfies `plan_settings` is the whole
/// assertion, and it is made at compile time.
struct JustAName;

impl Named for JustAName {
    fn mod_name(&self) -> String {
        String::from("better-belt-balancer")
    }
}

#[test]
fn plan_settings_takes_only_a_name() {
    let mut lib = Lib::new();
    lib.legacy_bool_setting("bbb-multi-edge-parts", false, "a");

    let ops = lib.plan_settings(&JustAName).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="bool-setting", name="bbb-multi-edge-parts", setting_type="startup", default_value=false, order="a"}"#,
        ],
    );
}

/// The parser's output, exported so a consumer can ask questions this library
/// cannot: their own entity's locale entry, a translation file's key set.
#[test]
fn locale_entries_reads_the_file() {
    let cfg = "[mod-setting-name]\n\
               bbb-recipe-cost=Recipe cost\n\
               \n\
               [entity-name]\n\
               bbb-balancer-1-to-2=Balancer\n";
    let got = locale_entries(cfg);
    let want = vec![
        CfgEntry {
            section: String::from("mod-setting-name"),
            key: String::from("bbb-recipe-cost"),
            value: String::from("Recipe cost"),
        },
        CfgEntry {
            section: String::from("entity-name"),
            key: String::from("bbb-balancer-1-to-2"),
            value: String::from("Balancer"),
        },
    ];
    assert_eq!(got, want);
}

// Silences the unused-import warning for a type only named in a signature.
#[allow(dead_code)]
fn _uses_recipe_ref(_: RecipeRef) {}
