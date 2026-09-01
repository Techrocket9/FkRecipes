use alloc::format;
use alloc::vec::Vec;

use crate::op::Op;
use crate::plan::{ItemSpec, Lib, NumericSpec, RecipeSpec, TechSpec};
use crate::tests::data::STEEL_PROCESSING_UNIT;
use crate::tests::*;
use crate::value::Value;

#[test]
fn plan_settings_prototypes() {
    let mut lib = Lib::new();
    lib.bool_setting("hardened-tools", true);
    lib.int_setting("axe-durability", 250, NumericSpec::between(50.0, 1000.0));
    lib.double_setting(
        "axe-craft-time",
        2.5,
        NumericSpec {
            min: Some(0.5),
            max: None,
        },
    );
    lib.dropdown_setting_needing_locale("smelting-style", "furnace", &["furnace", "foundry"]);

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="aa"}"#,
            r#"extend {type="int-setting", name="steelworks-axe-durability", setting_type="startup", default_value=250, order="ab", minimum_value=50, maximum_value=1000}"#,
            r#"extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=2.5000000000000000e0, order="ac", minimum_value=5.0000000000000000e-1}"#,
            r#"extend {type="string-setting", name="steelworks-smelting-style", setting_type="startup", default_value="furnace", order="ad", allowed_values=["furnace", "foundry"]}"#,
        ],
    );
}

/// The order strings are what puts the settings screen in the order the
/// consumer wrote them, so the second letter has to roll over into the first.
#[test]
fn plan_settings_order_strings_roll_over() {
    let mut lib = Lib::new();
    for i in 0..28 {
        lib.bool_setting(&format!("toggle-{}", i), false);
    }
    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    for (at, order) in [(0usize, "aa"), (25, "az"), (26, "ba"), (27, "bb")] {
        let got = match &ops[at] {
            crate::op::Op::Extend(proto) => field(proto, "order"),
            _ => None,
        };
        assert_eq!(
            got,
            Some(crate::value::Value::string(order)),
            "setting {} has the wrong order",
            at
        );
    }
}

