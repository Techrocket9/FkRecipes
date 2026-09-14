//! The three surfaces a mod with hand-rolled settings needs to migrate onto
//! this library: names it can keep, ingredients a dropdown chooses, and a
//! research cost a dropdown chooses.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::plan::{
    CostChoice, CostChoices, CustomCost, Ingredient, IngredientChoice, IngredientChoices, ItemRef,
    ItemSpec, Lib, NumericSpec, Pack, RecipeSpec, TechSpec, UnitSpec,
};
use crate::tests::data::{LOGISTICS_2_UNIT, STEEL_PROCESSING_UNIT};
use crate::tests::{assert_composed, assert_lines, base_world, settings_world, transcript};
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
            want: "fkrecipes: a setting was declared with an empty name",
        },
        Case {
            name: "an empty order",
            build: |l| {
                l.legacy_bool_setting("bbb-enabled", true, "");
            },
            want: "fkrecipes: the legacy setting bbb-enabled was declared with an empty order",
        },
        Case {
            // The names differ as declared and collide as emitted, which is
            // the namespace the engine keeps.
            name: "a legacy name colliding with a generated one",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.legacy_bool_setting("steelworks-hardened-tools", false, "a");
            },
            want: "fkrecipes: two settings share the name steelworks-hardened-tools; the engine keeps the last one silently",
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
// Where a generated setting lands.
// ---------------------------------------------------------------------------

/// The pilot's plan, built once because several tests below hold it to the
/// same orders: two legacy dropdowns under the orders the mod already ships,
/// and six generated settings placed behind them, ONE FROM EVERY CONSTRUCTOR
/// that captures the placement. A constructor that stopped capturing would
/// move its own setting's order and only its own, which is what makes the
/// transcript below a witness for each of the six separately.
fn pilot_order_plan(lib: &mut Lib) {
    lib.legacy_dropdown_setting_needing_locale(
        "bbb-recipe-cost",
        "vanilla",
        &["vanilla", "cheap", "custom"],
        "a",
    );
    lib.order_after("a");
    let list = lib.ingredients_setting(
        "recipe-ingredients",
        vec![Ingredient::named(2, "steel-plate", &[])],
    );
    lib.bool_setting("recipe-hint", true);
    lib.legacy_dropdown_setting_needing_locale(
        "bbb-tech-cost",
        "logistics",
        &["logistics", "custom"],
        "b",
    );
    lib.order_after("b");
    let packs = lib.packs_setting("tech-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tech-count", 20, NumericSpec::between(1.0, 1000000.0));
    let seconds = lib.int_setting("tech-seconds", 15, NumericSpec::between(1.0, 3600.0));
    lib.dropdown_setting_needing_locale("tech-style", "plain", &["plain", "fancy"]);
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_from: Some(list),
            ..Default::default()
        },
    );
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            ..Default::default()
        },
    );
}

/// A GENERATED SETTING THE CONSUMER PLACES, which is what the two letters
/// alone cannot do. They count DECLARATION SLOTS, the legacy declarations
/// among them, running "aa" to "az" and then "ba": beside the orders "a" and
/// "b", a generated setting in any of the first twenty-six slots lands
/// BETWEEN the two dropdowns and the twenty-seventh declaration lands past
/// the second. For the pilot's research customizer that put the packs, the
/// count and the seconds above the dropdown that switches them on, by
/// arithmetic rather than by choice. The pilot's answer was to declare all
/// four as legacy purely to place them, hand-writing four prefixed names;
/// `order_after` places them and keeps the names.
#[test]
fn order_after_places_generated_settings_behind_a_named_order() {
    let mut lib = Lib::new();
    pilot_order_plan(&mut lib);

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_composed(
        &transcript(&ops),
        &[
            r#"extend {type="string-setting", name="bbb-recipe-cost", setting_type="startup", default_value="vanilla", order="a", allowed_values=["vanilla", "cheap", "custom"]}"#,
            r#"extend {type="string-setting", name="steelworks-recipe-ingredients", setting_type="startup", default_value="default", order="aab", auto_trim=true, localised_description=["", ["mod-setting-description.steelworks-recipe-ingredients"], "\ndefault: 2 steel-plate"<text tail>]}"#,
            r#"extend {type="bool-setting", name="steelworks-recipe-hint", setting_type="startup", default_value=true, order="aac"}"#,
            r#"extend {type="string-setting", name="bbb-tech-cost", setting_type="startup", default_value="logistics", order="b", allowed_values=["logistics", "custom"]}"#,
            r#"extend {type="string-setting", name="steelworks-tech-packs", setting_type="startup", default_value="default", order="bae", auto_trim=true, localised_description=["", ["mod-setting-description.steelworks-tech-packs"], "\ndefault: 1 automation-science-pack"<text tail>]}"#,
            r#"extend {type="int-setting", name="steelworks-tech-count", setting_type="startup", default_value=20, order="baf", minimum_value=1, maximum_value=1000000, localised_description=["", ["mod-setting-description.steelworks-tech-count"], "\nA whole number from 1 to 1000000."]}"#,
            r#"extend {type="int-setting", name="steelworks-tech-seconds", setting_type="startup", default_value=15, order="bag", minimum_value=1, maximum_value=3600, localised_description=["", ["mod-setting-description.steelworks-tech-seconds"], "\nA whole number from 1 to 3600."]}"#,
            r#"extend {type="string-setting", name="steelworks-tech-style", setting_type="startup", default_value="plain", order="bah", allowed_values=["plain", "fancy"]}"#,
        ],
    );
}

