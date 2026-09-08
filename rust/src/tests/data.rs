use crate::op::Op;
use crate::plan::{
    Ingredient, IngredientChoice, IngredientChoices, ItemRef, ItemSpec, Lib, NumericSpec, Pack,
    RecipeRef, RecipeSpec, TechSpec, UnitSpec,
};
use crate::tests::*;
use crate::value::{kv, Value};

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
            ..Default::default()
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

/// A FLUID IS ITS OWN NAMESPACE AND ITS OWN FIELD. The ladder asks
/// `fluid_exists`, so a rung that names an item is skipped even though the
/// game has that item, and what reaches the prototype carries type="fluid"
/// and the double the author declared.
///
/// MEASURED (2.0.77): a fluid ingredient is {type="fluid", name=..., amount=...}
/// and the engine takes a fractional amount there (0.5 and 1000000000 both
/// load and dump as written), which is why the amount is a double and not the
/// item path's integer.
#[test]
fn fluid_ingredients_carry_their_own_type_and_their_own_ladder() {
    let mut lib = Lib::new();
    let mix = lib.item("sulfuric-mix", ItemSpec::default());
    lib.recipe(
        mix,
        RecipeSpec {
            category: "chemistry".into(),
            ingredients: vec![
                Ingredient::named(2, "iron-plate", &[]),
                Ingredient::fluid(0.5, "water", &[]),
                // The first rung is an ITEM the world has and not a fluid, so
                // a ladder that asked the item question would stop here.
                Ingredient::fluid(10.0, "iron-plate", &["steam"]),
                Ingredient::fluid(1.0, "light-oil", &["lubricant"]),
            ],
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: sulfuric-mix: none of light-oil, lubricant is present, so the ingredient is dropped",
            r#"extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-sulfuric-mix", category="chemistry", enabled=true, ingredients=[{type="item", name="iron-plate", amount=2}, {type="fluid", name="water", amount=5.0000000000000000e-1}, {type="fluid", name="steam", amount=10}], results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}"#,
        ],
    );
}

