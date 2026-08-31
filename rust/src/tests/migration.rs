//! The three surfaces a mod with hand-rolled settings needs to migrate onto
//! this library: names it can keep, ingredients a dropdown chooses, and a
//! research cost a dropdown chooses.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::plan::{
    CostChoice, CostChoices, Ingredient, IngredientChoice, IngredientChoices, ItemRef, ItemSpec,
    Lib, NumericSpec, Pack, RecipeSpec, TechSpec, UnitSpec,
};
use crate::tests::data::STEEL_PROCESSING_UNIT;
use crate::tests::{assert_lines, base_world, settings_world, transcript};
use crate::value::Value;

#[test]
fn legacy_settings_keep_their_names_and_orders() {
    let mut lib = Lib::new();
    lib.legacy_dropdown_setting_needing_locale(
        "bbb-recipe-cost",
        "vanilla",
        &["vanilla", "cheap", "belt-fast"],
        "a",
    );
    lib.legacy_bool_setting("bbb-multi-edge-parts", false, "b");
    lib.legacy_int_setting("bbb-batch", 4, NumericSpec::between(1.0, 20.0), "c");
    lib.legacy_double_setting("bbb-speed", 2.5, NumericSpec::default(), "d");
    // A generated setting beside them keeps the prefix and the derived order.
    lib.bool_setting("hardened-tools", true);

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="string-setting", name="bbb-recipe-cost", setting_type="startup", default_value="vanilla", order="a", allowed_values=["vanilla", "cheap", "belt-fast"]}"#,
            r#"extend {type="bool-setting", name="bbb-multi-edge-parts", setting_type="startup", default_value=false, order="b"}"#,
            r#"extend {type="int-setting", name="bbb-batch", setting_type="startup", default_value=4, order="c", minimum_value=1, maximum_value=20}"#,
            r#"extend {type="double-setting", name="bbb-speed", setting_type="startup", default_value=2.5000000000000000e0, order="d"}"#,
            r#"extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="ae"}"#,
        ],
    );
}

/// A legacy handle is an ordinary handle: the bindings take it unchanged.
#[test]
fn legacy_settings_bind_like_generated_ones() {
    let mut lib = Lib::new();
    let enabled = lib.legacy_bool_setting("bbb-enabled", true, "a");
    let forging = lib.legacy_double_setting("bbb-forging-time", 3.0, NumericSpec::default(), "b");
    let axe = lib.item("steel-axe", ItemSpec::default());
    lib.recipe(
        axe,
        RecipeSpec {
            craft_time_from: forging,
            ..Default::default()
        },
    );
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: "steel-processing".into(),
            enabled_by: enabled,
            ..Default::default()
        },
    );

    let w = base_world()
        .with_setting("bbb-enabled", Value::Bool(false))
        .with_setting("bbb-forging-time", Value::Num(9.0));
    let ops = lib.plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-steel-axe", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", energy_required=9, enabled=true, ingredients=[], results=[{type="item", name="steelworks-steel-axe", amount=1}]}"#,
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-steel-axes", unit={}, enabled=false, hidden=true}}"#,
                STEEL_PROCESSING_UNIT
            ),
        ],
    );
}

/// The generated minimum still applies to a legacy double that backs a
/// crafting time, because the engine's floor does not care where the name came
/// from.
#[test]
fn a_legacy_double_bound_as_a_craft_time_gets_the_minimum() {
    let mut lib = Lib::new();
    let forging = lib.legacy_double_setting("bbb-forging-time", 3.0, NumericSpec::default(), "a");
    let axe = lib.item("steel-axe", ItemSpec::default());
    lib.recipe(
        axe,
        RecipeSpec {
            craft_time_from: forging,
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="double-setting", name="bbb-forging-time", setting_type="startup", default_value=3, order="a", minimum_value=2.0000000000000000e-3}"#,
        ],
    );
}