/// THE LINE THE PAST RULE DRAWS, from the side it does not refuse. Twenty-four
/// settings placed under "a" carry "aac" through "aaz", every one of them
/// still before a legacy "ab": the plan is accepted whole, and the last of
/// them is pinned because it is the boundary the next declaration crosses.
#[test]
fn order_after_accepts_placed_settings_that_stay_under_the_named_order() {
    let mut lib = Lib::new();
    lib.legacy_bool_setting("bbb-cost", false, "a");
    lib.legacy_bool_setting("bbb-cost-detail", false, "ab");
    lib.order_after("a");
    for i in 0..24 {
        lib.bool_setting(&format!("hardened-tools-{}", i), true);
    }

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    let lines = transcript(&ops);
    assert_eq!(lines.len(), 26, "the plan lost a setting");
    assert_eq!(
        lines[lines.len() - 1],
        r#"extend {type="bool-setting", name="steelworks-hardened-tools-23", setting_type="startup", default_value=true, order="aaz"}"#
    );
}

/// A PLAN THAT NEVER PLACES ANYTHING KEEPS ITS LEGACY ORDERS. Every order
/// extends the empty string, and a legacy "a" sorts before the first generated
/// setting's "aa", so a past rule that ran without a prefix would refuse the
/// ordinary migration this library emitted before `order_after` existed. The
/// tie rule still applies here, which the refusals below say.
#[test]
fn a_plan_that_places_nothing_keeps_a_legacy_order_it_sorts_after() {
    let mut lib = Lib::new();
    lib.bool_setting("hardened-tools", true);
    lib.legacy_bool_setting("bbb-enabled", false, "a");

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="aa"}"#,
            r#"extend {type="bool-setting", name="bbb-enabled", setting_type="startup", default_value=false, order="a"}"#,
        ],
    );
}

/// A LEGACY SETTING KEEPS ITS OWN ORDER WHATEVER PLACEMENT IS IN FORCE. What
/// this holds is `emitted_order`'s first line, which answers with the
/// declared order before it reads the captured prefix at all; it says nothing
/// about what the constructor stored, because every declaration captures the
/// placement in force and a legacy one's is read by nothing. The text
/// constructor is the one worth saying it about: its body is shared with the
/// generated surface, so a captured placement passes through it.
#[test]
fn a_legacy_setting_declared_under_a_placement_keeps_its_own_order() {
    let mut lib = Lib::new();
    lib.order_after("a");
    let list = lib.legacy_ingredients_setting(
        "bbb-recipe-parts",
        vec![Ingredient::named(2, "steel-plate", &[])],
        "c",
    );
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_from: Some(list),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_composed(
        &transcript(&ops),
        &[
            r#"extend {type="string-setting", name="bbb-recipe-parts", setting_type="startup", default_value="default", order="c", auto_trim=true, localised_description=["", ["mod-setting-description.bbb-recipe-parts"], "\ndefault: 2 steel-plate"<text tail>]}"#,
        ],
    );
}

