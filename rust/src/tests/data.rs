use crate::op::Op;
use crate::plan::{
    Ingredient, ItemRef, ItemSpec, Lib, NumericSpec, Pack, RecipeRef, RecipeSpec, TechSpec,
    UnitSpec,
};
use crate::tests::*;
use crate::value::Value;

// A believable little mod: an axe head, a steel axe made from it, and the
// research that unlocks the recipe.

#[test]
fn plan_data_item_and_recipe_shapes() {
    let mut lib = Lib::new();
    let axe = lib.item(
        "steel-axe",
        ItemSpec {
            icon: "__steelworks__/graphics/icons/steel-axe.png".into(),
            icon_size: 64,
            stack_size: 20,
            subgroup: "tool".into(),
            display_name: "Steel axe".into(),
            description: "Chops trees at twice the speed.".into(),
        },
    );
    let head = lib.item(
        "axe-head",
        ItemSpec {
            icon: "__steelworks__/graphics/icons/axe-head.png".into(),
            ..Default::default()
        },
    );
    lib.recipe(
        head,
        RecipeSpec {
            ingredients: vec![Ingredient::named(2, "steel-plate", &["iron-plate"])],
            ..Default::default()
        },
    );
    lib.recipe(
        axe,
        RecipeSpec {
            craft_time: 2.5,
            category: "crafting".into(),
            result_count: 2,
            ingredients: vec![
                Ingredient::of(head, 1),
                Ingredient::named(4, "steel-plate", &["iron-plate"]),
            ],
            display_name: "Steel axe".into(),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-steel-axe", localised_name=["", "Steel axe"], localised_description=["", "Chops trees at twice the speed."], icon="__steelworks__/graphics/icons/steel-axe.png", icon_size=64, stack_size=20, subgroup="tool"}"#,
            r#"extend {type="item", name="steelworks-axe-head", icon="__steelworks__/graphics/icons/axe-head.png", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-axe-head", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-axe-head", amount=1}]}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", localised_name=["", "Steel axe"], category="crafting", energy_required=2.5000000000000000e0, enabled=true, ingredients=[{type="item", name="steelworks-axe-head", amount=1}, {type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=2}]}"#,
        ],
    );
}

/// The ladder resolves to the first candidate the game actually has, and
/// drops the ingredient when it has none of them. Never a guess: a name the
/// game does not have is a hard load failure naming the consumer's mod.
#[test]
fn ingredient_ladder_falls_back_and_drops() {
    let mut lib = Lib::new();
    let axe = lib.item(
        "steel-axe",
        ItemSpec {
            icon: "__steelworks__/graphics/icons/steel-axe.png".into(),
            ..Default::default()
        },
    );
    lib.recipe(
        axe,
        RecipeSpec {
            ingredients: vec![
                Ingredient::named(4, "steel-plate", &["iron-plate"]),
                Ingredient::named(1, "tungsten-plate", &["titanium-plate"]),
            ],
            ..Default::default()
        },
    );

    // The world has iron but no steel, so the ladder takes its second rung.
    let ops = lib
        .plan_data(&base_world().without_item("steel-plate"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steel-axe: none of tungsten-plate, titanium-plate is present, so the ingredient is dropped",
            r#"extend {type="item", name="steelworks-steel-axe", icon="__steelworks__/graphics/icons/steel-axe.png", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="iron-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}"#,
        ],
    );
}

