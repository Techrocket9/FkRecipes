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
use crate::tests::{
    assert_composed, assert_has_line, assert_lines, base_world, description_ref_in, packless_log,
    settings_world, transcript, unit_of, unreadable_source_log, PACKLESS_TOOLTIP, UNPRICED_TOOLTIP,
};
use crate::value::{kv, Value};

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
            r#"extend {type="string-setting", name="steelworks-recipe-ingredients", setting_type="startup", default_value="default", order="aab", auto_trim=true, localised_description=["", ["?", ["mod-setting-description.steelworks-recipe-ingredients"], "steelworks-recipe-ingredients"], "\ndefault: 2 steel-plate"<text tail>]}"#,
            r#"extend {type="bool-setting", name="steelworks-recipe-hint", setting_type="startup", default_value=true, order="aac"}"#,
            r#"extend {type="string-setting", name="bbb-tech-cost", setting_type="startup", default_value="logistics", order="b", allowed_values=["logistics", "custom"]}"#,
            r#"extend {type="string-setting", name="steelworks-tech-packs", setting_type="startup", default_value="default", order="bae", auto_trim=true, localised_description=["", ["?", ["mod-setting-description.steelworks-tech-packs"], "steelworks-tech-packs"], "\ndefault: 1 automation-science-pack"<packs tail>]}"#,
            r#"extend {type="int-setting", name="steelworks-tech-count", setting_type="startup", default_value=20, order="baf", minimum_value=1, maximum_value=1000000, localised_description=["", ["?", ["mod-setting-description.steelworks-tech-count"], "steelworks-tech-count"], "\nA whole number from 1 to 1000000."]}"#,
            r#"extend {type="int-setting", name="steelworks-tech-seconds", setting_type="startup", default_value=15, order="bag", minimum_value=1, maximum_value=3600, localised_description=["", ["?", ["mod-setting-description.steelworks-tech-seconds"], "steelworks-tech-seconds"], "\nA whole number from 1 to 3600."]}"#,
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
            r#"extend {type="string-setting", name="bbb-recipe-parts", setting_type="startup", default_value="default", order="c", auto_trim=true, localised_description=["", ["?", ["mod-setting-description.bbb-recipe-parts"], "bbb-recipe-parts"], "\ndefault: 2 steel-plate"<text tail>]}"#,
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
/// applies and the technology hangs off nothing, and the technology's own
/// description says so: a prerequisite that is gone is presence a player cannot
/// check anywhere. See `unpriced_source_note`.
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
        &alloc::format!(
            r#"extend {{type="technology", name="steelworks-hardened-tips", localised_description=["", {}, "{}"], unit={{count=60, time=30, ingredients=[["automation-science-pack", 1]]}}}}"#,
            description_ref_in("technology", "steelworks-hardened-tips"),
            UNPRICED_TOOLTIP
        ),
    ]);
}

/// A FALLBACK NOBODY REACHES ASKS THE GAME NOTHING. The chosen ladder settles
/// on a source that carries a copyable unit AND keeps at least one pack
/// through the tool probe, so the fallback's own packs are never walked: no
/// rung is probed, no drop is logged, and the load is not refused over a price
/// that could never apply.
///
/// BOTH HALVES OF THAT CONDITION MATTER and the second one is easy to lose. A
/// source answering is not on its own the end of the pack question: a copied
/// unit whose every pack the game lacks takes the packless-source arm and
/// reaches the fallback after all, which is exactly what a fixture with no
/// tool in it produces. This one keeps logistics-2's own pack in the game.
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

/// The edge the ladder chose joins the cycle overlay like any other, and it is
/// an edge THIS PLAN MADE, so the ring is resolved by dropping it rather than
/// refused. The technology is still emitted, priced on the source it chose, and
/// it hangs off nothing.
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

    let ops = lib.plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
            "log fkrecipes: ERROR: hardened-tips: requiring logistics-2 would loop this game's technology tree (logistics-2 -> steelworks-hardened-tips -> logistics-2), so the prerequisite is dropped",
            &(String::from(r#"extend {type="technology", name="steelworks-hardened-tips", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-hardened-tips")
                + r#", "Requiring logistics-2 would loop this game's technology tree, so this research was left without that prerequisite. The reason is in the log."], unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}"#),
        ],
    );
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
        // A REACHED FALLBACK WHOSE ONLY PACK THE GAME LACKS IS NOT HERE ANY
        // MORE. Its packs are still probed where the fallback is what applies,
        // and a cost with nothing left is EMITTED EMPTY with a line and a
        // tooltip rather than refused: see
        // `a_fallback_that_keeps_no_pack_is_emitted_empty` below.
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