/// The refusals `order_after` brings, and the ones that say WHICH refusal a
/// plan carrying more than one mistake gets.
///
/// ALL OF THEM ARE THE SETTINGS STAGE'S. `validate_settings` is run by
/// `plan_settings` and by nothing else: the data stage reads a setting's
/// VALUE and never writes its prototype, so an order it cannot see is not its
/// to refuse. The block after the cases holds that up to the light.
#[test]
fn order_after_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
    }

    const EMPTY_ORDER: &str = "fkrecipes: OrderAfter was given an empty order; name the order string the generated settings should follow";

    let cases = [
        Case {
            // NOTHING IS DECLARED AT ALL, and it is refused anyway: a consumer
            // who named no order meant to place something. A check inside the
            // per-setting loop has not one setting to reach here.
            name: "an empty order with nothing declared at all",
            build: |l| {
                l.order_after("");
            },
            want: EMPTY_ORDER,
        },
        Case {
            name: "an empty order with a setting declared after it",
            build: |l| {
                l.legacy_bool_setting("bbb-enabled", true, "a");
                l.order_after("");
                l.bool_setting("hardened-tools", true);
            },
            want: EMPTY_ORDER,
        },
        Case {
            // A LATER, VALID CALL DOES NOT CLEAR IT. The empty call was a
            // mistake where it was written, and the settings placed under
            // some other order afterwards do not make it one the consumer
            // meant; the flag is sticky and the plan says so.
            name: "an empty order a later call moves on from",
            build: |l| {
                l.order_after("");
                l.order_after("a");
                l.bool_setting("hardened-tools", true);
            },
            want: EMPTY_ORDER,
        },
        Case {
            // The placed setting lands exactly where the consumer aimed it,
            // and a legacy setting is already there.
            name: "a placed order landing on a legacy one",
            build: |l| {
                pilot_order_plan(l);
                l.legacy_bool_setting("bbb-multi-edge-parts", false, "aab");
            },
            want: "fkrecipes: the setting recipe-ingredients would carry the order aab, which the legacy setting bbb-multi-edge-parts already carries; give one of them an order of its own",
        },
        Case {
            // NO CALL AT ALL, and the tie is likelier here than under one:
            // "aa" is what the first generated setting carries, and a
            // migrating mod that hand-wrote its first order wrote "a" or
            // "aa". A plan like this one LOADED before this check existed.
            name: "a generated order landing on a legacy one with no call",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.legacy_bool_setting("bbb-multi-edge-parts", false, "aa");
            },
            want: "fkrecipes: the setting hardened-tools would carry the order aa, which the legacy setting bbb-multi-edge-parts already carries; give one of them an order of its own",
        },
        Case {
            // THE PLACEMENT A TIE CHECK MISSES. Twenty-four settings fit
            // between "a" and a legacy "ab" ("aac" through "aaz"); the
            // twenty-fifth rolls the two letters over to "ba" and lands at
            // "aba", which sorts past "ab" rather than under "a". The
            // acceptance test above pins the other side of the same line.
            name: "a placed order that walks past a legacy order extending the named one",
            build: |l| {
                l.legacy_bool_setting("bbb-cost", false, "a");
                l.legacy_bool_setting("bbb-cost-detail", false, "ab");
                l.order_after("a");
                for i in 0..25 {
                    l.bool_setting(&format!("hardened-tools-{}", i), true);
                }
            },
            want: "fkrecipes: the setting hardened-tools-24 would carry the order aba and sort past the legacy setting bbb-cost-detail at ab, which extends a; OrderAfter(a) places settings before every legacy order that extends a",
        },
        Case {
            // THE VERY FIRST PLACED SETTING, past it already: a legacy "ba"
            // sits directly under "b", and nothing placed behind "b" can sort
            // before it. There is no count to reach here, so the plan is
            // wrong from its first declaration.
            name: "a placed order past a legacy order that sits directly under the named one",
            build: |l| {
                l.legacy_bool_setting("bbb-cost", false, "b");
                l.legacy_bool_setting("bbb-cost-detail", false, "ba");
                l.order_after("b");
                l.bool_setting("hardened-tools", true);
            },
            want: "fkrecipes: the setting hardened-tools would carry the order bac and sort past the legacy setting bbb-cost-detail at ba, which extends b; OrderAfter(b) places settings before every legacy order that extends b",
        },
        Case {
            // THE FIRST ORDERING WITNESS. The empty order is refused before
            // the per-setting loop, so it wins over a tie the order scan
            // would find afterwards.
            name: "an empty order beside a tie",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.legacy_bool_setting("bbb-multi-edge-parts", false, "aa");
                l.order_after("");
            },
            want: EMPTY_ORDER,
        },
        Case {
            // THE SECOND ORDERING WITNESS, on one setting: the second
            // declaration both shares a name with the first and lands on the
            // legacy order, and the name is the sentence. A shared name is
            // silent last-writer-wins, so the plan the consumer gets back is
            // missing a setting entirely; where it would have sorted is the
            // smaller problem.
            name: "a tie beside a duplicate name",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.bool_setting("hardened-tools", false);
                l.legacy_bool_setting("bbb-multi-edge-parts", false, "ab");
            },
            want: "fkrecipes: two settings share the name steelworks-hardened-tools; the engine keeps the last one silently",
        },
        Case {
            // THE THIRD ORDERING WITNESS, and the one the scan's own shape
            // rests on: the tie is at the FIRST setting and the empty name is
            // at the second, so a scan running inside the per-setting loop
            // answers a plan whose real mistake it has not reached, quoting a
            // setting with no name in its own sentence.
            name: "a tie beside a later setting with an empty name",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.legacy_bool_setting("", false, "aa");
            },
            want: "fkrecipes: a setting was declared with an empty name",
        },
        Case {
            // The same shape with the legacy order left empty, which is the
            // other declaration the loop refuses on a setting's own terms.
            name: "a tie beside a later legacy setting with an empty order",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.legacy_bool_setting("bbb-multi-edge-parts", false, "aa");
                l.legacy_bool_setting("bbb-detail", false, "");
            },
            want: "fkrecipes: the legacy setting bbb-detail was declared with an empty order",
        },
        Case {
            // And with a default the engine would refuse at load: a plan the
            // player cannot start is answered before a placement they can
            // still see.
            name: "a tie beside a later dropdown's default",
            build: |l| {
                l.bool_setting("hardened-tools", true);
                l.legacy_bool_setting("bbb-multi-edge-parts", false, "aa");
                l.dropdown_setting_needing_locale("finish", "gilded", &["plain", "fancy"]);
            },
            want: "fkrecipes: the dropdown setting finish defaults to gilded, which is not one of its allowed values",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        (c.build)(&mut lib);
        // A REFUSED PLAN CARRIES NO OPS, which this half gets from the type
        // rather than from an assertion: the error arm of a Result holds no
        // Vec for one to hide in. The Go mirror checks the returned slice.
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

    // THE STAGE SPLIT, AS A FACT RATHER THAN AS THE COMMENT ABOVE. The data
    // stage runs the binding and text-setting validators and never this one,
    // so a plan the settings stage refuses for its orders is one it accepts.
    let mut lib = Lib::new();
    lib.order_after("");
    lib.bool_setting("hardened-tools", true);
    lib.plan_data(&base_world())
        .expect("the data stage refused a settings-stage mistake");
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
    tier_plan_priced(choices, vec![Pack::new("automation-science-pack", 1)])
}