/// The engine refuses a fluid in the crafting category by name (measured:
/// "Recipe is in \'crafting\' category but has a non-item ingredient
/// \'water\' (fluid)."), and an empty category IS crafting. The plan refuses
/// first, so the consumer reads a sentence naming their own recipe instead of
/// the engine naming a prototype they did not write by hand.
#[test]
fn plan_data_refuses_a_declared_fluid_a_recipe_cannot_take() {
    struct Case {
        name: &'static str,
        category: &'static str,
        ingredient: fn() -> Ingredient,
        want: &'static str,
    }

    let cases = [
        Case {
            name: "the default category",
            category: "",
            ingredient: || Ingredient::fluid(10.0, "water", &["steam"]),
            want: "fkrecipes: the recipe sulfuric-mix takes the fluid water, and a recipe in the crafting category takes items only",
        },
        Case {
            name: "the crafting category, spelled out",
            category: "crafting",
            ingredient: || Ingredient::fluid(10.0, "water", &[]),
            want: "fkrecipes: the recipe sulfuric-mix takes the fluid water, and a recipe in the crafting category takes items only",
        },
        Case {
            name: "an amount the engine refuses",
            category: "chemistry",
            ingredient: || Ingredient::fluid(0.0, "water", &[]),
            want: "fkrecipes: the recipe sulfuric-mix has a fluid amount at or below zero, which the engine refuses",
        },
        Case {
            name: "an amount that is not a number",
            category: "chemistry",
            ingredient: || Ingredient::fluid(f64::NAN, "water", &[]),
            want: "fkrecipes: the recipe sulfuric-mix declares a fluid amount that is not a finite number",
        },
        Case {
            // The measured ceiling, asked of the AUTHOR's declaration: above
            // it the engine does not refuse the load, it aborts inside
            // FixedPointNumber and hands the player the crash handler. The
            // player-typed path has had this since the language landed;
            // without it here, only one of the two ways into a recipe was
            // guarded.
            name: "an amount above the measured ceiling",
            category: "chemistry",
            ingredient: || Ingredient::fluid(1e302, "water", &[]),
            want: "fkrecipes: the recipe sulfuric-mix takes the fluid water at an amount above 1e301, which the game cannot hold",
        },
        Case {
            // THE CATEGORY BEATS THE CEILING, the same way round as in the
            // player's path (see the language's own
            // a_crafting_recipe_hears_about_the_category_before_the_ceiling):
            // a fluid this recipe cannot take at all is the larger mistake.
            name: "an amount above the ceiling in a category that takes no fluid",
            category: "crafting",
            ingredient: || Ingredient::fluid(1e302, "water", &[]),
            want: "fkrecipes: the recipe sulfuric-mix takes the fluid water, and a recipe in the crafting category takes items only",
        },
        Case {
            // The ladder's FIRST candidate is what the sentence names, and an
            // empty one would leave a hole in it. That is why the empty-name
            // refusal runs ahead of both.
            name: "a fluid with an empty name",
            category: "chemistry",
            ingredient: || Ingredient::fluid(10.0, "", &["water"]),
            want: "fkrecipes: the recipe sulfuric-mix names an ingredient with an empty name",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        let mix = lib.item("sulfuric-mix", ItemSpec::default());
        lib.recipe(
            mix,
            RecipeSpec {
                category: c.category.into(),
                ingredients: vec![(c.ingredient)()],
                ..Default::default()
            },
        );
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

/// The same rule reaches a fluid a DROPDOWN would have chosen: every choice
/// is validated, not just the default one, because the player picking the
/// third preset is not a load failure the mod author should hear about from
/// the engine.
#[test]
fn plan_data_refuses_a_fluid_inside_a_choice_a_player_could_pick() {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("quench-medium", "dry", &["dry", "wet"]);
    let mix = lib.item("sulfuric-mix", ItemSpec::default());
    lib.recipe(
        mix,
        RecipeSpec {
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![
                    IngredientChoice {
                        value: "dry".into(),
                        ingredients: vec![Ingredient::named(2, "iron-plate", &[])],
                    },
                    IngredientChoice {
                        value: "wet".into(),
                        ingredients: vec![Ingredient::fluid(10.0, "water", &[])],
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    match lib.plan_data(&base_world()) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the recipe sulfuric-mix takes the fluid water, and a recipe in the crafting category takes items only"
        ),
    }
}

/// A fluid ladder with nothing present drops the ingredient and says so,
/// exactly as the item ladder does: the two kinds share the sentence because
/// they share the decision.
#[test]
fn a_fluid_ladder_with_no_rung_present_drops_the_ingredient() {
    let mut lib = Lib::new();
    let mix = lib.item("sulfuric-mix", ItemSpec::default());
    lib.recipe(
        mix,
        RecipeSpec {
            category: "chemistry".into(),
            ingredients: vec![Ingredient::fluid(2.0, "water", &["steam"])],
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(&base_world().without_fluid("water").without_fluid("steam"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: sulfuric-mix: none of water, steam is present, so the ingredient is dropped",
            r#"extend {type="item", name="steelworks-sulfuric-mix", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-sulfuric-mix", category="chemistry", enabled=true, ingredients=[], results=[{type="item", name="steelworks-sulfuric-mix", amount=1}]}"#,
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
                packs: vec![Pack::new("automation-science-pack", 1)],
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
                    Pack::new("automation-science-pack", 1),
                    Pack::new("logistic-science-pack", 2),
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

/// A PACK IS A LADDER, and it is walked through `tool_exists` and nothing
/// else.
///
/// The middle rung is the whole test: `chemical-science-pack` is an ITEM this
/// world has and is not one of its tools, so a walk that asked the item
/// question would stop there and price the research in something the engine
/// refuses ("Invalid research unit (...). Research unit(s) can only be tool
/// type items at the moment."). The rung after it is a real tool, and that is
/// the one that must come out.
#[test]
fn a_pack_ladder_takes_the_first_rung_the_game_has() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 75,
                seconds: 30.0,
                packs: vec![Pack::named(
                    2,
                    "military-science-pack",
                    &["chemical-science-pack", "logistic-science-pack"],
                )],
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    // No log line: a ladder that finds a rung degrades nothing and says
    // nothing, exactly as an ingredient ladder does.
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["logistic-science-pack", 2]]}}"#,
        ],
    );
}

/// A PACK WHOSE RUNGS ARE ALL ABSENT IS DROPPED, with the ingredient drop's
/// own sentence and one word changed, and the research goes out cheaper. A
/// unit left with NO pack at all is the one thing this refuses, because that
/// research cannot be paid for at any price.
///
/// This is the pair the review turned over: untouched packs used to refuse
/// where untouched ingredients dropped, which made a modpack that renamed the
/// science packs a hard load failure with the consumer's name on it.
#[test]
fn a_pack_ladder_drops_and_a_unit_with_nothing_left_is_refused() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 75,
                seconds: 30.0,
                packs: vec![
                    Pack::new("automation-science-pack", 1),
                    Pack::named(3, "military-science-pack", &["space-science-pack"]),
                ],
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steel-axes: none of military-science-pack, space-science-pack is present, so the science pack is dropped",
            r#"extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["automation-science-pack", 1]]}}"#,
        ],
    );

    // The same plan with nothing left standing. Both packs drop, and what
    // would have been emitted is a technology nobody can research.
    let mut nothing = Lib::new();
    nothing.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 75,
                seconds: 30.0,
                packs: vec![
                    Pack::new("military-science-pack", 1),
                    Pack::named(3, "space-science-pack", &["metallurgic-science-pack"]),
                ],
            }),
            ..Default::default()
        },
    );
    match nothing.plan_data(&base_world()) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the technology steel-axes has no science pack the game has; research takes at least one"
        ),
    }
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