/// cost_of copies the named technology's whole unit VERBATIM: a count_formula
/// is a string, so an infinite technology's cost comes along with no
/// evaluator and no key of it rewritten or reordered.
#[test]
fn technology_cost_of_copies_unit_verbatim() {
    let mut lib = Lib::new();
    let axe = lib.item(
        "steel-axe",
        ItemSpec {
            icon: "__steelworks__/graphics/icons/steel-axe.png".into(),
            ..Default::default()
        },
    );
    let rec = lib.recipe(
        axe,
        RecipeSpec {
            ingredients: vec![Ingredient::named(4, "steel-plate", &[])],
            ..Default::default()
        },
    );
    lib.technology(
        "steel-axes",
        TechSpec {
            icon: "__steelworks__/graphics/technology/steel-axes.png".into(),
            icon_size: 128,
            cost_of: "mining-productivity-4".into(),
            after: "steel-processing".into(),
            unlocks: vec![rec],
            display_name: "Steel axes".into(),
            description: "Sharper edges, fewer swings.".into(),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-steel-axe", icon="__steelworks__/graphics/icons/steel-axe.png", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", enabled=false, ingredients=[{type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}"#,
            r#"extend {type="technology", name="steelworks-steel-axes", localised_name=["", "Steel axes"], localised_description=["", "Sharper edges, fewer swings."], icon="__steelworks__/graphics/technology/steel-axes.png", icon_size=128, prerequisites=["steel-processing"], unit={count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}, max_level="infinite", effects=[{type="unlock-recipe", recipe="steelworks-steel-axe"}]}"#,
        ],
    );
}

const MINING_UNIT: &str = r#"{count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}"#;

/// The level cap is a TECHNOLOGY field, not a unit field, so the verbatim
/// unit copy cannot carry it: cost_of reads it separately, or an infinite
/// source would produce a one-level copy.
#[test]
fn technology_cost_of_carries_max_level() {
    struct Case {
        name: &'static str,
        cost_of: &'static str,
        world: fn(FixtureWorld) -> FixtureWorld,
        want: &'static str,
    }

    let cases = [
        Case {
            name: "an infinite source keeps its infinity",
            cost_of: "mining-productivity-4",
            world: |w: FixtureWorld| w,
            want: r#"extend {type="technology", name="steelworks-steel-axes", unit=MINING_UNIT, max_level="infinite"}"#,
        },
        Case {
            // An overhaul that caps the endless research at a finite level.
            name: "a numeric cap comes across as a number",
            cost_of: "mining-productivity-4",
            world: |w: FixtureWorld| w.with_max_level("mining-productivity-4", Value::Num(20.0)),
            want: r#"extend {type="technology", name="steelworks-steel-axes", unit=MINING_UNIT, max_level=20}"#,
        },
        Case {
            name: "a single-level source gets no cap at all",
            cost_of: "steel-processing",
            world: |w: FixtureWorld| w,
            want: r#"extend {type="technology", name="steelworks-steel-axes", unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}"#,
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_of: c.cost_of.into(),
                ..Default::default()
            },
        );

        let ops = lib
            .plan_data(&(c.world)(base_world()))
            .expect("plan refused");

        let want = c.want.replace("MINING_UNIT", MINING_UNIT);
        println!("case: {}", c.name);
        assert_lines(&transcript(&ops), &[want.as_str()]);
    }
}

/// A hand-rolled unit has no source technology to read a cap from, so the
/// escape hatch never emits max_level.
#[test]
fn technology_unit_spec_carries_no_max_level() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 75,
                seconds: 30.0,
                packs: vec![Pack {
                    name: "automation-science-pack".into(),
                    amount: 1,
                }],
            }),
            ..Default::default()
        },
    );

    // This world answers a cap for any name at all, so the only thing keeping
    // max_level off the prototype is the planner not asking.
    let ops = lib
        .plan_data(&base_world().answering_max_level_for_any_name(Value::Num(20.0)))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["automation-science-pack", 1]]}}"#,
        ],
    );
}

/// The two ingredient shapes are not interchangeable: a recipe takes the long
/// dict form, a technology unit takes the short tuple form, and the engine
/// refuses each in the other's place.
#[test]
fn technology_unit_spec_uses_short_tuple_form() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 75,
                seconds: 30.0,
                packs: vec![
                    Pack {
                        name: "automation-science-pack".into(),
                        amount: 1,
                    },
                    Pack {
                        name: "logistic-science-pack".into(),
                        amount: 2,
                    },
                ],
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 2]]}}"#,
        ],
    );
}