/// The same plan with the fallback's price named, for the tests that are
/// about the fallback's own packs.
fn tier_plan_priced(choices: Vec<CostChoice>, packs: Vec<Pack>) -> Lib {
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
                    packs,
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

/// A PRESENT-BUT-UNUSABLE UNIT IS STEPPED PAST, which witnesses the
/// `Some(non-map)` half of the ladder's fall-through arm independently of the
/// `None` half that `cost_by_steps_past_a_source_that_is_not_there` covers.
///
/// A present nil is what `from_v` produces for a unit whose table carried a
/// key this library drops, so it is a real answer rather than an invented one.
///
/// THE GO MIRROR CARRIES ONE TEST THIS FILE CANNOT, and the type is why. Go's
/// `TechUnit` returns `(Value, bool)`, a pair that can disagree with itself, so
/// a World there can hand back a real map beside ok=false and the flag is a
/// separate guard needing its own witness. `tech_unit` returns
/// `Option<Value>`: a value cannot ride along with absence, that case is
/// unrepresentable, and the single fall-through arm below is the whole guard.
#[test]
fn cost_by_steps_past_a_present_but_unusable_unit() {
    let choices = vec![
        cost_choice("logistics", &["steel-processing", "logistics-2"]),
        cost_choice("military", &["logistics-2"]),
    ];
    let w = base_world().with_nil_unit("steel-processing");
    let ops = tier_plan(choices).plan_data(&w).expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
        &alloc::format!(
            r#"extend {{type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit={}}}"#,
            LOGISTICS_2_UNIT
        ),
    ]);
}