#[test]
fn legacy_setting_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
    }

    let cases = [
        Case {
            name: "an empty legacy name",
            build: |l| {
                l.legacy_bool_setting("", true, "a");
            },
            want: "fkrecipes: at the settings stage, a setting was declared with an empty name",
        },
        Case {
            name: "an empty order",
            build: |l| {
                l.legacy_bool_setting("bbb-enabled", true, "");
            },
            want: "fkrecipes: at the settings stage, the legacy setting bbb-enabled was declared with an empty order",
        },
        Case {
            // The names differ as declared and collide as emitted, which is
            // the namespace the engine keeps.
            name: "a legacy name colliding with a generated one",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.legacy_bool_setting("steelworks-hardened-tools", false, "a");
            },
            want: "fkrecipes: at the settings stage, two settings share the name steelworks-hardened-tools; the engine keeps the last one silently",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        (c.build)(&mut lib);
        match lib.plan_settings(&settings_world()) {
            Ok(ops) => panic!(
                "{}: the plan was accepted with {} ops, want refusal {}",
                c.name,
                ops.len(),
                c.want
            ),
            Err(got) => assert_eq!(got, c.want, "{}", c.name),
        }
    }
}

/// check_locale reads a legacy setting under the name it actually carries, and
/// polices its dropdown values under that name too.
#[test]
fn check_locale_covers_legacy_names() {
    let mut lib = Lib::new();
    lib.legacy_dropdown_setting_needing_locale(
        "bbb-recipe-cost",
        "vanilla",
        &["vanilla", "cheap"],
        "a",
    );

    let cfg = "[mod-setting-name]\n\
               bbb-recipe-cost=Recipe cost\n\
               \n\
               [string-mod-setting]\n\
               bbb-recipe-cost-vanilla=Vanilla\n\
               bbb-recipe-cost-belt-express=Express belts\n";

    assert_lines(
        &lib.check_locale("better-belt-balancer", cfg),
        &[
            "the dropdown setting bbb-recipe-cost has no [string-mod-setting] entry for its value cheap",
            "the [string-mod-setting] entry bbb-recipe-cost-belt-express matches no dropdown value this plan declares",
        ],
    );
}

// ---------------------------------------------------------------------------
// Ingredients a dropdown chooses.
// ---------------------------------------------------------------------------

fn quench_plan(choices: Vec<IngredientChoice>) -> Lib {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());
    lib.recipe(
        plate,
        RecipeSpec {
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices,
            }),
            ..Default::default()
        },
    );
    lib
}

fn water_and_oil() -> Vec<IngredientChoice> {
    vec![
        IngredientChoice {
            value: "water".into(),
            ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
        },
        IngredientChoice {
            value: "oil".into(),
            ingredients: vec![Ingredient::named(3, "iron-plate", &[])],
        },
    ]
}

#[test]
fn ingredients_by_follows_the_setting() {
    let ops = quench_plan(water_and_oil())
        .plan_data(&base_world().with_setting("steelworks-quench-medium", Value::string("oil")))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="iron-plate", amount=3}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
        ],
    );
}

/// An unreadable setting takes the declared default, with the ordinary line.
#[test]
fn ingredients_by_falls_back_to_its_default_value() {
    let ops = quench_plan(water_and_oil())
        .plan_data(&base_world())
        .expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: the setting steelworks-quench-medium was not readable, so its default applies",
        r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
        r#"extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
    ]);
}

/// A chosen plan that names nothing this game has is a recipe made of nothing,
/// so the default option's plan applies instead and says so.
#[test]
fn ingredients_by_falls_back_to_the_default_plan() {
    let choices = vec![
        IngredientChoice {
            value: "water".into(),
            ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
        },
        IngredientChoice {
            value: "oil".into(),
            ingredients: vec![Ingredient::named(3, "tungsten-carbide", &[])],
        },
    ];
    let ops = quench_plan(choices)
        .plan_data(&base_world().with_setting("steelworks-quench-medium", Value::string("oil")))
        .expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: hardened-steel-plate: none of tungsten-carbide is present, so the ingredient is dropped",
        "log fkrecipes: hardened-steel-plate: the oil ingredients name nothing this game has, so the water ingredients apply",
        r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
        r#"extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
    ]);
}