/// THE THREE POST-RESOLUTION CHECKS HAVE ONE ORDER, and a plan carrying all
/// of their problems at once is what pins it: the resolved crafting times,
/// then the science packs the game actually has, then the cycle walk.
///
/// Each of these is defensible in another order, so only a witness makes one
/// of them the language. Both halves report the same sentence about the same
/// plan or they are two libraries.
#[test]
fn the_post_resolution_checks_report_in_their_fixed_order() {
    // Somebody's overhaul rang two of the game's own technologies together,
    // which the cycle walk finds whether or not the plan touches them.
    let ringed = || base_world().with_prereqs("logistics-2", &["logistics", "logistics-3"]);
    // A pack the game does not have, so the ladder drops it and the unit is
    // left with nothing.
    let unpayable = || TechSpec {
        unit: Some(UnitSpec {
            count: 50,
            seconds: 15.0,
            packs: vec![Pack::new("military-science-pack", 1)],
        }),
        ..Default::default()
    };

    // ALL THREE AT ONCE: the crafting time is what is reported.
    let mut all_three = Lib::new();
    let axe = all_three.item("steel-axe", ItemSpec::default());
    let from = all_three.double_setting("axe-craft-time", 2.5, NumericSpec::default());
    all_three.recipe(
        axe,
        RecipeSpec {
            craft_time_from: from,
            ..Default::default()
        },
    );
    all_three.technology("steel-axes", unpayable());
    let world = ringed().with_setting("steelworks-axe-craft-time", Value::Num(0.001));
    match all_three.plan_data(&world) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, which answers at or below the engine floor (energy_required can't be <= 0.001)"
        ),
    }

    // The same plan with the crafting time taken out of the argument: the
    // packs beat the ring.
    let mut two = Lib::new();
    let axe = two.item("steel-axe", ItemSpec::default());
    two.recipe(
        axe,
        RecipeSpec {
            craft_time: 2.5,
            ..Default::default()
        },
    );
    two.technology("steel-axes", unpayable());
    match two.plan_data(&ringed()) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the technology steel-axes has no science pack the game has; research takes at least one"
        ),
    }

    // And the ring on its own is still found, so the two rows above are an
    // ordering and not a walk that never ran.
    let mut ring_only = Lib::new();
    ring_only.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 50,
                seconds: 15.0,
                packs: vec![Pack::new("automation-science-pack", 1)],
            }),
            ..Default::default()
        },
    );
    match ring_only.plan_data(&ringed()) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: a prerequisite cycle: logistics-2 -> logistics-3 -> logistics-2"
        ),
    }
}

