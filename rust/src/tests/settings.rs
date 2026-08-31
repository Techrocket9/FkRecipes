use alloc::format;
use alloc::vec::Vec;

use crate::op::Op;
use crate::plan::{ItemSpec, Lib, NumericSpec, TechSpec};
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
            want: "fkrecipes: at the settings stage, two settings share the name hardened-tools; the engine keeps the last one silently",
        },
        Case {
            name: "a setting with an empty name",
            build: |l: &mut Lib| {
                l.bool_setting("", true);
            },
            want: "fkrecipes: at the settings stage, a setting was declared with an empty name",
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
            want: "fkrecipes: at the settings stage, the dropdown setting smelting-style defaults to electric-furnace, which is not one of its allowed values",
        },
        Case {
            name: "minimum above maximum",
            build: |l: &mut Lib| {
                l.int_setting("axe-durability", 250, NumericSpec::between(1000.0, 50.0));
            },
            want: "fkrecipes: at the settings stage, the numeric setting axe-durability declares a minimum above its maximum",
        },
        Case {
            name: "default outside the bounds",
            build: |l: &mut Lib| {
                l.double_setting("axe-craft-time", 12.0, NumericSpec::between(0.5, 8.0));
            },
            want: "fkrecipes: at the settings stage, the numeric setting axe-craft-time declares a default outside its own minimum and maximum",
        },
        Case {
            name: "a default that is not a number",
            build: |l: &mut Lib| {
                l.double_setting("axe-craft-time", f64::NAN, NumericSpec::default());
            },
            want: "fkrecipes: at the settings stage, the numeric setting axe-craft-time declares a value that is not a finite number",
        },
        Case {
            // The declared i64 is the one number the plan converts to a
            // double on the way in, so it is the one the entry point has to
            // check while it is still an integer.
            name: "an int default past what a double holds",
            build: |l: &mut Lib| {
                l.int_setting("axe-durability", 9007199254740993, NumericSpec::default());
            },
            want: "fkrecipes: at the settings stage, the numeric setting axe-durability declares a default a Lua double cannot hold exactly: 9007199254740993",
        },
        Case {
            name: "an int default past what a double holds, negative",
            build: |l: &mut Lib| {
                l.int_setting("axe-durability", -9007199254740993, NumericSpec::default());
            },
            want: "fkrecipes: at the settings stage, the numeric setting axe-durability declares a default a Lua double cannot hold exactly: -9007199254740993",
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
            want: "fkrecipes: at the settings stage, the numeric setting axe-craft-time declares a value that is not a finite number",
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
            "fkrecipes: at the settings stage, the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName"
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
            "fkrecipes: at the settings stage, this Lib was built without New, so its handles cannot be validated"
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
            "fkrecipes: at the data stage, this Lib was built without New, so its handles cannot be validated"
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

/// The stage in a refusal comes from the World, not from a constant. Factorio
/// runs four stages and fkdata reports whichever is live, so a consumer
/// patching from data-updates is told which of their own calls raised.
#[test]
fn refusals_name_the_stage_the_world_reports() {
    let mut lib = Lib::new();
    lib.item("steel-axe", ItemSpec::default());
    lib.item("steel-axe", ItemSpec::default());

    match lib.plan_data(&base_world().with_stage("data-updates")) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: at the data-updates stage, two items share the name steel-axe; the second would overwrite the first"
        ),
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