/// When the default resolves to nothing either, the recipe is emitted with no
/// ingredients and every drop is on the record. The load completes and says
/// what happened rather than breaking.
#[test]
fn ingredients_by_emits_nothing_when_no_plan_resolves() {
    let choices = vec![
        IngredientChoice {
            value: "water".into(),
            ingredients: vec![Ingredient::named(2, "titanium-plate", &[])],
        },
        IngredientChoice {
            value: "oil".into(),
            ingredients: vec![Ingredient::named(3, "tungsten-carbide", &[])],
        },
    ];
    let ops = quench_plan(choices)
        .plan_data(&base_world().with_setting("steelworks-quench-medium", Value::string("oil")))
        .expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: hardened-steel-plate: none of tungsten-carbide is present, so the ingredient is dropped",
        "log fkrecipes: hardened-steel-plate: the oil ingredients name nothing this game has, so the water ingredients apply",
        "log fkrecipes: hardened-steel-plate: none of titanium-plate is present, so the ingredient is dropped",
        r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
        r#"extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
    ]);
}

// ---------------------------------------------------------------------------
// A research cost a dropdown chooses.
// ---------------------------------------------------------------------------

fn tier_plan(choices: Vec<CostChoice>) -> Lib {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale(
        "tips-research-tier",
        "logistics",
        &["logistics", "military"],
    );
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices,
                fallback: UnitSpec {
                    count: 60,
                    seconds: 30.0,
                    packs: vec![Pack {
                        name: "automation-science-pack".into(),
                        amount: 1,
                    }],
                },
            }),
            ..Default::default()
        },
    );
    lib
}

fn cost_choice(value: &str, sources: &[&str]) -> CostChoice {
    CostChoice {
        value: value.into(),
        sources: sources.iter().map(|s| String::from(*s)).collect(),
    }
}

/// The ladder takes the first source that exists and carries a unit, copies it
/// with its level cap, and makes that source the sole prerequisite.
#[test]
fn cost_by_copies_the_unit_and_takes_the_prerequisite() {
    let choices = vec![
        cost_choice("logistics", &["logistics-4", "mining-productivity-4"]),
        cost_choice("military", &["steel-processing"]),
    ];
    let ops = tier_plan(choices)
        .plan_data(&base_world())
        .expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
        r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["mining-productivity-4"], unit={count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}, max_level="infinite"}"#,
    ]);
}

/// A research_trigger technology is not a cost source, so the ladder steps
/// past it exactly as it steps past one that is not there.
///
/// The trigger flag is what has to do the work here, so the fixture's
/// steam-power is given a dictionary unit it would not have in the game: with
/// no unit the "carries no unit" arm would step past it anyway and the trigger
/// check would never be the reason.
#[test]
fn cost_by_steps_past_a_research_trigger_source() {
    let choices = vec![
        cost_choice("logistics", &["steam-power", "steel-processing"]),
        cost_choice("military", &["steel-processing"]),
    ];
    let w = base_world().with_unit(
        "steam-power",
        Value::Map(vec![
            crate::value::kv("count", Value::Num(1.0)),
            crate::value::kv("time", Value::Num(1.0)),
            crate::value::kv(
                "ingredients",
                Value::Arr(vec![Value::Arr(vec![
                    Value::string("automation-science-pack"),
                    Value::Num(1.0),
                ])]),
            ),
        ]),
    );
    let ops = tier_plan(choices).plan_data(&w).expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
        &alloc::format!(
            r#"extend {{type="technology", name="steelworks-hardened-tips", prerequisites=["steel-processing"], unit={}}}"#,
            STEEL_PROCESSING_UNIT
        ),
    ]);
}