/// AN ABSENT SOURCE IS STEPPED PAST, which is the ladder's whole reason for
/// being a list: a mod prices its research from whichever of several
/// technologies the player's install actually has.
///
/// There is no presence probe in the walk. A technology the game does not have
/// carries no unit either, so the "carries no usable unit" arm steps past an
/// absent rung by the same test it steps past a unit-less one, and this test
/// is what says that arm really does cover absence. It goes red if that arm is
/// broken, which is what makes the deleted tech_exists check unnecessary
/// rather than merely redundant.
#[test]
fn cost_by_steps_past_a_source_that_is_not_there() {
    let choices = vec![
        // Two rungs no vanilla install has, then one it does.
        cost_choice(
            "logistics",
            &["quarry-drills", "logistics-4", "steel-processing"],
        ),
        cost_choice("military", &["steel-processing"]),
    ];
    let ops = tier_plan(choices)
        .plan_data(&base_world())
        .expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
        &alloc::format!(
            r#"extend {{type="technology", name="steelworks-hardened-tips", prerequisites=["steel-processing"], unit={}}}"#,
            STEEL_PROCESSING_UNIT
        ),
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

/// A FALLBACK NOBODY REACHES ASKS THE GAME NOTHING. The chosen ladder settles
/// on a real source, so the fallback's own packs are never walked: no rung is
/// probed, no drop is logged, and the load is not refused over a price that
/// could never apply.
///
/// This is the pilot's finding turned into a test. The fallback here is
/// priced entirely in packs no vanilla install has, which under the old order
/// refused the load of a plan whose cost came from somewhere else entirely.
#[test]
fn an_unreached_fallback_is_never_resolved() {
    let choices = vec![
        cost_choice("logistics", &["logistics-2"]),
        cost_choice("military", &["steel-processing"]),
    ];
    let lib = tier_plan_priced(
        choices,
        vec![Pack::named(
            1,
            "military-science-pack",
            &["space-science-pack"],
        )],
    );

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(&transcript(&ops), &[
        "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
        &alloc::format!(
            r#"extend {{type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit={}}}"#,
            LOGISTICS_2_UNIT
        ),
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
            "fkrecipes: a prerequisite cycle: logistics-2 -> steelworks-hardened-tips -> logistics-2"
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
            want: "fkrecipes: the recipe hardened-steel-plate names both Ingredients and IngredientsBy; pick one",
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
            want: "fkrecipes: the recipe hardened-steel-plate names an ingredients setting that this plan never declared",
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
            want: "fkrecipes: the recipe hardened-steel-plate offers something for brine where the setting steelworks-quench-medium allows oil",
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
            want: "fkrecipes: the recipe hardened-steel-plate offers nothing for the value oil that the setting steelworks-quench-medium allows",
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
            want: "fkrecipes: the recipe hardened-steel-plate offers something for oil, which the setting steelworks-quench-medium does not allow",
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
            want: "fkrecipes: the recipe hardened-steel-plate names an ingredient item that this plan never declared",
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
            want: "fkrecipes: the technology hardened-tips names CostBy with a placement; the prerequisite moves with the unit, so CostBy places the technology itself",
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
            want: "fkrecipes: the technology hardened-tips names a cost setting that this plan never declared",
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
            want: "fkrecipes: the technology hardened-tips offers something for military where the setting steelworks-tips-research-tier allows logistics",
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
            want: "fkrecipes: the technology hardened-tips has a unit count below 1, which the engine refuses",
        },
        Case {
            // The fallback IS reached here (the chosen ladder is empty), so
            // its pack ladder is walked, the one rung drops, and the unit is
            // left with nothing to price the research in. A fallback nobody
            // reaches asks the game nothing at all, which is what
            // an_unreached_fallback_is_never_resolved holds.
            name: "a reached fallback whose only pack the game does not have",
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
                                packs: vec![Pack::new("military-science-pack", 1)],
                            },
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips has no science pack the game has; research takes at least one",
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