#[test]
fn technology_enabled_by() {
    struct Case {
        name: &'static str,
        def: bool,
        world: fn(FixtureWorld) -> FixtureWorld,
        tail: &'static str,
        log_line: &'static str,
    }

    let cases = [
        Case {
            name: "the setting is on",
            def: false,
            world: |w: FixtureWorld| w.with_setting("steelworks-hardened-tools", Value::Bool(true)),
            tail: ", enabled=true}",
            log_line: "",
        },
        Case {
            name: "the setting is off",
            def: true,
            world: |w: FixtureWorld| w.with_setting("steelworks-hardened-tools", Value::Bool(false)),
            tail: ", enabled=false, hidden=true}",
            log_line: "",
        },
        Case {
            name: "the setting is unreadable and defaults on",
            def: true,
            world: |w: FixtureWorld| w,
            tail: ", enabled=true}",
            log_line: "log fkrecipes: the setting steelworks-hardened-tools was not readable, so its default applies",
        },
        Case {
            name: "the setting is unreadable and defaults off",
            def: false,
            world: |w: FixtureWorld| w,
            tail: ", enabled=false, hidden=true}",
            log_line: "log fkrecipes: the setting steelworks-hardened-tools was not readable, so its default applies",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        let on = lib.bool_setting("hardened-tools", c.def);
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_of: "steel-processing".into(),
                enabled_by: on,
                ..Default::default()
            },
        );

        let ops = lib
            .plan_data(&(c.world)(base_world()))
            .expect("plan refused");

        let mut want: Vec<String> = Vec::new();
        if !c.log_line.is_empty() {
            want.push(c.log_line.into());
        }
        want.push(format!(
            r#"extend {{type="technology", name="steelworks-steel-axes", unit={}{}"#,
            STEEL_PROCESSING_UNIT, c.tail
        ));
        let want_refs: Vec<&str> = want.iter().map(|s| s.as_str()).collect();
        println!("case: {}", c.name);
        assert_lines(&transcript(&ops), &want_refs);
    }
}

pub(crate) const STEEL_PROCESSING_UNIT: &str =
    r#"{count=50, ingredients=[["automation-science-pack", 1]], time=15}"#;

#[test]
fn tree_placement() {
    struct Case {
        name: &'static str,
        after: &'static str,
        /// Empty for the plain After case.
        before: &'static str,
        world: fn(FixtureWorld) -> FixtureWorld,
        want: &'static [&'static str],
    }

    let cases = [
        Case {
            name: "after alone",
            after: "logistics-2",
            before: "",
            world: |w: FixtureWorld| w,
            want: &[
                r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}"#,
            ],
        },
        Case {
            name: "after alone, absent",
            after: "quarry-drills",
            before: "",
            world: |w: FixtureWorld| w,
            want: &[
                "log fkrecipes: steel-axes: quarry-drills is absent, so the prerequisite is dropped",
                r#"extend {type="technology", name="steelworks-steel-axes", unit=UNIT}"#,
            ],
        },
        Case {
            name: "insert between, the splice replaces the edge",
            after: "logistics-2",
            before: "logistics-3",
            world: |w: FixtureWorld| w,
            want: &[
                r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}"#,
                r#"set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]"#,
            ],
        },
        Case {
            name: "insert between, the far end does not require the near one",
            after: "electronics",
            before: "logistics-3",
            world: |w: FixtureWorld| w,
            want: &[
                "log fkrecipes: steel-axes: logistics-3 does not require electronics, so the new technology is appended to its prerequisites",
                r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["electronics"], unit=UNIT}"#,
                r#"set technology.logistics-3.prerequisites = ["logistics-2", "steelworks-steel-axes"]"#,
            ],
        },
        Case {
            // Another mod removed the technology this plan meant to splice
            // in front of.
            name: "insert between, the far end is absent",
            after: "logistics-2",
            before: "logistics-3",
            world: |w: FixtureWorld| w.without_tech("logistics-3"),
            want: &[
                "log fkrecipes: steel-axes: logistics-3 is absent, so InsertBetween degrades to After(logistics-2)",
                r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}"#,
            ],
        },
        Case {
            name: "insert between, the near end is absent",
            after: "quarry-drills",
            before: "logistics-3",
            world: |w: FixtureWorld| w,
            want: &[
                "log fkrecipes: steel-axes: quarry-drills is absent, so the prerequisite is dropped",
                r#"extend {type="technology", name="steelworks-steel-axes", unit=UNIT}"#,
            ],
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_of: "steel-processing".into(),
                after: c.after.into(),
                before: c.before.into(),
                ..Default::default()
            },
        );

        let ops = lib
            .plan_data(&(c.world)(base_world()))
            .expect("plan refused");

        let want: Vec<String> = c
            .want
            .iter()
            .map(|line| line.replace("UNIT", STEEL_PROCESSING_UNIT))
            .collect();
        let want_refs: Vec<&str> = want.iter().map(|s| s.as_str()).collect();
        println!("case: {}", c.name);
        assert_lines(&transcript(&ops), &want_refs);
    }
}