/// A TIER'S CHOSEN SOURCE IS COPIED THE SAME WAY, and the drop line names the
/// source the tier landed on rather than a `cost_of` that is not there.
#[test]
fn a_tier_whose_copied_unit_drops_one_pack_keeps_the_rest() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "mid", &["mid"]);
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("mid", &["logistics-2"])],
                fallback: UnitSpec {
                    count: 1,
                    seconds: 1.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
                },
            }),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(&base_world().without_tool("logistic-science-pack"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: logistic-science-pack is not a science pack this game has, so it is left out of the logistics-2 cost",
            &(String::from(r#"extend {type="technology", name="steelworks-steel-axes", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-steel-axes")
                + r#", "This game has no logistic-science-pack, so this research was priced without it. The reason is in the log."], prerequisites=["logistics-2"], unit={count=200, ingredients=[["automation-science-pack", 1]], time=30}}"#),
        ],
    );
}

/// AND A TIER THAT LOSES EVERY PACK DEGRADES INSTEAD OF REFUSING, because there
/// IS a declared cost behind it: the author's own `fallback` unit, resolved
/// through the same ladder so its own absent rungs drop the same way.
///
/// THE ERROR LINE IS NOT A PLAYER'S FALLBACK, and it carries no route to the
/// settings screen: nothing was typed, so there is no field to send anybody to.
///
/// THE PREREQUISITE AND THE LEVEL CAP STAY. The tier still chose this rung;
/// only the price moved.
#[test]
fn a_tier_whose_copied_unit_loses_every_pack_takes_the_declared_fallback() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("early", &["steel-processing"])],
                fallback: UnitSpec {
                    count: 7,
                    seconds: 8.0,
                    packs: vec![Pack::new("chemical-science-pack", 2)],
                },
            }),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(&base_world().without_tool("automation-science-pack"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost",
            "log fkrecipes: ERROR: steel-axes: the steel-processing cost names no science pack this game has, so this mod's own declared cost applies instead",
            &(String::from(r#"extend {type="technology", name="steelworks-steel-axes", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-steel-axes")
                + r#", "This game has none of the science packs the steel-processing cost names, so that cost was not used to price this research. The reason is in the log."], prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["chemical-science-pack", 2]]}}"#),
        ],
    );
}

/// AND A TIER WHOSE SOURCES CARRY NO COST AT ALL SAYS SO ON THE TECHNOLOGY,
/// which is the only degradation of this set that changes WHERE IN THE TREE a
/// technology sits: the two arms above move a PRICE, and this one also takes
/// the PREREQUISITE away, so the research sits at the root of the technology
/// tree and is researchable from the first minute.
///
/// THAT IS NOT A CLAIM THAT IT OUTRANKS EVERY OTHER SENTENCE, and the test
/// below is where the ordering is actually settled: a research left with no
/// science pack at all completes for free, which is the more urgent thing to
/// say, so that sentence takes the slot from this one where both are true.
///
/// THE FIXTURE IS THE CONSUMER'S OWN, reduced: a pack that renames the base
/// technology the chosen tier names, so every rung of the chosen ladder is
/// absent and not one of them carries a unit. The declared fallback KEEPS a
/// pack here, which is what leaves this sentence the only one competing for the
/// slot; the test below is the other half of that.
///
/// THE WHOLE `localised_description` IS ASSERTED, because the note is the thing
/// under test and a substring match would pass on a prototype carrying somebody
/// else's sentence.
#[test]
fn a_tier_whose_sources_carry_no_cost_says_so_on_the_technology() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "logistics", &["logistics"]);
    lib.technology(
        "balancer",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("logistics", &["logistics", "logistics-2"])],
                fallback: UnitSpec {
                    count: 20,
                    seconds: 15.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
                },
            }),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(
            &base_world()
                .without_tech("logistics")
                .without_tech("logistics-2"),
        )
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: balancer: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
            &format!(
                r#"extend {{type="technology", name="steelworks-balancer", localised_description=["", {}, "{}"], unit={{count=20, time=15, ingredients=[["automation-science-pack", 1]]}}}}"#,
                description_ref_in("technology", "steelworks-balancer"),
                UNPRICED_TOOLTIP
            ),
        ],
    );
}