/// When no source in the chosen ladder carries a unit, the fallback cost
/// applies and the technology hangs off nothing.
#[test]
fn cost_by_falls_back_with_no_prerequisite() {
    let choices = vec![
        cost_choice("logistics", &["logistics-4", "steam-power"]),
        cost_choice("military", &["steel-processing"]),
    ];
    let ops = tier_plan(choices)
        .plan_data(&base_world())
        .expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
        "log fkrecipes: hardened-tips: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
        r#"extend {type="technology", name="steelworks-hardened-tips", unit={count=60, time=30, ingredients=[["automation-science-pack", 1]]}}"#,
    ]);
}

/// The edge the ladder chose joins the cycle overlay like any other.
#[test]
fn cost_by_edge_reaches_the_cycle_walk() {
    let choices = vec![
        cost_choice("logistics", &["logistics-2"]),
        cost_choice("military", &["steel-processing"]),
    ];
    let lib = tier_plan(choices);
    // logistics-2 is made to require the technology the plan is about to add,
    // so the copied edge closes a ring.
    let w = base_world().with_prereqs("logistics-2", &["steelworks-hardened-tips"]);

    match lib.plan_data(&w) {
        Ok(ops) => panic!(
            "the plan was accepted with {} ops, want a cycle refusal",
            ops.len()
        ),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: at the data stage, a prerequisite cycle: logistics-2 -> steelworks-hardened-tips -> logistics-2"
        ),
    }
}