/// DECLARATION ORDER, not name order: two technologies that both lose every
/// pack are reported as the FIRST one declared, so the author reads about the
/// one they wrote first rather than about whichever name sorts earlier.
#[test]
fn the_all_dropped_refusal_names_the_first_technology_declared() {
    let unpayable = || TechSpec {
        unit: Some(UnitSpec {
            count: 50,
            seconds: 15.0,
            packs: vec![Pack::new("military-science-pack", 1)],
        }),
        ..Default::default()
    };
    let mut lib = Lib::new();
    lib.technology("bbb-second", unpayable());
    lib.technology("aaa-first", unpayable());

    match lib.plan_data(&base_world()) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the technology bbb-second has no science pack the game has; research takes at least one"
        ),
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
            want: "fkrecipes: two items share the name steelworks-steel-axe; the second would overwrite the first",
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
            want: "fkrecipes: two recipes share the name steelworks-steel-axe-forging; the second would overwrite the first",
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
            want: "fkrecipes: two technologies share the name steelworks-steel-axes; the second would overwrite the first",
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
            want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
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
            want: "fkrecipes: the recipe steel-axe names an ingredient item that this plan never declared",
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
            want: "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit, CostBy or CostFrom",
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
                            packs: vec![Pack::new("automation-science-pack", 1)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit, CostBy or CostFrom",
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
            want: "fkrecipes: the technology steel-axes names Before without After; InsertBetween needs both ends",
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
                            packs: vec![Pack::new("automation-science-pack", 1)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes has a unit count below 1, which the engine refuses",
        },
        Case {
            // A pack the game does not have is DROPPED, like an ingredient;
            // it is the unit left with nothing at all that is refused, and
            // this unit had one pack to lose. The drop line itself is
            // asserted in a_pack_ladder_drops_and_a_unit_with_nothing_left_is_refused.
            name: "a unit whose only science pack the game does not have",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 15.0,
                            packs: vec![Pack::new("military-science-pack", 1)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes has no science pack the game has; research takes at least one",
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
            want: "fkrecipes: CostOf(logistics-4): no technology of that name exists",
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
            want: "fkrecipes: CostOf(steam-power): steam-power is a research_trigger technology with no unit to copy; name a unit-carrying technology instead",
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
            want: "fkrecipes: the technology steel-axes unlocks a recipe that this plan never declared",
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
            want: "fkrecipes: the technology steel-axes names an EnabledBy setting that this plan never declared",
        },
        Case {
            name: "an item this plan would overwrite",
            world: |w: FixtureWorld| w.with_item("steelworks-steel-axe"),
            build: |l: &mut Lib| {
                l.item("steel-axe", ItemSpec::default());
            },
            want: "fkrecipes: the item steelworks-steel-axe already exists in data.raw; this plan would overwrite it",
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
            want: "fkrecipes: the recipe steelworks-steel-axe-forging already exists in data.raw; this plan would overwrite it",
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
            want: "fkrecipes: the technology steelworks-steel-axes already exists in data.raw; this plan would overwrite it",
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
            want: "fkrecipes: the technology steel-axes names both After and AfterTech; pick one anchor",
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
            want: "fkrecipes: the technology steel-axes names Before with AfterTech; InsertBetween splices around a technology that already exists",
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
            want: "fkrecipes: the recipe steel-axe declares a crafting time that is not a finite number",
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
                            packs: vec![Pack::new("automation-science-pack", 1)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes declares a research time that is not a finite number",
        },
        Case {
            name: "an item with an empty name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.item("", ItemSpec::default());
            },
            want: "fkrecipes: an item was declared with an empty name",
        },
        Case {
            name: "a technology with an empty name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: a technology was declared with an empty name",
        },
        Case {
            // A holed or mixed Lua table crosses as a number-keyed map, which
            // converts to nil as a whole subtree rather than being half kept.
            // A unit that lost its ingredient list is a technology researchable
            // for free, so the copy is refused rather than emitted.
            name: "a CostOf source whose unit lost a subtree on the way in",
            world: |w: FixtureWorld| {
                w.with_unit(
                    "steel-processing",
                    Value::Map(vec![
                        kv("count", Value::Num(50.0)),
                        kv("ingredients", Value::Nil),
                        kv("time", Value::Num(15.0)),
                    ]),
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
            want: "fkrecipes: CostOf(steel-processing): the unit of steel-processing holds a table this library cannot copy faithfully",
        },
        Case {
            name: "a science pack with an empty name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 15.0,
                            packs: vec![Pack::new("", 1)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes prices itself in a pack with an empty name",
        },
        Case {
            name: "both a fixed and a bound crafting time",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let from = l.double_setting("axe-craft-time", 2.5, NumericSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time: 2.5,
                        craft_time_from: from,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe names both CraftTime and CraftTimeFrom; pick one",
        },
        Case {
            name: "a crafting-time setting from another plan",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let mut other = Lib::new();
                let stray = other.double_setting("axe-craft-time", 2.5, NumericSpec::default());
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: stray,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe names a crafting-time setting that this plan never declared",
        },
        Case {
            // Measured: the engine refuses energy_required <= 0.001.
            name: "a declared crafting time below the engine floor",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time: 0.001,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe declares a crafting time the engine refuses (energy_required can't be <= 0.001)",
        },
        Case {
            // The generated setting's own minimum clears the floor, so this is
            // what a colliding mod's setting looks like: same name, same type,
            // last declaration wins, silently.
            name: "a bound crafting time answered below the engine floor",
            world: |w: FixtureWorld| {
                w.with_setting("steelworks-axe-craft-time", Value::Num(0.001))
            },
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let from = l.double_setting("axe-craft-time", 2.5, NumericSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: from,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, which answers at or below the engine floor (energy_required can't be <= 0.001)",
        },
        Case {
            // An infinity is ABOVE the floor, so the floor arm would wave it
            // through and ship a recipe that never completes.
            name: "a bound crafting time answered as an infinity",
            world: |w: FixtureWorld| {
                w.with_setting("steelworks-axe-craft-time", Value::Num(f64::INFINITY))
            },
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let from = l.double_setting("axe-craft-time", 2.5, NumericSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: from,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, which answers a value that is not a finite number",
        },
        Case {
            // A NaN compares false against the floor, so it reached the floor
            // arm and was reported as a value at or below it, which it is not.
            name: "a bound crafting time answered as a NaN",
            world: |w: FixtureWorld| {
                w.with_setting("steelworks-axe-craft-time", Value::Num(f64::NAN))
            },
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let from = l.double_setting("axe-craft-time", 2.5, NumericSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time_from: from,
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, which answers a value that is not a finite number",
        },
        Case {
            // PRESENT and nil, which is what a unit whose table carried a
            // numeric key collapses to: a different answer from "no unit".
            name: "a CostOf source whose unit arrives as nil",
            world: |w: FixtureWorld| w.with_nil_unit("steel-processing"),
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: CostOf(steel-processing): steel-processing has a unit that is not a dictionary",
        },
        Case {
            name: "a CostOf source whose max_level lost a subtree on the way in",
            world: |w: FixtureWorld| {
                w.with_max_level(
                    "steel-processing",
                    Value::Map(vec![kv("levels", Value::Nil)]),
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
            want: "fkrecipes: CostOf(steel-processing): the max_level of steel-processing holds a table this library cannot copy faithfully",
        },
        Case {
            // The World says the technology is there and is not a research
            // trigger, but hands back no unit: the arm the emit layer's
            // tech_unit read lands on.
            name: "a CostOf source that carries no unit at all",
            world: |w: FixtureWorld| w.with_unit("steel-processing", Value::Nil),
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        cost_of: "steel-processing".into(),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: CostOf(steel-processing): steel-processing carries no unit to copy",
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
            want: "fkrecipes: the item steel-axe has a negative stack size, which the engine refuses",
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
            want: "fkrecipes: the item steel-axe has a negative icon size, which the engine refuses",
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
            want: "fkrecipes: the technology steel-axes has a negative icon size, which the engine refuses",
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
            want: "fkrecipes: the recipe steel-axe has an ingredient amount below 1, which the engine refuses",
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
            want: "fkrecipes: the recipe steel-axe has a negative result count, which the engine refuses",
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
            want: "fkrecipes: the recipe steel-axe has a negative crafting time, which the engine refuses",
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
                            packs: vec![Pack::new("automation-science-pack", 0)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes has a science pack amount below 1, which the engine refuses",
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
                            packs: vec![Pack::new("automation-science-pack", 1)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes has a research time at or below zero, which the engine refuses",
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
            want: "fkrecipes: the item steel-axe declares a stack size a Lua double cannot hold exactly: 9007199254740993",
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
            want: "fkrecipes: the recipe steel-axe declares an ingredient amount a Lua double cannot hold exactly: 9007199254740993",
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
                            packs: vec![Pack::new("automation-science-pack", 1)],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes declares a unit count a Lua double cannot hold exactly: 9007199254740993",
        },
        Case {
            // ITEM AND FLUID ALIKE, and a FALLBACK rung as well as a first
            // choice: a rung that can never resolve silently shortens the
            // ladder the author wrote, exactly as an empty pack rung does.
            name: "an ingredient with an empty name",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        ingredients: vec![Ingredient::named(2, "", &[])],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe names an ingredient with an empty name",
        },
        Case {
            name: "an ingredient with an empty fallback rung",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                l.recipe(
                    axe,
                    RecipeSpec {
                        ingredients: vec![Ingredient::named(2, "steel-plate", &["", "iron-plate"])],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe names an ingredient with an empty name",
        },
        Case {
            // A UNIT THAT NAMED NO PACK AT ALL, refused before any world
            // question. The sentence about packs the game does not have is
            // reserved for a list that named some and lost them all, which is
            // the row above this one.
            name: "a unit declared with no science pack",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 15.0,
                            packs: Vec::new(),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes declares no science pack; research takes at least one",
        },
        Case {
            // The placement of that check, pinned: the count is asked first,
            // so a unit with both problems hears about the count. The
            // fallback fixture in the migration suite rests on this order.
            name: "a unit with no packs and a count below one",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 0,
                            seconds: 15.0,
                            packs: Vec::new(),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes has a unit count below 1, which the engine refuses",
        },
        Case {
            // The empty rung INSIDE a ladder, which the first-choice row
            // above does not reach.
            name: "a science pack with an empty fallback rung",
            world: |w: FixtureWorld| w,
            build: |l: &mut Lib| {
                l.technology(
                    "steel-axes",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 50,
                            seconds: 15.0,
                            packs: vec![Pack::named(
                                1,
                                "automation-science-pack",
                                &["", "logistic-science-pack"],
                            )],
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology steel-axes prices itself in a pack with an empty name",
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
            want: "fkrecipes: CostOf(steel-processing): steel-processing has a unit that is not a dictionary",
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
            want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
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
            want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
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
            want: "fkrecipes: the recipe steel-axe names an ingredient item that this plan never declared",
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
            want: "fkrecipes: the technology steel-axes names an EnabledBy setting that this plan never declared",
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
            want: "fkrecipes: the technology steel-axes unlocks a recipe that this plan never declared",
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
            want: "fkrecipes: the technology steel-axes names an AfterTech technology that this plan never declared",
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
            "fkrecipes: the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName"
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
            // The INGREDIENT rides its own ceiling rather than a wide number:
            // an item amount goes up to 65535 (the engine's u16, measured) and
            // a declared list is held to that exactly as a typed one is. The
            // wide number in this recipe is the result count, which has no
            // such ceiling.
            ingredients: vec![Ingredient::named(65_535, "steel-plate", &[])],
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
                packs: vec![Pack::new("automation-science-pack", 3_000_000_000)],
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-steel-axe", stack_size=9007199254740992}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steel-plate", amount=65535}], results=[{type="item", name="steelworks-steel-axe", amount=2500000000}]}"#,
            r#"extend {type="technology", name="steelworks-steel-axes", unit={count=5000000000, time=15, ingredients=[["automation-science-pack", 3000000000]]}}"#,
        ],
    );
}

/// A max_level the World reports as present but nil is a value this library
/// could not carry across; writing it would put a nil into the prototype.
#[test]
fn a_nil_max_level_is_not_emitted() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: "steel-processing".into(),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(&base_world().with_nil_max_level("steel-processing"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[&alloc::format!(
            r#"extend {{type="technology", name="steelworks-steel-axes", unit={}}}"#,
            STEEL_PROCESSING_UNIT
        )],
    );
}

/// A Set op's value reaches fkdata::set, where a nil DELETES the key instead
/// of writing one. Nothing plans a deletion, and this is what says so.
#[test]
fn no_planned_set_op_carries_a_nil_value() {
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

    let mut sets = 0;
    for op in &ops {
        if let Op::Set(path, val) = op {
            sets += 1;
            assert!(
                !matches!(val, Value::Nil),
                "a Set op at {} carries nil, which would delete the key",
                render_path(path)
            );
            if let Value::Arr(entries) = val {
                for entry in entries {
                    assert!(
                        !matches!(entry, Value::Nil),
                        "a Set op at {} carries a nil entry",
                        render_path(path)
                    );
                }
            }
        }
    }
    assert_eq!(sets, 2, "expected two splices");
}

/// The whole binding, end to end: the settings stage generates the setting
/// with a minimum that clears the engine floor, and the data stage reads the
/// player's answer back into energy_required.
#[test]
fn craft_time_binding_round_trip() {
    let mut lib = Lib::new();
    let axe = lib.item("steel-axe", ItemSpec::default());
    let from = lib.double_setting("axe-craft-time", 2.5, NumericSpec::default());
    lib.recipe(
        axe,
        RecipeSpec {
            craft_time_from: from,
            ingredients: vec![Ingredient::named(4, "steel-plate", &[])],
            ..Default::default()
        },
    );

    let settings_ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_lines(
        &transcript(&settings_ops),
        &[
            r#"extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=2.5000000000000000e0, order="aa", minimum_value=2.0000000000000000e-3}"#,
        ],
    );

    // The player set it to four seconds.
    let data_ops = lib
        .plan_data(&base_world().with_setting("steelworks-axe-craft-time", Value::Num(4.0)))
        .expect("plan refused");
    assert_lines(
        &transcript(&data_ops),
        &[
            r#"extend {type="item", name="steelworks-steel-axe", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", energy_required=4, enabled=true, ingredients=[{type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}"#,
        ],
    );
}

/// An unreadable setting degrades the same way an unreadable enablement does:
/// one log line, and the declared default applies.
#[test]
fn craft_time_binding_falls_back_to_its_default() {
    let mut lib = Lib::new();
    let axe = lib.item("steel-axe", ItemSpec::default());
    let from = lib.double_setting("axe-craft-time", 2.5, NumericSpec::default());
    lib.recipe(
        axe,
        RecipeSpec {
            craft_time_from: from,
            ..Default::default()
        },
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-axe-craft-time was not readable, so its default applies",
            r#"extend {type="item", name="steelworks-steel-axe", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-steel-axe", energy_required=2.5000000000000000e0, enabled=true, ingredients=[], results=[{type="item", name="steelworks-steel-axe", amount=1}]}"#,
        ],
    );
}

/// The same, for the two technologies the split-emission test prices from.
pub(crate) const LOGISTICS_2_UNIT: &str = r#"{count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}"#;
pub(crate) const LOGISTICS_3_UNIT: &str = r#"{count=400, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], time=60}"#;

fn logistics_2_unit_value() -> Value {
    unit_of(
        200,
        30.0,
        &["automation-science-pack", "logistic-science-pack"],
    )
}

/// THE ONE-DATA-HOOK RULE IS PER Lib, NOT PER MOD, and this is what says so.
///
/// A single Lib emitted from two data-family hooks refuses, because the second
/// pass finds the first pass's prototypes already in data.raw and reports the
/// overwrite. That is a real rule and it is not the rule "a mod gets one data
/// hook": a mod may carry TWO plans, one creating its own content at `fk_data`
/// and one patching another mod's tree at `fk_data_updates`, each emitting its
/// own settings at `fk_settings`. The stages share one data.raw, so what makes
/// it work is that the two plans declare different names; nothing else is
/// needed.
///
/// The patching plan is planned against a world that carries the creating
/// plan's prototypes, which is what data.raw actually looks like by the time
/// `fk_data_updates` runs.
#[test]
fn two_libs_split_creation_from_patching() {
    // PLAN A, at fk_data: this mod's own content.
    let mut creation = Lib::new();
    let hardened = creation.bool_setting("hardened-tools", true);
    let plate = creation.item("hardened-steel-plate", ItemSpec::default());
    creation.recipe(
        plate,
        RecipeSpec {
            ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
            ..Default::default()
        },
    );
    creation.technology(
        "hardened-steel",
        TechSpec {
            cost_of: String::from("logistics-2"),
            after: String::from("steel-processing"),
            enabled_by: hardened,
            ..Default::default()
        },
    );

    // The setting is answered rather than left unreadable, so the transcript
    // is the split itself and not a degradation log.
    let a_ops = creation
        .plan_data(&base_world().with_setting("steelworks-hardened-tools", Value::Bool(true)))
        .expect("the creating plan was refused");
    assert_lines(
        &transcript(&a_ops),
        &[
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-hardened-steel", prerequisites=["steel-processing"], unit={}, enabled=true}}"#,
                LOGISTICS_2_UNIT
            ),
        ],
    );

    // data.raw AS PLAN A LEFT IT. The patching plan runs a stage later, so
    // everything above is already there and is what it plans against.
    let after = base_world()
        .with_setting("steelworks-deep-tempering", Value::Bool(true))
        .with_item("steelworks-hardened-steel-plate")
        .with_recipe("steelworks-hardened-steel-plate")
        .with_tech(FixtureTech {
            name: String::from("steelworks-hardened-steel"),
            prereqs: alloc::vec![String::from("steel-processing")],
            unit: logistics_2_unit_value(),
            max_level: Value::Nil,
            trigger: false,
        });

    // PLAN B, at fk_data_updates: a patch, spliced around plan A's OWN
    // technology by name. It cannot use after_tech, because that takes a
    // handle and handles do not cross plans; a name is how one plan reaches
    // another's emitted prototype, which is the same way it reaches any other
    // mod's.
    let mut patch = Lib::new();
    let deep = patch.bool_setting("deep-tempering", true);
    patch.technology(
        "tempering",
        TechSpec {
            cost_of: String::from("logistics-3"),
            after: String::from("steel-processing"),
            before: String::from("steelworks-hardened-steel"),
            enabled_by: deep,
            ..Default::default()
        },
    );

    let b_ops = patch
        .plan_data(&after)
        .expect("the patching plan was refused");
    assert_lines(
        &transcript(&b_ops),
        &[
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-tempering", prerequisites=["steel-processing"], unit={}, enabled=true}}"#,
                LOGISTICS_3_UNIT
            ),
            r#"set technology.steelworks-hardened-steel.prerequisites = ["steelworks-tempering"]"#,
        ],
    );

    // Each plan emits ITS OWN settings, and the two sets are disjoint: the
    // settings stage runs once, so both hooks route into it and a shared name
    // would be the silent last-writer-wins the settings validator refuses
    // within one plan and cannot see across two.
    let a_settings = creation
        .plan_settings(&settings_world())
        .expect("plan refused");
    let b_settings = patch
        .plan_settings(&settings_world())
        .expect("plan refused");
    assert_lines(
        &transcript(&a_settings),
        &[
            r#"extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="aa"}"#,
        ],
    );
    assert_lines(
        &transcript(&b_settings),
        &[
            r#"extend {type="bool-setting", name="steelworks-deep-tempering", setting_type="startup", default_value=true, order="aa"}"#,
        ],
    );
}

/// The other half of the rule, so the pair cannot both pass on a library that
/// never refuses an overwrite: two plans that DO share a name are refused, and
/// the refusal comes from the world carrying the first plan's prototype rather
/// than from anything the second plan knows about the first.
#[test]
fn a_second_lib_sharing_a_name_is_still_refused() {
    let mut patch = Lib::new();
    patch.technology(
        "hardened-steel",
        TechSpec {
            cost_of: String::from("logistics-3"),
            ..Default::default()
        },
    );

    let after = base_world().with_tech(FixtureTech {
        name: String::from("steelworks-hardened-steel"),
        prereqs: alloc::vec![String::from("steel-processing")],
        unit: logistics_2_unit_value(),
        max_level: Value::Nil,
        trigger: false,
    });

    match patch.plan_data(&after) {
        Ok(ops) => panic!(
            "the plan was accepted with {} ops, want an overwrite refusal",
            ops.len()
        ),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: the technology steelworks-hardened-steel already exists in data.raw; this plan would overwrite it"
        ),
    }
}