/// AND THE SAME SENTENCE ON THE OTHER WAY INTO THAT ARM, which is the path the
/// sentence was FALSE on until this round.
///
/// THE SOURCE HERE IS PRESENT AND ITS COST IS UNUSABLE, rather than absent: the
/// technology is in this game, `tech_unit` answers `Some`, and the value is not
/// a dictionary (a present nil, which is what a lossy read leaves behind for a
/// unit whose table carried a key this library drops). The ladder steps past it
/// on the SHAPE term rather than on absence, lands on no source at all, and the
/// same arm runs. The first draft of this note said "carries a cost in this
/// game", which is plainly false here, and the test that was meant to guard it
/// only ever exercised the absent path: an absent source makes the false clause
/// true by accident, so nothing went red. "A cost this mod can use here" is
/// what is true of both.
///
/// THE WHOLE `localised_description` IS ASSERTED, for the reason the test above
/// asserts it: the note is the thing under test.
#[test]
fn a_tier_whose_only_source_carries_an_unusable_cost_says_so_too() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "logistics", &["logistics"]);
    lib.technology(
        "balancer",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("logistics", &["steel-processing"])],
                fallback: UnitSpec {
                    count: 20,
                    seconds: 15.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
                },
            }),
            ..Default::default()
        },
    );

    // steel-processing IS in this world; only its unit is a shape the copy
    // cannot use.
    let ops = lib
        .plan_data(&base_world().with_nil_unit("steel-processing"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: balancer: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
            &format!(
                r#"extend {{type="technology", name="steelworks-balancer", localised_description=["", {}, "{}"], unit={{count=20, time=15, ingredients=[["automation-science-pack", 1]]}}}}"#,
                description_ref_in("technology", "steelworks-balancer"),
                UNPRICED_TOOLTIP
            ),
        ],
    );
}

/// AND THE NOTE IS OFFERED AFTER THE FALLBACK IS RESOLVED, SO A WORSE OUTCOME
/// WINS THE SLOT. The same plan with a declared fallback this game cannot pay
/// either leaves the technology with NO SCIENCE PACK, which is a research that
/// completes for free: that is what the tooltip says, and the missing
/// prerequisite stays in the log line where an author reads it.
///
/// WITHOUT THIS THE ORDERING IS A COMMENT NOBODY CHECKS. `note_on` keeps the
/// FIRST note per prototype, so recording this arm's sentence before the
/// fallback is resolved would leave a technology that costs nothing at all
/// saying only that it has no prerequisite, which is the less urgent half of
/// what happened to it.
#[test]
fn an_unpriced_tier_yields_the_slot_to_the_packless_note() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "logistics", &["logistics"]);
    lib.technology(
        "balancer",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("logistics", &["logistics", "logistics-2"])],
                fallback: UnitSpec {
                    count: 20,
                    seconds: 15.0,
                    packs: vec![Pack::new("military-science-pack", 1)],
                },
            }),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(
            &base_world()
                .without_tech("logistics")
                .without_tech("logistics-2"),
        )
        .expect("plan refused");

    let lines = transcript(&ops);
    assert_lines(
        &lines,
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: balancer: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
            "log fkrecipes: balancer: none of military-science-pack is present, so the science pack is dropped",
            &packless_log("balancer", &["military-science-pack"]),
            &format!(
                r#"extend {{type="technology", name="steelworks-balancer", localised_description=["", {}, "{}"], unit={{count=20, time=15, ingredients=[]}}}}"#,
                description_ref_in("technology", "steelworks-balancer"),
                PACKLESS_TOOLTIP
            ),
        ],
    );

    for line in &lines {
        assert!(
            !line.contains(UNPRICED_TOOLTIP),
            "the unpriced-tier note took a slot the packless note had to have: {}",
            line
        );
    }
}