#[test]
fn choice_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
    }

    let cases = [
        Case {
            name: "a recipe naming both ingredient forms",
            build: |l| {
                let medium =
                    l.dropdown_setting_needing_locale("quench-medium", "water", &["water"]);
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients: vec![Ingredient::named(1, "steel-plate", &[])],
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: Vec::new(),
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe hardened-steel-plate names both Ingredients and IngredientsBy; pick one",
        },
        Case {
            // This plan declares a dropdown of its own, so the stray handle
            // is IN RANGE here and only the per-plan tag can tell it apart.
            name: "an ingredients setting from another plan",
            build: |l| {
                let mut other = Lib::new();
                let stray =
                    other.dropdown_setting_needing_locale("quench-medium", "water", &["water"]);
                l.dropdown_setting_needing_locale("quench-medium", "water", &["water"]);
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: stray,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: Vec::new(),
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe hardened-steel-plate names an ingredients setting that this plan never declared",
        },
        Case {
            name: "a choice for a value the setting does not allow",
            build: |l| {
                let medium =
                    l.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![
                                IngredientChoice {
                                    value: "water".into(),
                                    ingredients: Vec::new(),
                                },
                                IngredientChoice {
                                    value: "brine".into(),
                                    ingredients: Vec::new(),
                                },
                            ],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe hardened-steel-plate offers something for brine where the setting steelworks-quench-medium allows oil",
        },
        Case {
            name: "a value with no choice behind it",
            build: |l| {
                let medium =
                    l.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: Vec::new(),
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe hardened-steel-plate offers nothing for the value oil that the setting steelworks-quench-medium allows",
        },
        Case {
            name: "a choice beyond what the setting allows",
            build: |l| {
                let medium =
                    l.dropdown_setting_needing_locale("quench-medium", "water", &["water"]);
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![
                                IngredientChoice {
                                    value: "water".into(),
                                    ingredients: Vec::new(),
                                },
                                IngredientChoice {
                                    value: "oil".into(),
                                    ingredients: Vec::new(),
                                },
                            ],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe hardened-steel-plate offers something for oil, which the setting steelworks-quench-medium does not allow",
        },
        Case {
            name: "an ingredient inside a choice that names nothing this plan declared",
            build: |l| {
                let medium =
                    l.dropdown_setting_needing_locale("quench-medium", "water", &["water"]);
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: vec![Ingredient::of(ItemRef::default(), 1)],
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe hardened-steel-plate names an ingredient item that this plan never declared",
        },
        Case {
            name: "a technology naming CostBy and a placement",
            build: |l| {
                let tier = l.dropdown_setting_needing_locale(
                    "tips-research-tier",
                    "logistics",
                    &["logistics"],
                );
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        after: "steel-processing".into(),
                        cost_by: Some(CostChoices {
                            setting: tier,
                            choices: vec![cost_choice("logistics", &[])],
                            fallback: UnitSpec {
                                count: 1,
                                seconds: 1.0,
                                packs: Vec::new(),
                            },
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology hardened-tips names CostBy with a placement; the prerequisite moves with the unit, so CostBy places the technology itself",
        },
        Case {
            // In range here too, for the same reason.
            name: "a cost setting from another plan",
            build: |l| {
                let mut other = Lib::new();
                let stray = other.dropdown_setting_needing_locale(
                    "tips-research-tier",
                    "logistics",
                    &["logistics"],
                );
                l.dropdown_setting_needing_locale(
                    "tips-research-tier",
                    "logistics",
                    &["logistics"],
                );
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(CostChoices {
                            setting: stray,
                            choices: vec![cost_choice("logistics", &[])],
                            fallback: UnitSpec {
                                count: 1,
                                seconds: 1.0,
                                packs: Vec::new(),
                            },
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology hardened-tips names a cost setting that this plan never declared",
        },
        Case {
            name: "a cost choice for a value the setting does not allow",
            build: |l| {
                let tier = l.dropdown_setting_needing_locale(
                    "tips-research-tier",
                    "logistics",
                    &["logistics"],
                );
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(CostChoices {
                            setting: tier,
                            choices: vec![cost_choice("military", &[])],
                            fallback: UnitSpec {
                                count: 1,
                                seconds: 1.0,
                                packs: Vec::new(),
                            },
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology hardened-tips offers something for military where the setting steelworks-tips-research-tier allows logistics",
        },
        Case {
            // The fallback is the cost that applies when nothing else does, so
            // it is held to the same rules as a hand-rolled one.
            name: "a fallback the engine would refuse",
            build: |l| {
                let tier = l.dropdown_setting_needing_locale(
                    "tips-research-tier",
                    "logistics",
                    &["logistics"],
                );
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(CostChoices {
                            setting: tier,
                            choices: vec![cost_choice("logistics", &[])],
                            fallback: UnitSpec {
                                count: 0,
                                seconds: 30.0,
                                packs: Vec::new(),
                            },
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology hardened-tips has a unit count below 1, which the engine refuses",
        },
        Case {
            name: "a fallback priced in a pack that does not exist",
            build: |l| {
                let tier = l.dropdown_setting_needing_locale(
                    "tips-research-tier",
                    "logistics",
                    &["logistics"],
                );
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(CostChoices {
                            setting: tier,
                            choices: vec![cost_choice("logistics", &[])],
                            fallback: UnitSpec {
                                count: 60,
                                seconds: 30.0,
                                packs: vec![Pack {
                                    name: "military-science-pack".into(),
                                    amount: 1,
                                }],
                            },
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology hardened-tips prices itself in military-science-pack, which does not exist",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        (c.build)(&mut lib);
        match lib.plan_data(&base_world()) {
            Ok(ops) => panic!(
                "{}: the plan was accepted with {} ops, want refusal {}",
                c.name,
                ops.len(),
                c.want
            ),
            Err(got) => assert_eq!(got, c.want, "{}", c.name),
        }
    }
}

/// A choice list is a snapshot like every other spec slice. Rust moves the
/// vector in, so the caller's copy is what is mutated here; the plan must be
/// unmoved by it.
#[test]
fn choices_do_not_alias_the_caller_vectors() {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("quench-medium", "water", &["water"]);
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());

    let mut choices = vec![IngredientChoice {
        value: "water".into(),
        ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
    }];
    lib.recipe(
        plate,
        RecipeSpec {
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: choices.clone(),
            }),
            ..Default::default()
        },
    );
    choices[0] = IngredientChoice {
        value: "oil".into(),
        ingredients: vec![Ingredient::named(99, "iron-plate", &[])],
    };

    let ops = lib.plan_data(&base_world()).expect("plan refused");
    let joined = transcript(&ops).join("\n");
    assert!(
        joined.contains(r#"{type="item", name="steel-plate", amount=2}"#),
        "the plan followed the caller's edits:\n{}",
        joined
    );
}