#[test]
fn plan_settings_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
    }

    let cases = [
        Case {
            name: "two settings share a name",
            build: |l: &mut Lib| {
                l.bool_setting("hardened-tools", true);
                l.int_setting("hardened-tools", 3, NumericSpec::default());
            },
            want: "fkrecipes: two settings share the name steelworks-hardened-tools; the engine keeps the last one silently",
        },
        Case {
            name: "a setting with an empty name",
            build: |l: &mut Lib| {
                l.bool_setting("", true);
            },
            want: "fkrecipes: a setting was declared with an empty name",
        },
        Case {
            name: "dropdown default is not an allowed value",
            build: |l: &mut Lib| {
                l.dropdown_setting_needing_locale(
                    "smelting-style",
                    "electric-furnace",
                    &["furnace", "foundry"],
                );
            },
            want: "fkrecipes: the dropdown setting smelting-style defaults to electric-furnace, which is not one of its allowed values",
        },
        Case {
            name: "minimum above maximum",
            build: |l: &mut Lib| {
                l.int_setting("axe-durability", 250, NumericSpec::between(1000.0, 50.0));
            },
            want: "fkrecipes: the numeric setting axe-durability declares a minimum above its maximum",
        },
        Case {
            name: "default outside the bounds",
            build: |l: &mut Lib| {
                l.double_setting("axe-craft-time", 12.0, NumericSpec::between(0.5, 8.0));
            },
            want: "fkrecipes: the numeric setting axe-craft-time declares a default outside its own minimum and maximum",
        },
        Case {
            name: "a default that is not a number",
            build: |l: &mut Lib| {
                l.double_setting("axe-craft-time", f64::NAN, NumericSpec::default());
            },
            want: "fkrecipes: the numeric setting axe-craft-time declares a value that is not a finite number",
        },
        Case {
            // The declared i64 is the one number the plan converts to a
            // double on the way in, so it is the one the entry point has to
            // check while it is still an integer.
            name: "an int default past what a double holds",
            build: |l: &mut Lib| {
                l.int_setting("axe-durability", 9007199254740993, NumericSpec::default());
            },
            want: "fkrecipes: the numeric setting axe-durability declares a default a Lua double cannot hold exactly: 9007199254740993",
        },
        Case {
            name: "an int default past what a double holds, negative",
            build: |l: &mut Lib| {
                l.int_setting("axe-durability", -9007199254740993, NumericSpec::default());
            },
            want: "fkrecipes: the numeric setting axe-durability declares a default a Lua double cannot hold exactly: -9007199254740993",
        },
        Case {
            // The auto-minimum is a real bound: a default below it is refused
            // exactly as it would be below a declared one.
            name: "a default below the generated craft-time minimum",
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let from = l.double_setting("axe-craft-time", 0.0001, NumericSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: from,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting axe-craft-time backs a crafting time, so its minimum is 0.002, which is above the declared default",
        },
        Case {
            // The consumer declared only a maximum, so a refusal blaming a
            // declared minimum would send them looking for a line they never
            // wrote.
            name: "a declared maximum below the generated craft-time minimum",
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let from = l.double_setting(
                    "axe-craft-time",
                    0.0015,
                    NumericSpec {
                        min: None,
                        max: Some(0.0015),
                    },
                );
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: from,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting axe-craft-time backs a crafting time, so its minimum is 0.002, which is above the declared maximum",
        },
        Case {
            name: "an explicit minimum at the engine floor on a craft-time setting",
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let from = l.double_setting("axe-craft-time", 2.5, NumericSpec::between(0.001, 60.0));
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: from,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting axe-craft-time backs a crafting time but declares a minimum at or below the engine floor (energy_required can't be <= 0.001)",
        },
        Case {
            name: "a bound that is not a number",
            build: |l: &mut Lib| {
                l.double_setting(
                    "axe-craft-time",
                    2.5,
                    NumericSpec {
                        min: None,
                        max: Some(f64::INFINITY),
                    },
                );
            },
            want: "fkrecipes: the numeric setting axe-craft-time declares a value that is not a finite number",
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

/// The prefix comes from the packaged mod, and there is no prefix parameter
/// to fall back on: with no mod name there is nothing safe to emit.
#[test]
fn plan_settings_refuses_an_empty_mod_name() {
    let mut lib = Lib::new();
    lib.bool_setting("hardened-tools", true);

    match lib.plan_settings(&settings_world().with_mod_name("")) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName"
        ),
    }
}

/// A Lib that never went through new carries no id, and every such plan would
/// carry the SAME one, so its handles cannot be told apart from another
/// plan's. Both entry points refuse rather than validate against an identity
/// nothing owns.
///
/// Only half of the Go mirror's test: dropping the Default derive makes a
/// zero Lib unconstructible from safe code, so the guard is unreachable here
/// and this reaches it the only way the crate can, from the inside.
#[test]
fn planning_refuses_a_lib_built_without_new() {
    let settings_plan = Lib {
        id: 0,
        settings: Vec::new(),
        items: Vec::new(),
        recipes: Vec::new(),
        techs: Vec::new(),
    };

    match settings_plan.plan_settings(&settings_world()) {
        Ok(ops) => panic!("the settings plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: this Lib was built without New, so its handles cannot be validated"
        ),
    }

    let data_plan = Lib {
        id: 0,
        settings: Vec::new(),
        items: Vec::new(),
        recipes: Vec::new(),
        techs: Vec::new(),
    };

    match data_plan.plan_data(&base_world()) {
        Ok(ops) => panic!("the data plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: this Lib was built without New, so its handles cannot be validated"
        ),
    }
}

/// The id is what tells two plans apart, so it is never zero and never
/// repeats. Compared with greater-than rather than plus-one: other tests
/// build plans too, and this harness runs them in parallel threads.
#[test]
fn new_gives_every_plan_its_own_id() {
    let first = Lib::new();
    let second = Lib::new();
    assert!(
        first.id != 0 && second.id != 0,
        "new handed out a zero id: {} then {}",
        first.id,
        second.id
    );
    assert!(
        second.id > first.id,
        "ids are not increasing: {} then {}",
        first.id,
        second.id
    );
}

/// The two stages have to agree on a setting's name to the byte. They derive
/// it from the same World, so this asserts the whole round trip: the name the
/// settings stage creates is the name the data stage finds, and finding it is
/// visible as the technology being hidden with no degradation log.
#[test]
fn settings_and_data_agree_on_the_setting_name() {
    let mut lib = Lib::new();
    let on = lib.bool_setting("hardened-tools", true);
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: "steel-processing".into(),
            enabled_by: on,
            ..Default::default()
        },
    );

    let settings_ops = lib.plan_settings(&settings_world()).expect("plan refused");
    let declared = match &settings_ops[0] {
        Op::Extend(proto) => match field(proto, "name") {
            Some(Value::Str(name)) => name,
            _ => panic!("the setting prototype carries no name"),
        },
        _ => panic!("the first op is not an extend"),
    };

    // The game now holds exactly the setting the settings stage declared,
    // switched off by the player.
    let data_ops = lib
        .plan_data(&base_world().with_setting(&declared, Value::Bool(false)))
        .expect("plan refused");

    assert_lines(
        &transcript(&data_ops),
        &[&alloc::format!(
            r#"extend {{type="technology", name="steelworks-steel-axes", unit={}, enabled=false, hidden=true}}"#,
            STEEL_PROCESSING_UNIT
        )],
    );
}