/// AND WHEN THE PLAYER HAS TYPED A PACK LIST, THE TIER'S SENTENCE IS TAKEN
/// BACK, because it is not true any more.
///
/// THE SHAPE IS THE EXAMPLE GUEST'S OWN: a technology declaring `cost_by` and
/// `cost_from` together, on a mod set where the tier's source loses every pack.
/// The tier arm writes the ERROR line and the tooltip note BEFORE the custom
/// cost is read, so without the retraction the technology says the copied cost
/// did not price it while the packs, and the count and the seconds beside them,
/// are the player's. A false statement in a tooltip is worse than none.
///
/// THE DROP LINE STAYS, because it is still true: that pack really is absent.
#[test]
fn a_typed_pack_list_takes_back_the_tiers_packless_sentence() {
    let plan = || {
        let mut lib = Lib::new();
        let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
        let packs = lib.packs_setting("axe-packs", vec![Pack::new("chemical-science-pack", 2)]);
        let count = lib.int_setting("axe-count", 0, NumericSpec::between(0.0, 1000.0));
        let seconds = lib.int_setting("axe-seconds", 0, NumericSpec::between(0.0, 600.0));
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: vec![cost_choice("early", &["steel-processing"])],
                    fallback: UnitSpec {
                        count: 7,
                        seconds: 8.0,
                        packs: vec![Pack::new("chemical-science-pack", 2)],
                    },
                }),
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );
        lib
    };

    let typed = base_world()
        .without_tool("automation-science-pack")
        .with_setting(
            "steelworks-axe-packs",
            Value::string("3 logistic-science-pack"),
        );
    let ops = plan().plan_data(&typed).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost",
            "log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs: count 7, time 8, packs 3 logistic-science-pack; the steelworks-tier choice early supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["logistic-science-pack", 3]]}}"#,
        ],
    );

    // AND THE SAME PLAN WITH THE FIELD LEFT ALONE KEEPS BOTH, which is what
    // scopes the retraction to the case that made them false: here the mod's
    // own declared cost really is what applies.
    let ops = plan()
        .plan_data(&base_world().without_tool("automation-science-pack"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost",
            "log fkrecipes: ERROR: steel-axes: the steel-processing cost names no science pack this game has, so this mod's own declared cost applies instead",
            "log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-packs was not readable, so its default applies",
            &(String::from(r#"extend {type="technology", name="steelworks-steel-axes", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-steel-axes")
                + r#", "This game has none of the science packs the steel-processing cost names, so that cost was not used to price this research. The reason is in the log."], prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["chemical-science-pack", 2]]}}"#),
        ],
    );
}

/// AND THE SAME RETRACTION FOR THE UNPRICED-TIER SENTENCE, which the test above
/// does not reach: it exercises the packless-SOURCE arm, where a source
/// answered and then lost every pack, and this is the arm where no source
/// answered at all.
///
/// ONE `restore_note` CALL IS WHAT IS BEING WITNESSED. The tier arm's whole
/// note slot goes back to what it held before that arm ran, whichever of its
/// sentences was in it, and the claim that this sentence rides on the same
/// mechanism was an assertion in the notes with no test under it.
///
/// THE LOG LINE IS NOT RETRACTED HERE, and that is the shape rather than an
/// oversight in this test: the unpriced arm's line has no named retraction
/// beside `packless_source_line` and `unreadable_source_line`, so it stays. It
/// is author-facing and its second clause, that the technology has no
/// prerequisite, is still true of the emitted prototype.
#[test]
fn a_typed_pack_list_takes_back_the_unpriced_tier_sentence() {
    let plan = || {
        let mut lib = Lib::new();
        let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
        let packs = lib.packs_setting("axe-packs", vec![Pack::new("chemical-science-pack", 2)]);
        let count = lib.int_setting("axe-count", 0, NumericSpec::between(0.0, 1000.0));
        let seconds = lib.int_setting("axe-seconds", 0, NumericSpec::between(0.0, 600.0));
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: vec![cost_choice("early", &["steel-processing"])],
                    fallback: UnitSpec {
                        count: 7,
                        seconds: 8.0,
                        packs: vec![Pack::new("chemical-science-pack", 2)],
                    },
                }),
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );
        lib
    };

    // The tier's only source is gone, so no source answers and the arm under
    // test runs; then the player types a pack list over it.
    let typed = base_world().without_tech("steel-processing").with_setting(
        "steelworks-axe-packs",
        Value::string("3 logistic-science-pack"),
    );
    let ops = plan().plan_data(&typed).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: no source for the early cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
            "log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs: count 7, time 8, packs 3 logistic-science-pack; the steelworks-tier choice early supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-steel-axes", unit={count=7, time=8, ingredients=[["logistic-science-pack", 3]]}}"#,
        ],
    );

    // AND THE SAME PLAN WITH THE FIELD LEFT ALONE KEEPS IT, which is what
    // scopes the retraction to the case that made it false.
    let ops = plan()
        .plan_data(&base_world().without_tech("steel-processing"))
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: no source for the early cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
            "log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-packs was not readable, so its default applies",
            &format!(
                r#"extend {{type="technology", name="steelworks-steel-axes", localised_description=["", {}, "{}"], unit={{count=7, time=8, ingredients=[["chemical-science-pack", 2]]}}}}"#,
                description_ref_in("technology", "steelworks-steel-axes"),
                UNPRICED_TOOLTIP
            ),
        ],
    );
}