/// Two splices into the same technology chain: the second reads what the
/// first planned, or the second Set op would silently undo the first.
#[test]
fn two_splices_into_one_technology_chain() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: "steel-processing".into(),
            after: "logistics-2".into(),
            before: "logistics-3".into(),
            ..Default::default()
        },
    );
    lib.technology(
        "bronze-axes",
        TechSpec {
            cost_of: "steel-processing".into(),
            after: "electronics".into(),
            before: "logistics-3".into(),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: bronze-axes: logistics-3 does not require electronics, so the new technology is appended to its prerequisites",
            r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}"#,
            r#"extend {type="technology", name="steelworks-bronze-axes", prerequisites=["electronics"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}"#,
            r#"set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]"#,
            r#"set technology.logistics-3.prerequisites = ["steelworks-steel-axes", "steelworks-bronze-axes"]"#,
        ],
    );
}

/// Nothing this library emits can carry a name it did not prefix. The check
/// is a property over the whole stream, not one assertion per prototype.
#[test]
fn every_emitted_name_is_prefixed() {
    let mut lib = Lib::new();
    let axe = lib.item(
        "steel-axe",
        ItemSpec {
            icon: "__steelworks__/graphics/icons/steel-axe.png".into(),
            ..Default::default()
        },
    );
    let rec = lib.recipe(
        axe,
        RecipeSpec {
            name: "steel-axe-forging".into(),
            ingredients: vec![Ingredient::named(4, "steel-plate", &[])],
            ..Default::default()
        },
    );
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: "steel-processing".into(),
            after: "logistics-2".into(),
            before: "logistics-3".into(),
            unlocks: vec![rec],
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    let mut extends = 0;
    for op in &ops {
        if let Op::Extend(proto) = op {
            extends += 1;
            match field(proto, "name") {
                Some(Value::Str(name)) => assert!(
                    name.starts_with("steelworks-"),
                    "prototype name {} is not prefixed",
                    name
                ),
                _ => panic!("a prototype carries no name: {}", render_value(proto)),
            }
        }
    }
    assert_eq!(extends, 3, "expected three prototypes");

    // The splice writes into somebody else's technology, so the PATH keeps
    // their unprefixed name and only the value carries ours.
    match ops.last() {
        Some(Op::Set(path, val)) => {
            assert_eq!(render_path(path), "technology.logistics-3.prerequisites");
            assert_eq!(render_value(val), r#"["steelworks-steel-axes"]"#);
        }
        _ => panic!("the last op is not the splice"),
    }
}

#[test]
fn plan_data_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        world: fn(FixtureWorld) -> FixtureWorld,
        want: &'static str,
    }

    let cases = [
        Case {
            name: "two items share a name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.item("steel-axe", ItemSpec::default());
                l.item("steel-axe", ItemSpec::default());
            },
            want: "fkrecipes: at the data stage, two items share the name steel-axe; the second would overwrite the first",
        },
        Case {
            name: "two recipes share a name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let head = l.item("axe-head", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        name: "steel-axe-forging".into(),
                        ..Default::default()
                    },
                );
                l.recipe(
                    head,
                    RecipeSpec {
                        name: "steel-axe-forging".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, two recipes share the name steel-axe-forging; the second would overwrite the first",
        },
        Case {
            name: "two technologies share a name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "logistics-2".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, two technologies share the name steel-axes; the second would overwrite the first",
        },
        Case {
            name: "a recipe with no result item",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.recipe(
                    ItemRef::default(),
                    RecipeSpec {
                        name: "steel-axe-forging".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, a recipe was declared with no result item; Recipe needs an item this plan declared",
        },
        Case {
            name: "an ingredient item from no plan",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        ingredients: vec![Ingredient::of(ItemRef::default(), 1)],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steel-axe names an ingredient item that this plan never declared",
        },
        Case {
            name: "neither cost_of nor unit",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        after: "steel-processing".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes must name exactly one of CostOf or Unit",
        },
        Case {
            name: "both cost_of and unit",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 15.0,
                            packs: vec![Pack {
                                name: "automation-science-pack".into(),
                                amount: 1,
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes must name exactly one of CostOf or Unit",
        },
        Case {
            name: "before without after",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        before: "logistics-3".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes names Before without After; InsertBetween needs both ends",
        },
        Case {
            name: "a unit count below one",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 0,
                            seconds: 15.0,
                            packs: vec![Pack {
                                name: "automation-science-pack".into(),
                                amount: 1,
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes has a unit count below 1, which the engine refuses",
        },
        Case {
            name: "a science pack the game does not have",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 15.0,
                            packs: vec![Pack {
                                name: "military-science-pack".into(),
                                amount: 1,
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes prices itself in military-science-pack, which does not exist",
        },
        Case {
            name: "cost_of names a technology that is not there",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "logistics-4".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, CostOf(logistics-4): no technology of that name exists",
        },
        Case {
            name: "cost_of names a research_trigger technology",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steam-power".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, CostOf(steam-power): steam-power is a research_trigger technology with no unit to copy; name a unit-carrying technology instead",
        },
        Case {
            name: "unlocking a recipe from no plan",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        unlocks: vec![RecipeRef::default()],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes unlocks a recipe that this plan never declared",
        },
        Case {
            // Out of range for THIS plan, which is the shape that used to
            // index straight into a shorter vector and panic.
            name: "an EnabledBy setting out of range",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let mut other = Lib::new();
                other.bool_setting("hardened-tools", true);
                other.bool_setting("brittle-heads", false);
                let stray = other.bool_setting("sharpened-edges", true);
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        enabled_by: stray,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes names an EnabledBy setting that this plan never declared",
        },
        Case {
            name: "an item this plan would overwrite",
            world: |w: FixtureWorld| w.with_item("steelworks-steel-axe"),
            build: |l: &mut Lib| {
                l.item("steel-axe", ItemSpec::default());
            },
            want: "fkrecipes: at the data stage, the item steelworks-steel-axe already exists in data.raw; this plan would overwrite it",
        },
        Case {
            name: "a recipe this plan would overwrite",
            world: |w: FixtureWorld| w.with_recipe("steelworks-steel-axe-forging"),
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        name: "steel-axe-forging".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steelworks-steel-axe-forging already exists in data.raw; this plan would overwrite it",
        },
        Case {
            name: "a technology this plan would overwrite",
            world: |w: FixtureWorld| {
                w.with_tech(FixtureTech {
                    name: "steelworks-steel-axes".into(),
                    prereqs: Vec::new(),
                    unit: unit_of(50, 15.0, &["automation-science-pack"]),
                    max_level: Value::Nil,
                    trigger: false,
                })
            },
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steelworks-steel-axes already exists in data.raw; this plan would overwrite it",
        },
        Case {
            name: "both anchors at once",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let first = l.technology(
                    "bronze-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        after: "logistics-2".into(),
                        after_tech: first,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes names both After and AfterTech; pick one anchor",
        },
        Case {
            name: "a splice around a technology this plan declares",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let first = l.technology(
                    "bronze-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        after_tech: first,
                        before: "logistics-3".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes names Before with AfterTech; InsertBetween splices around a technology that already exists",
        },
        Case {
            name: "a crafting time that is not a number",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time: f64::INFINITY,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steel-axe declares a crafting time that is not a finite number",
        },
        Case {
            name: "a research time that is not a number",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: f64::NAN,
                            packs: vec![Pack {
                                name: "automation-science-pack".into(),
                                amount: 1,
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes declares a research time that is not a finite number",
        },
        Case {
            name: "a negative stack size",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.item(
                    "steel-axe",
                    ItemSpec {
                        stack_size: -20,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the item steel-axe has a negative stack size, which the engine refuses",
        },
        Case {
            name: "a negative icon size on an item",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.item(
                    "steel-axe",
                    ItemSpec {
                        icon_size: -64,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the item steel-axe has a negative icon size, which the engine refuses",
        },
        Case {
            name: "a negative icon size on a technology",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        icon_size: -128,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes has a negative icon size, which the engine refuses",
        },
        Case {
            name: "an ingredient amount below one",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        ingredients: vec![Ingredient::named(0, "steel-plate", &[])],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steel-axe has an ingredient amount below 1, which the engine refuses",
        },
        Case {
            name: "a negative result count",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        result_count: -2,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steel-axe has a negative result count, which the engine refuses",
        },
        Case {
            name: "a negative crafting time",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time: -2.5,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steel-axe has a negative crafting time, which the engine refuses",
        },
        Case {
            name: "a science pack amount below one",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 15.0,
                            packs: vec![Pack {
                                name: "automation-science-pack".into(),
                                amount: 0,
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes has a science pack amount below 1, which the engine refuses",
        },
        Case {
            name: "a research time of zero",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 0.0,
                            packs: vec![Pack {
                                name: "automation-science-pack".into(),
                                amount: 1,
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes has a research time at or below zero, which the engine refuses",
        },
        Case {
            name: "a stack size past what a double holds",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.item(
                    "steel-axe",
                    ItemSpec {
                        stack_size: 9007199254740993,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the item steel-axe declares a stack size a Lua double cannot hold exactly: 9007199254740993",
        },
        Case {
            name: "an ingredient amount past what a double holds",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        ingredients: vec![Ingredient::named(9007199254740993, "steel-plate", &[])],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steel-axe declares an ingredient amount a Lua double cannot hold exactly: 9007199254740993",
        },
        Case {
            name: "a unit count past what a double holds",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 9007199254740993,
                            seconds: 15.0,
                            packs: vec![Pack {
                                name: "automation-science-pack".into(),
                                amount: 1,
                            }],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes declares a unit count a Lua double cannot hold exactly: 9007199254740993",
        },
        Case {
            name: "a CostOf source whose unit is not a dictionary",
            // A Lua sequence IS a table, which is why the refusal says
            // dictionary: this shape has to be refused too.
            world: |w: FixtureWorld| {
                w.with_unit(
                    "steel-processing",
                    Value::Arr(vec![Value::Num(50.0), Value::Num(15.0)]),
                )
            },
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, CostOf(steel-processing): steel-processing has a unit that is not a dictionary",
        },
        Case {
            // The result handle is checked before the duplicate-name scan, so
            // a recipe with no result is told what is actually wrong instead
            // of being reported as a name collision with the recipe it
            // accidentally shares a name with.
            name: "a recipe with no result item that also collides by name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        name: "steel-axe-forging".into(),
                        ..Default::default()
                    },
                );
                l.recipe(
                    ItemRef::default(),
                    RecipeSpec {
                        name: "steel-axe-forging".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, a recipe was declared with no result item; Recipe needs an item this plan declared",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        (c.build)(&mut lib);
        match lib.plan_data(&(c.world)(base_world())) {
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

/// A handle is a fact about ONE plan. Another plan's handle is in range here
/// and points at something else entirely, so it is refused rather than
/// silently resolved into this plan's prototypes.
#[test]
fn handles_from_another_plan_are_refused() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
    }

    let cases = [
        Case {
            // The review's first shape: an in-range ItemRef that names a
            // different item in the other plan.
            name: "a result item handle from another plan",
            build: |l: &mut Lib| {
                let mut other = Lib::new();
                let bronze = other.item("bronze-axe", ItemSpec::default());
                l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    bronze,
                    RecipeSpec {
                        name: "steel-axe-forging".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, a recipe was declared with no result item; Recipe needs an item this plan declared",
        },
        Case {
            name: "an ingredient handle from another plan",
            build: |l: &mut Lib| {
                let mut other = Lib::new();
                let bronze = other.item("bronze-axe", ItemSpec::default());
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        ingredients: vec![Ingredient::of(bronze, 1)],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the recipe steel-axe names an ingredient item that this plan never declared",
        },
        Case {
            // The review's second shape: a BoolSettingRef that lands on this
            // plan's int setting, which would have read a bool out of it.
            name: "an EnabledBy handle from another plan lands on an int setting",
            build: |l: &mut Lib| {
                let mut other = Lib::new();
                let flag = other.bool_setting("hardened-tools", true);
                l.int_setting("axe-durability", 250, NumericSpec::between(50.0, 1000.0));
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        enabled_by: flag,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes names an EnabledBy setting that this plan never declared",
        },
        Case {
            name: "an unlock handle from another plan",
            build: |l: &mut Lib| {
                let mut other = Lib::new();
                let bronze = other.item("bronze-axe", ItemSpec::default());
                let rec = other.recipe(bronze, RecipeSpec::default());
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(axe, RecipeSpec::default());
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        unlocks: vec![rec],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes unlocks a recipe that this plan never declared",
        },
        Case {
            name: "an AfterTech handle from another plan",
            build: |l: &mut Lib| {
                let mut other = Lib::new();
                let anchor = other.technology(
                    "bronze-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        after_tech: anchor,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: at the data stage, the technology steel-axes names an AfterTech technology that this plan never declared",
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

/// after_tech is how a plan chains its own research: after probes the GAME,
/// so an own-tech name would log a drop and leave the second technology
/// detached.
#[test]
fn after_tech_chains_own_technologies() {
    let mut lib = Lib::new();
    let first = lib.technology(
        "bronze-axes",
        TechSpec {
            cost_of: "steel-processing".into(),
            after: "steel-processing".into(),
            ..Default::default()
        },
    );
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: "logistics-2".into(),
            after_tech: first,
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            &format!(
                r#"extend {{type="technology", name="steelworks-bronze-axes", prerequisites=["steel-processing"], unit={}}}"#,
                STEEL_PROCESSING_UNIT
            ),
            r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["steelworks-bronze-axes"], unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}"#,
        ],
    );
}

#[test]
fn plan_data_refuses_an_empty_mod_name() {
    let mut lib = Lib::new();
    lib.item("steel-axe", ItemSpec::default());

    match lib.plan_data(&base_world().with_mod_name("")) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: at the data stage, the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName"
        ),
    }
}

/// TinyGo's wasm int is 32 bits and this half's i64 is not, so the widths are
/// pinned rather than assumed. These are not realistic Factorio numbers: the
/// point is that both halves carry the same one.
#[test]
fn wide_amounts_survive_the_emit() {
    let mut lib = Lib::new();
    // 2^53 exactly: the largest integer a Lua double still holds without
    // rounding, so the planner accepts it and the transcript prints it as the
    // plain digits it is.
    let axe = lib.item(
        "steel-axe",
        ItemSpec {
            stack_size: 9_007_199_254_740_992,
            ..Default::default()
        },
    );
    lib.recipe(
        axe,
        RecipeSpec {
            ingredients: vec![Ingredient::named(3_000_000_000, "steel-plate", &[])],
            result_count: 2_500_000_000,
            ..Default::default()
        },
    );
    lib.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 5_000_000_000,
                seconds: 15.0,
                packs: vec![Pack {
                    name: "automation-science-pack".into(),
                    amount: 3_000_000_000,
                }],
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-steel-axe", stack_size=9007199254740992}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steel-plate", amount=3000000000}], results=[{type="item", name="steelworks-steel-axe", amount=2500000000}]}"#,
            r#"extend {type="technology", name="steelworks-steel-axes", unit={count=5000000000, time=15, ingredients=[["automation-science-pack", 3000000000]]}}"#,
        ],
    );
}