/// The composition, one refusal per validator family: each case runs a real
/// plan and holds the SENTENCE THAT COMES OUT to the rule, so what is proven
/// here is that a refusal composes into something the host can prefix without
/// saying the stage twice. Emit hands that message to `fkdata::raise`, whose
/// host side prefixes "fklua: at the <stage> stage, " before it.
///
/// THESE SIX ARE A SAMPLE, NOT A SWEEP, and the difference matters: this
/// crate builds some eighty message chunks and six plans cannot reach them
/// all. `tests::source::no_message_carries_its_own_stage` is the sweep, over
/// every string literal in the crate; a stage put back into a template these
/// six never touch is caught there and nowhere else. Neither replaces the
/// other: the property cannot tell whether a message composes correctly, and
/// these cannot tell whether every template obeys.
#[test]
fn refusals_compose_without_their_own_stage() {
    struct Case {
        name: &'static str,
        settings_stage: bool,
        plan: fn(&mut Lib) -> FixtureWorld,
    }

    let cases = [
        Case {
            name: "a settings-stage refusal",
            settings_stage: true,
            plan: |l| {
                l.bool_setting("hardened-tools", true);
                l.bool_setting("hardened-tools", false);
                settings_world()
            },
        },
        Case {
            name: "a declaration refusal",
            settings_stage: false,
            plan: |l| {
                l.item("steel-axe", ItemSpec::default());
                l.item("steel-axe", ItemSpec::default());
                base_world()
            },
        },
        Case {
            name: "a world-probe refusal",
            settings_stage: false,
            plan: |l| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "quarry-drills".into(),
                        ..Default::default()
                    },
                );
                base_world()
            },
        },
        Case {
            name: "a resolved-value refusal",
            settings_stage: false,
            plan: |l| {
                let forging = l.double_setting("forging-time", 3.0, NumericSpec::default());
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: forging,
                        ..Default::default()
                    },
                );
                base_world().with_setting("steelworks-forging-time", Value::Num(0.0005))
            },
        },
        Case {
            name: "a cycle refusal",
            settings_stage: false,
            plan: |l| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        after: "steel-processing".into(),
                        ..Default::default()
                    },
                );
                base_world().with_prereqs("logistics-2", &["logistics", "logistics-3"])
            },
        },
        Case {
            name: "the empty mod name",
            settings_stage: false,
            plan: |_l| base_world().with_mod_name(""),
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        let w = (c.plan)(&mut lib);
        let planned = if c.settings_stage {
            lib.plan_settings(&w)
        } else {
            lib.plan_data(&w)
        };
        match planned {
            Ok(ops) => panic!(
                "{}: the plan was accepted with {} ops, want a refusal",
                c.name,
                ops.len()
            ),
            Err(got) => {
                assert!(
                    got.starts_with("fkrecipes: "),
                    "{}: a refusal does not open with the library attribution: {}",
                    c.name,
                    got
                );
                assert!(
                    !got.contains(" stage,"),
                    "{}: a refusal names a stage the host will name again: {}",
                    c.name,
                    got
                );
            }
        }
    }
}

/// The boundary itself is exact, so it is accepted and comes back unchanged.
#[test]
fn int_setting_accepts_the_exact_boundary() {
    let mut lib = Lib::new();
    lib.int_setting("axe-durability", 9007199254740992, NumericSpec::default());

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="int-setting", name="steelworks-axe-durability", setting_type="startup", default_value=9007199254740992, order="aa"}"#,
        ],
    );
}

/// A double setting nothing binds keeps the bounds the consumer gave it, so
/// the generated minimum is not imposed on every double in the plan.
#[test]
fn an_unbound_double_setting_keeps_its_own_bounds() {
    let mut lib = Lib::new();
    lib.double_setting("axe-craft-time", 2.5, NumericSpec::default());

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=2.5000000000000000e0, order="aa"}"#,
        ],
    );
}

/// The marking scan follows only handles THIS plan issued. A handle from
/// another plan lands on a live index here, and following it would bind a
/// setting the consumer never bound: this plan's setting would pick up a
/// generated minimum above the maximum it declares, and a clean settings stage
/// would start refusing.
#[test]
fn a_foreign_craft_time_handle_marks_nothing() {
    let mut other = Lib::new();
    let stray = other.double_setting("other-craft-time", 2.5, NumericSpec::default());

    let mut lib = Lib::new();
    // Index 1 in this plan too, and a maximum the generated minimum of 0.002
    // would exceed.
    lib.double_setting(
        "axe-craft-time",
        0.001,
        NumericSpec {
            min: None,
            max: Some(0.0015),
        },
    );
    let axe = lib.item("steel-axe", ItemSpec::default());
    lib.recipe(
        axe,
        RecipeSpec {
            craft_time_from: stray,
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=1.0000000000000000e-3, order="aa", maximum_value=1.5000000000000000e-3}"#,
        ],
    );

    // The same handle is refused by name at the data stage, which is where a
    // reference to another plan gets answered.
    match lib.plan_data(&base_world()) {
        Ok(ops) => panic!("the data plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the recipe steel-axe names a crafting-time setting that this plan never declared"
        ),
    }
}