/// AND THE FALLBACK THAT APPLIES AND KEEPS NO PACK IS THE DEGRADATION IT USED
/// TO BE A REFUSAL FOR. The chosen value names no source at all, so the
/// fallback IS what applies: its packs are probed, the only one drops, and what
/// is left is emitted with an empty ingredient list and a tooltip saying so. A
/// fallback nobody reaches is a different test, above.
#[test]
fn a_fallback_that_keeps_no_pack_is_emitted_empty() {
    let mut lib = Lib::new();
    let tier =
        lib.dropdown_setting_needing_locale("tips-research-tier", "logistics", &["logistics"]);
    lib.technology(
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

    let ops = lib.plan_data(&base_world()).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
            "log fkrecipes: hardened-tips: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
            "log fkrecipes: hardened-tips: none of military-science-pack is present, so the science pack is dropped",
            &packless_log("hardened-tips", &["military-science-pack"]),
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-hardened-tips", localised_description=["", {}, "{}"], unit={{count=60, time=30, ingredients=[]}}}}"#,
                description_ref_in("technology", "steelworks-hardened-tips"),
                PACKLESS_TOOLTIP
            ),
        ],
    );
}

/// A TIER WHOSE CHOSEN SOURCE CARRIES A PACK LIST THIS LIBRARY CANNOT READ
/// DEGRADES ONTO THE AUTHOR'S OWN DECLARED COST, which is the same answer the
/// arm beside it gives for a source that lost every pack, in its own words:
/// nothing was DROPPED here, because nothing was read, so a sentence saying
/// this game has none of those packs would be stating something the library
/// does not know.
///
/// THE PREREQUISITE AND THE LEVEL CAP STAY, because the tier still chose that
/// rung; only the price moved. That is the one thing this differs in from the
/// `cost_of` case, where there is nothing declared behind the copy at all.
#[test]
fn a_tier_whose_source_pack_list_is_unreadable_falls_back_to_the_declared_cost() {
    let odd = Value::Map(alloc::vec![
        kv("count", Value::Num(10.0)),
        kv(
            "ingredients",
            Value::Arr(alloc::vec![Value::string("automation-science-pack")])
        ),
        kv("time", Value::Num(15.0)),
    ]);

    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("early", &["steel-processing"])],
                fallback: UnitSpec {
                    count: 7,
                    seconds: 8.0,
                    packs: vec![Pack::new("chemical-science-pack", 2)],
                },
            }),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(&base_world().with_unit("steel-processing", odd))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            &unreadable_source_log("steel-axes", "steel-processing"),
            &(String::from(
                r#"extend {type="technology", name="steelworks-steel-axes", localised_description=["", "#,
            ) + &description_ref_in("technology", "steelworks-steel-axes")
                + r#", "The steel-processing cost this research copies cannot be read in this game, so that cost was not used to price this research. The reason is in the log."], prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["chemical-science-pack", 2]]}}"#),
        ],
    );
}

/// AND WHEN THE PLAYER HAS TYPED A PACK LIST, THE PACKLESS PAIR IS TAKEN BACK
/// TOO, which is the half the tier's own retraction does not cover: here BOTH
/// ladders missed, so the tier arm wrote its sentence and the declared fallback
/// then went packless on top of it, and the technology's tooltip said the
/// research completes for free. The player's own list is what it is priced in,
/// so neither line nor either tooltip is true any more.
#[test]
fn a_typed_pack_list_takes_back_the_packless_pair_as_well() {
    let plan = || {
        let mut lib = Lib::new();
        let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
        let packs = lib.packs_setting("axe-packs", vec![Pack::new("chemical-science-pack", 2)]);
        let count = lib.int_setting("axe-count", 0, NumericSpec::between(0.0, 1000.0));
        let seconds = lib.int_setting("axe-seconds", 0, NumericSpec::between(0.0, 600.0));
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: vec![cost_choice("early", &["steel-processing"])],
                    fallback: UnitSpec {
                        count: 7,
                        seconds: 8.0,
                        packs: vec![Pack::new("military-science-pack", 2)],
                    },
                }),
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );
        lib
    };

    let typed = base_world()
        .without_tool("automation-science-pack")
        .with_setting(
            "steelworks-axe-packs",
            Value::string("3 logistic-science-pack"),
        );
    let ops = plan().plan_data(&typed).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost",
            "log fkrecipes: steel-axes: none of military-science-pack is present, so the science pack is dropped",
            "log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs: count 7, time 8, packs 3 logistic-science-pack; the steelworks-tier choice early supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["logistic-science-pack", 3]]}}"#,
        ],
    );

    // AND THE SAME PLAN WITH THE FIELD LEFT ALONE KEEPS BOTH, which is what
    // scopes the retraction to the case that made them false.
    let ops = plan()
        .plan_data(&base_world().without_tool("automation-science-pack"))
        .expect("plan refused");
    assert_has_line(
        &transcript(&ops),
        &packless_log(
            "steel-axes",
            &["automation-science-pack", "military-science-pack"],
        ),
    );
    assert_has_line(
        &transcript(&ops),
        &alloc::format!(
            r#"extend {{type="technology", name="steelworks-steel-axes", localised_description=["", {}, "{}"], prerequisites=["steel-processing"], unit={{count=7, time=8, ingredients=[]}}}}"#,
            description_ref_in("technology", "steelworks-steel-axes"),
            PACKLESS_TOOLTIP
        ),
    );
}

/// AND THE RETRACTION IS A SNAPSHOT AND NOT A LIST OF NAMES, which is what
/// covers the sentence the retracting site cannot compose.
///
/// THE HAZARD, WHICH IS REACHABLE. The tier's declared fallback is resolved
/// BEFORE the tier's own sentence is offered to the note slot, and `note_on`
/// keeps the FIRST writer: two of that fallback's own pack ladders landing on
/// one name over the item ceiling leave a CLAMP note in the slot and shut the
/// tier's sentence out. A retraction that named `packless_source_note` would
/// then match nothing, and the technology would carry "the total was above what
/// one slot holds, so it was capped" over an emitted price that is the player's
/// own list with nothing in it clamped.
#[test]
fn a_typed_pack_list_takes_back_a_clamp_the_tier_arm_left_behind() {
    let plan = || {
        let mut lib = Lib::new();
        let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
        let packs = lib.packs_setting("axe-packs", vec![Pack::new("chemical-science-pack", 2)]);
        let count = lib.int_setting("axe-count", 0, NumericSpec::between(0.0, 1000.0));
        let seconds = lib.int_setting("axe-seconds", 0, NumericSpec::between(0.0, 600.0));
        lib.technology(
            "steel-axes",
            TechSpec {
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: vec![cost_choice("early", &["steel-processing"])],
                    fallback: UnitSpec {
                        count: 7,
                        seconds: 8.0,
                        packs: vec![
                            Pack::new("chemical-science-pack", 60000),
                            Pack::named(60000, "military-science-pack", &["chemical-science-pack"]),
                        ],
                    },
                }),
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );
        lib
    };

    let typed = base_world()
        .without_tool("automation-science-pack")
        .with_setting(
            "steelworks-axe-packs",
            Value::string("3 logistic-science-pack"),
        );
    let ops = plan().plan_data(&typed).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: steel-axes: automation-science-pack is not a science pack this game has, so it is left out of the steel-processing cost",
            "log fkrecipes: steel-axes: chemical-science-pack is in the list twice after the fallbacks, so the amounts are added: 60000 plus 60000 is 120000",
            "log fkrecipes: steel-axes: chemical-science-pack is in the list twice after the fallbacks, and 60000 plus 60000 is above the item ceiling of 65535, so it is capped there",
            "log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs: count 7, time 8, packs 3 logistic-science-pack; the steelworks-tier choice early supplies what the settings leave at default",
            // NO localised_description AT ALL, which is the assertion: the slot
            // is back to what it held before the tier was priced, and nothing
            // was priced at a ceiling in the list this technology emits.
            r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["logistic-science-pack", 3]]}}"#,
        ],
    );
    // THE DROP LINES STAY, because they are true whatever the price ended up
    // being, and so does the clamp line: what those two say is what the walk
    // asked the game and what it did with the fallback it built.

    // AND THE SAME PLAN WITH THE FIELD LEFT ALONE KEEPS THE CLAMP, which is
    // what scopes the restore to the case that made it false: that technology
    // really is priced at the ceiling.
    let ops = plan()
        .plan_data(&base_world().without_tool("automation-science-pack"))
        .expect("plan refused");
    assert_has_line(
        &transcript(&ops),
        &(String::from(
            r#"extend {type="technology", name="steelworks-steel-axes", localised_description=["", "#,
        ) + &description_ref_in("technology", "steelworks-steel-axes")
            + r#", "Two ingredients resolved onto chemical-science-pack and the total was above what one slot holds, so it was capped at 65535. The reason is in the log."], prerequisites=["steel-processing"], unit={count=7, time=8, ingredients=[["chemical-science-pack", 65535]]}}"#),
    );
}

/// AND THE RETRACTION IS KEYED ON THE DECLARATION AND NOT ON THE NAME, because
/// a DECLARED name is not unique. validate refuses two technologies whose
/// EMITTED names collide, and a legacy declaration keeps its name unprefixed, so
/// a legacy "steel-axes" and an ordinary one are a legal plan with one declared
/// name between them. A retraction that matched on the name would take back a
/// line that is still true and belongs to the other declaration.
///
/// THE SHAPE. The legacy technology goes packless on its own hand-rolled unit
/// and earns its line; the ordinary one shares the name, reaches a tier, and its
/// player-typed pack list runs the retraction. Nothing about the legacy one
/// changed, so its line must still be in the stream.
#[test]
fn the_packless_retraction_is_keyed_on_the_declaration_and_not_the_name() {
    let mut lib = Lib::new();
    lib.legacy_technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 5,
                seconds: 5.0,
                packs: vec![Pack::new("military-science-pack", 1)],
            }),
            ..Default::default()
        },
    );
    let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
    let packs = lib.packs_setting("axe-packs", vec![Pack::new("chemical-science-pack", 2)]);
    let count = lib.int_setting("axe-count", 0, NumericSpec::between(0.0, 1000.0));
    let seconds = lib.int_setting("axe-seconds", 0, NumericSpec::between(0.0, 600.0));
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("early", &["steel-processing"])],
                fallback: UnitSpec {
                    count: 7,
                    seconds: 8.0,
                    packs: vec![Pack::new("chemical-science-pack", 2)],
                },
            }),
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            ..Default::default()
        },
    );

    let w = base_world().with_setting(
        "steelworks-axe-packs",
        Value::string("3 logistic-science-pack"),
    );
    let ops = lib.plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steel-axes: none of military-science-pack is present, so the science pack is dropped",
            // THE LINE THE WHOLE TEST IS FOR. It belongs to the legacy
            // declaration, nothing about that declaration moved, and a
            // retraction keyed on the declared name takes it back from under it.
            &packless_log("steel-axes", &["military-science-pack"]),
            "log fkrecipes: the setting steelworks-tier was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs: count 50, time 15, packs 3 logistic-science-pack; the steelworks-tier choice early supplies what the settings leave at default",
            &alloc::format!(
                r#"extend {{type="technology", name="steel-axes", localised_description=["", {}, "{}"], unit={{count=5, time=5, ingredients=[]}}}}"#,
                description_ref_in("technology", "steel-axes"),
                PACKLESS_TOOLTIP
            ),
            r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["steel-processing"], unit={count=50, ingredients=[["logistic-science-pack", 3]], time=15}}"#,
        ],
    );
}

/// AND WHEN THE DECLARED FALLBACK IS ALSO UNPAYABLE THE LINE NAMES EVERY RUNG
/// the walk asked about: the copied pack first, then the fallback's own ladder,
/// in the order they were asked. The load is not stopped: the technology is
/// emitted with an empty unit and the tooltip says the research is free.
#[test]
fn a_tier_whose_fallback_is_also_unpayable_is_emitted_empty() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("early", &["steel-processing"])],
                fallback: UnitSpec {
                    count: 7,
                    seconds: 8.0,
                    packs: vec![Pack::named(
                        2,
                        "military-science-pack",
                        &["space-science-pack"],
                    )],
                },
            }),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(&base_world().without_tool("automation-science-pack"))
        .expect("plan refused");
    assert_has_line(
        &transcript(&ops),
        &packless_log(
            "steel-axes",
            &[
                "automation-science-pack",
                "military-science-pack",
                "space-science-pack",
            ],
        ),
    );
}

/// AND A NAME ASKED ABOUT TWICE IS NAMED ONCE. Two producers feed that list:
/// the chosen tier's copied unit, whose lost packs come first, and the declared
/// fallback's own ladders. Each deduped only itself, so a fallback rung that
/// repeats a copied pack used to print the name twice in one sentence.
#[test]
fn a_pack_asked_about_twice_is_named_once() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("early", &["steel-processing"])],
                fallback: UnitSpec {
                    count: 7,
                    seconds: 8.0,
                    packs: vec![Pack::named(2, "automation-science-pack", &[])],
                },
            }),
            ..Default::default()
        },
    );

    let ops = lib
        .plan_data(&base_world().without_tool("automation-science-pack"))
        .expect("plan refused");
    assert_has_line(
        &transcript(&ops),
        &packless_log("steel-axes", &["automation-science-pack"]),
    );
}

/// AND THE COPIED UNIT'S OWN LIST IS DEDUPED TOO, with no fallback anywhere in
/// the picture: a source technology that names one absent pack twice is one
/// producer repeating itself, and the sentence names the pack once.
#[test]
fn a_copied_unit_naming_one_absent_pack_twice_names_it_once() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: String::from("steel-processing"),
            ..Default::default()
        },
    );

    let w = base_world()
        .with_unit(
            "steel-processing",
            unit_of(
                50,
                15.0,
                &["automation-science-pack", "automation-science-pack"],
            ),
        )
        .without_tool("automation-science-pack");
    let ops = lib.plan_data(&w).expect("plan refused");
    assert_has_line(
        &transcript(&ops),
        &packless_log("steel-axes", &["automation-science-pack"]),
    );
}

/// AND THE NAMES KEEP FIRST-SEEN ORDER, which uniqueness alone does not pin: an
/// implementation keeping the LAST occurrence, or moving the survivor to the
/// end, would name the same three names in another order. Six asks here, in
/// three names: the copied unit lost automation, military and automation again,
/// then the fallback's ladders asked about military, chemical and automation.
#[test]
fn the_packless_names_keep_first_seen_order() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "early", &["early"]);
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_by: Some(CostChoices {
                setting: tier,
                choices: vec![cost_choice("early", &["steel-processing"])],
                fallback: UnitSpec {
                    count: 7,
                    seconds: 8.0,
                    packs: vec![
                        Pack::named(2, "military-science-pack", &["chemical-science-pack"]),
                        Pack::named(2, "automation-science-pack", &[]),
                    ],
                },
            }),
            ..Default::default()
        },
    );

    let w = base_world()
        .with_unit(
            "steel-processing",
            unit_of(
                50,
                15.0,
                &[
                    "automation-science-pack",
                    "military-science-pack",
                    "automation-science-pack",
                ],
            ),
        )
        .without_tool("automation-science-pack")
        .without_tool("chemical-science-pack");
    let ops = lib.plan_data(&w).expect("plan refused");
    assert_has_line(
        &transcript(&ops),
        &packless_log(
            "steel-axes",
            &[
                "automation-science-pack",
                "military-science-pack",
                "chemical-science-pack",
            ],
        ),
    );
}
