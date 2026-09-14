//! THE CUSTOMIZER: the settings a player types into, and everything that
//! binds them.
//!
//! The language itself is pinned by testdata/ingredient-list/cases.txt and
//! exercised in `tests::ingredient_list`. What is here is the BINDING: the
//! setting prototypes the language's declared list produces, the plan-time
//! rules about who may read one, and what the data stage does with what the
//! player wrote.

use crate::plan::{
    CustomCost, Ingredient, IngredientChoice, IngredientChoices, IngredientsSettingRef, ItemSpec,
    Lib, NumericSpec, Pack, PacksSettingRef, RecipeSpec, SettingDecl, SettingKind, TechSpec,
    UnitSpec,
};
use crate::tests::*;
use crate::value::Value;

/// One composed locale reference as the engine renders it: the alternatives
/// form, the key table, and the raw fallback LAST.
///
/// SPELLED OUT HERE RATHER THAN TAKEN FROM THE SOURCE, which is what a golden
/// is for: a test that asked `locale_ref` for the shape would move with any
/// edit to it, and the Go twin writes the same bytes out by hand.
fn wrapped(key: &str, raw: &str) -> Value {
    Value::Arr(vec![
        Value::string("?"),
        Value::Arr(vec![Value::string(key)]),
        Value::string(raw),
    ])
}

/// A research dropdown with one preset, so a refusal about the settings BESIDE
/// it is the sentence under test rather than one about the dropdown's own
/// shape.
fn cheap_tier(setting: crate::plan::DropdownSettingRef) -> crate::plan::CostChoices {
    crate::plan::CostChoices {
        setting,
        choices: vec![crate::plan::CostChoice {
            value: "a".into(),
            sources: strings(&["logistics-2"]),
        }],
        fallback: UnitSpec {
            count: 200,
            seconds: 30.0,
            packs: vec![Pack::new("automation-science-pack", 1)],
        },
    }
}

// ---------------------------------------------------------------------------
// The settings stage.
// ---------------------------------------------------------------------------

/// A TEXT SETTING'S WHOLE PROTOTYPE, and every field of it is a decision.
///
/// `default_value` is the WORD, never the list: the engine writes every
/// setting's current value into mod-settings.dat, untouched defaults included
/// (measured), so a rendered list as the default would freeze the mod's first
/// list into the game of every player who never opened the settings screen.
/// The list is written into the description instead, where a ladder shows its
/// first rung and an item this plan declares shows its prefixed name.
#[test]
fn plan_settings_text_setting_prototypes() {
    let mut lib = Lib::new();
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    let list = lib.ingredients_setting(
        "rivet-ingredients",
        vec![
            Ingredient::named(2, "tungsten-plate", &["steel-plate"]),
            Ingredient::of(rivet, 4),
            Ingredient::fluid(0.5, "water", &[]),
        ],
    );
    // A legacy text setting keeps the name and the order the mod ships.
    let packs = lib.legacy_packs_setting(
        "steelworks-research-packs",
        vec![Pack::named(
            1,
            "military-science-pack",
            &["automation-science-pack"],
        )],
        "z",
    );
    let count = lib.int_setting("tips-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 15, NumericSpec::between(1.0, 600.0));
    lib.recipe(
        rivet,
        RecipeSpec {
            category: "crafting-with-fluid".into(),
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

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_composed(
        &transcript(&ops),
        &[
            r#"extend {type="string-setting", name="steelworks-rivet-ingredients", setting_type="startup", default_value="default", order="aa", auto_trim=true, localised_description=["", ["?", ["mod-setting-description.steelworks-rivet-ingredients"], "steelworks-rivet-ingredients"], "\ndefault: 2 tungsten-plate, 4 steelworks-steel-rivet, 0.5 [fluid=water]"<text tail>]}"#,
            r#"extend {type="string-setting", name="steelworks-research-packs", setting_type="startup", default_value="default", order="z", auto_trim=true, localised_description=["", ["?", ["mod-setting-description.steelworks-research-packs"], "steelworks-research-packs"], "\ndefault: 1 military-science-pack"<packs tail>]}"#,
            r#"extend {type="int-setting", name="steelworks-tips-count", setting_type="startup", default_value=30, order="ac", minimum_value=1, maximum_value=100000, localised_description=["", ["?", ["mod-setting-description.steelworks-tips-count"], "steelworks-tips-count"], "\nA whole number from 1 to 100000."]}"#,
            r#"extend {type="int-setting", name="steelworks-tips-seconds", setting_type="startup", default_value=15, order="ad", minimum_value=1, maximum_value=600, localised_description=["", ["?", ["mod-setting-description.steelworks-tips-seconds"], "steelworks-tips-seconds"], "\nA whole number from 1 to 600."]}"#,
        ],
    );
}

/// An empty declared INGREDIENT list is legal and reads as the word a player
/// would type for it.
#[test]
fn an_empty_declared_ingredient_list_reads_as_none() {
    let mut lib = Lib::new();
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    let list = lib.ingredients_setting("rivet-ingredients", Vec::new());
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_from: Some(list),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops)[..1],
        &[
            r#"extend {type="string-setting", name="steelworks-rivet-ingredients", setting_type="startup", default_value="default", order="aa", auto_trim=true, localised_description=["", ["?", ["mod-setting-description.steelworks-rivet-ingredients"], "steelworks-rivet-ingredients"], "\ndefault: none"<text tail>]}"#,
        ],
    );
}

/// THE TWO LINES, BYTE FOR BYTE, and the number in the first one comes from
/// the same constant the parser refuses on. A sentence promising a limit the
/// parser does not keep is the drift this pins; the Go twin carries the same
/// bytes and the mirror compares the two transcripts.
#[test]
fn the_composed_text_lines_are_the_stated_ones() {
    use crate::ingredient_list::MAX_TEXT;
    use crate::settings::{
        text_format_line, INGREDIENT_PRESET_HEAD, LIST_WRAP_LINE, TEXT_FALLBACK_LINE,
    };

    assert_eq!(
        MAX_TEXT, 2000,
        "the sentence below and both halves' docs say 2000"
    );
    assert_eq!(
        text_format_line(false),
        "\nWrite internal names, as the default line above does, in at most 2000 characters."
    );
    // THE WORD none ON AN INGREDIENT LIST AND NOWHERE ELSE. A packs list
    // refuses it, so the sentence above is the whole packs line and this is the
    // whole ingredient one.
    assert_eq!(
        text_format_line(true),
        "\nWrite internal names, as the default line above does, in at most 2000 characters. The word none empties the list, so the recipe costs nothing to craft."
    );
    assert!(
        !text_format_line(false).contains("none"),
        "a packs setting's format line names the word none: {}",
        text_format_line(false)
    );
    assert_eq!(
        LIST_WRAP_LINE,
        "\nA list too long for one line continues on the next; the continuation is part of the same list."
    );
    assert_eq!(
        TEXT_FALLBACK_LINE,
        "\nA text this mod cannot use is set aside and that default applies instead; the reason is in the log, or in the load error if the load stops anyway."
    );
    // A LINE, NOT A SEPARATOR, and the word a player acts on opens it. The
    // dropdown label beside this is the consumer's prose and the client
    // truncates it at about 37 characters; the internal names are on their own
    // line so the copyable half of the tooltip is never the truncated half.
    assert_eq!(INGREDIENT_PRESET_HEAD, "\n  type: ");
}

/// THE DROPDOWN'S DESCRIPTION IS COMPOSED, because the engine will not let the
/// library pre-fill the text from the value the player had: the settings stage
/// sees no stored value at all (measured, `data.raw` is empty there) and
/// nothing later can write a setting. Showing each preset written out is what
/// is left, and it is enough to start from.
///
/// The label is the value's own locale entry rather than its raw key, because
/// the raw key is not what the dropdown shows.
#[test]
fn plan_settings_composes_a_dropdown_with_a_custom_arm() {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());
    let text = lib.ingredients_setting(
        "quench-ingredients",
        vec![Ingredient::named(2, "steel-plate", &[])],
    );
    lib.recipe(
        plate,
        RecipeSpec {
            category: "crafting-with-fluid".into(),
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![
                    IngredientChoice {
                        value: "water".into(),
                        ingredients: vec![
                            Ingredient::named(2, "steel-plate", &[]),
                            Ingredient::fluid(10.0, "water", &[]),
                        ],
                    },
                    IngredientChoice {
                        value: "oil".into(),
                        ingredients: vec![Ingredient::named(3, "steel-plate", &[])],
                    },
                ],
            }),
            ingredients_from: Some(text),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops)[..1],
        &[
            r#"extend {type="string-setting", name="steelworks-quench-medium", setting_type="startup", default_value="water", order="aa", allowed_values=["water", "oil"], localised_description=["", ["?", ["mod-setting-description.steelworks-quench-medium"], "steelworks-quench-medium"], ["", "\n", ["?", ["string-mod-setting.steelworks-quench-medium-water"], "water"], "\n  type: 2 steel-plate, 10 [fluid=water]"], ["", "\n", ["?", ["string-mod-setting.steelworks-quench-medium-oil"], "oil"], "\n  type: 3 steel-plate"], "\nA list too long for one line continues on the next; the continuation is part of the same list.", "\nThe setting below applies instead while it does not say default."]}"#,
        ],
    );
}

/// THE TWO SWITCH LINES FOLLOW THE EMITTED ORDER, NOT THE DECLARATION ORDER.
///
/// EVERY OTHER FIXTURE IN THIS FILE DECLARES THE DROPDOWN FIRST, so
/// `relative_order` answers "above" on the text side and "below" on the
/// dropdown side in all of them, and a pair of hard-coded constants would pass
/// every one. This is the other half of its domain, and the words it produces
/// here appear nowhere else in the repository: a LEGACY dropdown carrying the
/// consumer's own order "z" sorts UNDER a generated setting whose order is two
/// letters starting at "a", so the text setting points down and the dropdown
/// points up. Whether the author declared them in that order is not what
/// either sentence is about. The Go half holds the same transcript.
#[test]
fn the_switch_lines_follow_the_emitted_order() {
    let mut lib = Lib::new();
    let text = lib.ingredients_setting(
        "quench-ingredients",
        vec![Ingredient::named(2, "steel-plate", &[])],
    );
    let medium = lib.legacy_dropdown_setting_needing_locale(
        "steelworks-quench-medium",
        "water",
        &["water", "oil"],
        "z",
    );
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());
    lib.recipe(
        plate,
        RecipeSpec {
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![
                    IngredientChoice {
                        value: "water".into(),
                        ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
                    },
                    IngredientChoice {
                        value: "oil".into(),
                        ingredients: vec![Ingredient::named(3, "steel-plate", &[])],
                    },
                ],
            }),
            ingredients_from: Some(text),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops),
        &[
            r#"extend {type="string-setting", name="steelworks-quench-ingredients", setting_type="startup", default_value="default", order="aa", auto_trim=true, localised_description=["", ["?", ["mod-setting-description.steelworks-quench-ingredients"], "steelworks-quench-ingredients"], "\ndefault: 2 steel-plate"<text tail below>]}"#,
            r#"extend {type="string-setting", name="steelworks-quench-medium", setting_type="startup", default_value="water", order="z", allowed_values=["water", "oil"], localised_description=["", ["?", ["mod-setting-description.steelworks-quench-medium"], "steelworks-quench-medium"], ["", "\n", ["?", ["string-mod-setting.steelworks-quench-medium-water"], "water"], "\n  type: 2 steel-plate"], ["", "\n", ["?", ["string-mod-setting.steelworks-quench-medium-oil"], "oil"], "\n  type: 3 steel-plate"], "\nA list too long for one line continues on the next; the continuation is part of the same list.", "\nThe setting above applies instead while it does not say default."]}"#,
        ],
    );
}

/// A RESEARCH preset is a technology whose cost is copied, so what the line
/// says is which one, in the technology's own localised name: the internal
/// name rides inside the name key and is not shown to the player.
#[test]
fn plan_settings_composes_a_cost_dropdown_with_a_custom_arm() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale(
        "tips-research-tier",
        "projectile",
        &["projectile", "military"],
    );
    let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_by: Some(crate::plan::CostChoices {
                setting: tier,
                choices: vec![
                    crate::plan::CostChoice {
                        value: "projectile".into(),
                        sources: strings(&["tungsten-hardening", "logistics-2"]),
                    },
                    crate::plan::CostChoice {
                        value: "military".into(),
                        sources: strings(&["logistics-3"]),
                    },
                ],
                fallback: UnitSpec {
                    count: 200,
                    seconds: 30.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
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

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops)[..1],
        &[
            r#"extend {type="string-setting", name="steelworks-tips-research-tier", setting_type="startup", default_value="projectile", order="aa", allowed_values=["projectile", "military"], localised_description=["", ["?", ["mod-setting-description.steelworks-tips-research-tier"], "steelworks-tips-research-tier"], ["", "\n", ["?", ["string-mod-setting.steelworks-tips-research-tier-projectile"], "projectile"], ": cost of ", ["?", ["technology-name.tungsten-hardening"], "tungsten-hardening"]], ["", "\n", ["?", ["string-mod-setting.steelworks-tips-research-tier-military"], "military"], ": cost of ", ["?", ["technology-name.logistics-3"], "logistics-3"]], "\nThe setting below applies instead while it does not say default."]}"#,
        ],
    );
}

/// A COST PRESET WHOSE LADDER IS EMPTY says what it actually costs. Every
/// other preset names the technology whose unit it copies; this one has no
/// source to name, so the fallback is what pays and the line says so rather
/// than trailing off after the colon.
#[test]
fn a_cost_preset_with_no_source_reads_as_the_fallback() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale("tier", "cheap", &["cheap"]);
    let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_by: Some(crate::plan::CostChoices {
                setting: tier,
                choices: vec![crate::plan::CostChoice {
                    value: "cheap".into(),
                    sources: Vec::new(),
                }],
                fallback: UnitSpec {
                    count: 200,
                    seconds: 30.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
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

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops)[..1],
        &[
            r#"extend {type="string-setting", name="steelworks-tier", setting_type="startup", default_value="cheap", order="aa", allowed_values=["cheap"], localised_description=["", ["?", ["mod-setting-description.steelworks-tier"], "steelworks-tier"], ["", "\n", ["?", ["string-mod-setting.steelworks-tier-cheap"], "cheap"], ": the fallback cost"], "\nThe setting below applies instead while it does not say default."]}"#,
        ],
    );
}

/// THE SOURCE TECHNOLOGY IS NAMED THE WAY THE PLAYER KNOWS IT. The line used
/// to read `cost of tungsten-hardening`, which is an internal name and a
/// string nobody sees anywhere else in the game; it hands the engine the
/// technology's own name key now and lets the engine render it. The internal
/// name survives inside that key and nowhere else.
///
/// The KEY IS THE GAME'S, so the locale checker gains no obligation. What a
/// modpack without that technology shows is the engine's missing-key MARKER,
/// `Unknown key: "technology-name.tungsten-hardening"`, not the bare name; the
/// measured shape is the note on `Lib::check_locale`, and the trade is stated
/// on `cost_preset_tail`.
///
/// The whole nested value is asserted, because the two shapes differ by a
/// parameter: a choice with a source ends in the words and the name table,
/// and one with an empty ladder has no technology to name and keeps the four
/// parameters it always had.
#[test]
fn a_cost_preset_names_its_source_by_its_localised_name() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale(
        "tips-research-tier",
        "projectile",
        &["projectile", "military", "cheap"],
    );
    let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_by: Some(crate::plan::CostChoices {
                setting: tier,
                choices: vec![
                    crate::plan::CostChoice {
                        value: "projectile".into(),
                        sources: strings(&["tungsten-hardening", "logistics-2"]),
                    },
                    crate::plan::CostChoice {
                        value: "military".into(),
                        sources: strings(&["logistics-3"]),
                    },
                    crate::plan::CostChoice {
                        value: "cheap".into(),
                        sources: Vec::new(),
                    },
                ],
                fallback: UnitSpec {
                    count: 200,
                    seconds: 30.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
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

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    let crate::op::Op::Extend(proto) = &ops[0] else {
        panic!("the first op is not the dropdown")
    };
    assert_eq!(
        field(proto, "localised_description").expect("no description"),
        Value::Arr(vec![
            Value::string(""),
            wrapped(
                "mod-setting-description.steelworks-tips-research-tier",
                "steelworks-tips-research-tier"
            ),
            // FIVE PARAMETERS: the concatenating key, the newline, the
            // dropdown's own label, the words, and the technology's name key.
            // Each key rides inside the alternatives form with its raw
            // fallback LAST, which is what keeps an undefined key from taking
            // the whole tooltip.
            Value::Arr(vec![
                Value::string(""),
                Value::string("\n"),
                wrapped(
                    "string-mod-setting.steelworks-tips-research-tier-projectile",
                    "projectile"
                ),
                Value::string(": cost of "),
                wrapped("technology-name.tungsten-hardening", "tungsten-hardening"),
            ]),
            // The FIRST rung is the one named; the rest are what a modpack
            // missing it falls back to.
            Value::Arr(vec![
                Value::string(""),
                Value::string("\n"),
                wrapped(
                    "string-mod-setting.steelworks-tips-research-tier-military",
                    "military"
                ),
                Value::string(": cost of "),
                wrapped("technology-name.logistics-3", "logistics-3"),
            ]),
            // FOUR: nothing to name, so nothing is nested.
            Value::Arr(vec![
                Value::string(""),
                Value::string("\n"),
                wrapped(
                    "string-mod-setting.steelworks-tips-research-tier-cheap",
                    "cheap"
                ),
                Value::string(": the fallback cost"),
            ]),
            Value::string("\nThe setting below applies instead while it does not say default."),
        ]),
        "the composed description is not the shape the engine renders"
    );
}

/// MEASURED (2.0.77): a localised string takes at most 20 parameters and 20
/// levels of nesting, and 21 of either refuses the load naming nothing useful.
/// The consumer's own description key is the first parameter and the last two
/// are the wrap line and the switch line, so 17 presets ride at the top level
/// and the eighteenth turns the lot into groups of 19.
///
/// SEVENTEEN AND NOT EIGHTEEN IS WHAT THE WRAP LINE COST, and that is the
/// whole of what it cost: one parameter of the top table, on the one
/// composition that can reach the ceiling at all. Nesting is a fill rather
/// than a wall, so past it the description keeps working.
#[test]
fn an_ingredient_description_nests_past_seventeen_presets() {
    let composed = |presets: usize| -> Value {
        let mut lib = Lib::new();
        let values: Vec<String> = (0..presets).map(|i| alloc::format!("p{}", i)).collect();
        let refs: Vec<&str> = values.iter().map(|v| v.as_str()).collect();
        let setting = lib.dropdown_setting_needing_locale("tier", "p0", &refs);
        let plate = lib.item("hardened-steel-plate", ItemSpec::default());
        let text = lib.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
        lib.recipe(
            plate,
            RecipeSpec {
                ingredients_by: Some(IngredientChoices {
                    setting,
                    choices: (0..presets)
                        .map(|i| IngredientChoice {
                            value: alloc::format!("p{}", i),
                            ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                        })
                        .collect(),
                }),
                ingredients_from: Some(text),
                ..Default::default()
            },
        );
        let ops = lib.plan_settings(&settings_world()).expect("plan refused");
        match &ops[0] {
            crate::op::Op::Extend(proto) => {
                field(proto, "localised_description").expect("no description")
            }
            _ => panic!("the first op is not the dropdown"),
        }
    };

    // Seventeen presets: the key plus seventeen lines plus the wrap line plus
    // the switch line is twenty parameters, the measured ceiling, and nothing
    // nests.
    let Value::Arr(flat) = composed(17) else {
        panic!("the description is not a localised string")
    };
    assert_eq!(flat.len(), 21, "seventeen presets did not stay flat");
    assert_eq!(
        flat.last(),
        Some(&Value::string(
            "\nThe setting below applies instead while it does not say default."
        )),
        "the last parameter is not the switch line"
    );
    assert_eq!(
        flat[flat.len() - 2],
        Value::string(
            "\nA list too long for one line continues on the next; the continuation is part of the same list."
        ),
        "the second to last parameter is not the wrap line"
    );

    // Eighteen: the level keeps the first nineteen parameters and hands the
    // rest to a nested group in the twentieth slot.
    let Value::Arr(nested) = composed(18) else {
        panic!("the description is not a localised string")
    };
    assert_eq!(nested.len(), 21, "the nested form is not one level wide");
    let Value::Arr(group) = nested.last().expect("no last parameter") else {
        panic!("the last parameter is not a localised string")
    };
    // The empty key that concatenates, then the two lines that did not fit,
    // which are the wrap line and the switch line in that order: the
    // eighteenth preset takes the last flat slot and the two trailing lines
    // are what the level hands on.
    assert_eq!(
        group.len(),
        3,
        "the nested group does not hold what the level could not"
    );
    assert_eq!(
        group[1],
        Value::string(
            "\nA list too long for one line continues on the next; the continuation is part of the same list."
        ),
        "the nested group does not open with the wrap line"
    );
    assert_eq!(
        group[2],
        Value::string("\nThe setting below applies instead while it does not say default."),
        "the nested group does not end with the switch line"
    );

    // And one preset further, the group holds a preset LINE as well, which is
    // what says the fill keeps going rather than stopping at the two lines.
    let Value::Arr(deeper) = composed(19) else {
        panic!("the description is not a localised string")
    };
    let Value::Arr(group) = deeper.last().expect("no last parameter") else {
        panic!("the last parameter is not a localised string")
    };
    assert_eq!(group.len(), 4, "the nested group is not four elements wide");
    assert!(
        matches!(group[1], Value::Arr(_)),
        "the nested group's first member is not a preset line"
    );
}

/// THE COST DROPDOWN NESTS THE SAME WAY, and this is the pin that says the
/// five-parameter preset line did not change that. A line is ONE parameter of
/// the group above it however many tables sit inside it, so the technology's
/// name table costs the ceiling nothing.
///
/// THE NAME TABLE IS A SIBLING OF THE LABEL, not a level under it: both are
/// direct children of the preset line, so the deepest table in a flat
/// description is FOUR levels down, which is the three that composition had
/// before `locale_ref` plus the one level every wrapper spends, nowhere near
/// the twenty the engine takes.
#[test]
fn a_cost_dropdown_description_nests_past_nineteen_presets() {
    let composed = |presets: usize| -> Value {
        let mut lib = Lib::new();
        let values: Vec<String> = (0..presets).map(|i| alloc::format!("t{}", i)).collect();
        let refs: Vec<&str> = values.iter().map(|v| v.as_str()).collect();
        let setting = lib.dropdown_setting_needing_locale("tier", "t0", &refs);
        let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
        let count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
        let seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
        lib.technology(
            "hardened-tips",
            TechSpec {
                cost_by: Some(crate::plan::CostChoices {
                    setting,
                    choices: (0..presets)
                        .map(|i| crate::plan::CostChoice {
                            value: alloc::format!("t{}", i),
                            sources: vec![alloc::format!("source-{}", i)],
                        })
                        .collect(),
                    fallback: UnitSpec {
                        count: 200,
                        seconds: 30.0,
                        packs: vec![Pack::new("automation-science-pack", 1)],
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
        let ops = lib.plan_settings(&settings_world()).expect("plan refused");
        match &ops[0] {
            crate::op::Op::Extend(proto) => {
                field(proto, "localised_description").expect("no description")
            }
            _ => panic!("the first op is not the dropdown"),
        }
    };

    // How far down the deepest TABLE sits, counting the description itself as
    // level one. Tables are what the engine counts; a string is not a level.
    fn deepest_table(v: &Value, depth: usize) -> usize {
        match v {
            Value::Arr(items) => items
                .iter()
                .map(|item| deepest_table(item, depth + 1))
                .max()
                .unwrap_or(0)
                .max(depth),
            _ => 0,
        }
    }
    // Two presets, so nothing nests: description, preset line, the wrapper
    // around the technology name, and the name table inside it. The fourth
    // level is the one every `locale_ref` spends, against twenty the engine
    // takes.
    assert_eq!(
        deepest_table(&composed(2), 1),
        4,
        "a flat cost description is not four tables deep"
    );

    // MEASURED (2.0.77): twenty parameters load and twenty-one refuse. The
    // consumer's own description key is the first, so a level holds at most
    // twenty-one slots counting the empty key that concatenates them.
    let description = composed(21);
    let Value::Arr(top) = &description else {
        panic!("the description is not a localised string")
    };
    assert_eq!(top.len(), 21, "the top level is not one level wide");
    let Value::Arr(tail) = top.last().expect("no last parameter") else {
        panic!("the last slot is not a localised string")
    };
    // The empty key that concatenates, then the three lines that did not fit
    // and the switch line after them.
    assert_eq!(
        tail.len(),
        5,
        "the nested group does not hold what the level could not"
    );

    fn walk(v: &Value, depth: usize, lines: &mut usize) {
        let Value::Arr(items) = v else { return };
        assert!(
            items.len() <= 21,
            "a level holds {} parameters, want at most 21",
            items.len()
        );
        assert!(depth <= 20, "the nesting is deeper than the engine takes");
        // A preset line rather than a group: the newline in the second slot is
        // what tells them apart, and a line is counted rather than descended
        // into.
        if items.len() > 1 && items[1] == Value::string("\n") {
            *lines += 1;
            assert_eq!(
                items.len(),
                5,
                "a cost preset line holds {} parameters, want 5",
                items.len()
            );
            return;
        }
        for item in items {
            walk(item, depth + 1, lines);
        }
    }
    let mut lines = 0usize;
    walk(&description, 1, &mut lines);
    assert_eq!(lines, 21, "the description holds {} preset lines", lines);
}

// ---------------------------------------------------------------------------
// Plan-time refusals.
// ---------------------------------------------------------------------------

/// Everything a binding has to be true of, in both planners: the settings
/// stage renders the same declared list the data stage emits, so a
/// declaration neither can serve is refused by both.
/// The declaration both rows of the duplicate case build, so the two planners
/// are asked about ONE plan rather than about two that happen to look alike.
fn duplicate_default(l: &mut Lib) {
    let plate = l.item("hardened-steel-plate", ItemSpec::default());
    let list = l.ingredients_setting(
        "parts",
        vec![
            Ingredient::named(1, "steel-plate", &[]),
            Ingredient::named(2, "steel-plate", &[]),
        ],
    );
    l.recipe(
        plate,
        RecipeSpec {
            ingredients_from: Some(list),
            ..Default::default()
        },
    );
}

#[test]
fn customizer_refusals() {
    struct Case {
        name: &'static str,
        build: fn(&mut Lib),
        want: &'static str,
        /// Which planner is asked. Both run the text-setting rules; the
        /// binding rules live where the binding is read.
        data: bool,
    }

    let cases = [
        Case {
            name: "IngredientsFrom beside Ingredients",
            data: true,
            build: |l: &mut Lib| {
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let list = l.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-rivet names both Ingredients and IngredientsFrom; pick one",
        },
        Case {
            name: "an IngredientsFrom handle this plan never issued",
            data: true,
            build: |l: &mut Lib| {
                let rivet = l.item("steel-rivet", ItemSpec::default());
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_from: Some(Default::default()),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-rivet reads its ingredients from a setting that this plan never declared",
        },
        Case {
            name: "CostFrom beside Unit",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        unit: Some(UnitSpec {
                            count: 45,
                            seconds: 20.0,
                            packs: vec![Pack::new("automation-science-pack", 1)],
                        }),
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips must name exactly one of CostOf, Unit, CostBy or CostFrom",
        },
        Case {
            // THE NUMBER RULE STEPS PAST WHAT THE OTHER SENTENCES OWN, and
            // this technology is the one that made it necessary: it names two
            // cost sources, so it has not said what its research costs, and
            // the count it reads is one another technology reads too. Counting
            // it would answer an undeclared cost with a sentence about
            // sharing.
            name: "two cost sources over one count",
            data: true,
            build: |l: &mut Lib| {
                let axe_packs =
                    l.packs_setting("axe-packs", vec![Pack::new("automation-science-pack", 1)]);
                let saw_packs =
                    l.packs_setting("saw-packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("shared-count", 30, NumericSpec::between(1.0, 100.0));
                let axe_seconds = l.int_setting("axe-seconds", 15, NumericSpec::between(1.0, 60.0));
                let saw_seconds = l.int_setting("saw-seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_of: "logistics-2".into(),
                        cost_from: Some(CustomCost {
                            packs: axe_packs,
                            count,
                            seconds: axe_seconds,
                        }),
                        ..Default::default()
                    },
                );
                l.technology(
                    "steel-saws",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs: saw_packs,
                            count,
                            seconds: saw_seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips must name exactly one of CostOf, Unit, CostBy or CostFrom",
        },
        Case {
            // The recipe twin: a crafting time named twice, once by hand and
            // once by a handle. "Pick one" is what the author has to fix
            // first, and it is the data planner's line.
            name: "CraftTime beside CraftTimeFrom",
            data: true,
            build: |l: &mut Lib| {
                let axe = l.item("steel-axe", ItemSpec::default());
                let seconds =
                    l.double_setting("forge-seconds", 15.0, NumericSpec::between(1.0, 60.0));
                l.recipe(
                    axe,
                    RecipeSpec {
                        craft_time: 2.0,
                        craft_time_from: seconds,
                        ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-axe names both CraftTime and CraftTimeFrom; pick one",
        },
        Case {
            name: "a packs handle this plan never issued",
            data: true,
            build: |l: &mut Lib| {
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs: Default::default(),
                            count,
                            seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips reads its science packs from a setting that this plan never declared",
        },
        Case {
            name: "a Count handle this plan never issued",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count: Default::default(),
                            seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips reads its research count from a setting that this plan never declared",
        },
        Case {
            name: "a Seconds handle this plan never issued",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds: Default::default(),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips reads its research time from a setting that this plan never declared",
        },
        Case {
            // The engine refuses a unit count of 0, and it RESETS a stored
            // value outside a setting's own bounds to that setting's default
            // rather than clamping it, so the declared minimum is what makes
            // every readable value legal.
            name: "a Count setting with no minimum of at least 1",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(0.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
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
            },
            want: "fkrecipes: the setting count backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
        },
        Case {
            name: "a Seconds setting with no minimum of at least 1",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(0.0, 60.0));
                l.technology(
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
            },
            want: "fkrecipes: the setting seconds backs a research time but declares no minimum of at least 1 (the engine refuses a unit time of 0)",
        },
        Case {
            // A FIELD THE PLAYER TYPES INTO NEEDS A CEILING, and the settings
            // screen has no other one to show them: an int setting with no
            // maximum_value takes any number the engine's own encoding holds.
            name: "a Count setting with no maximum",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting(
                    "count",
                    30,
                    NumericSpec {
                        min: Some(1.0),
                        max: None,
                    },
                );
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
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
            },
            want: "fkrecipes: the setting count backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach",
        },
        Case {
            name: "a Seconds setting with no maximum",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting(
                    "seconds",
                    15,
                    NumericSpec {
                        min: Some(1.0),
                        max: None,
                    },
                );
                l.technology(
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
            },
            want: "fkrecipes: the setting seconds backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach",
        },
        Case {
            // A DECLARED MAXIMUM OF ZERO IS THE OTHER ARM OF THE SAME RULE, and
            // it is why the sentence says "no maximum of at least 1" rather
            // than "no maximum": this declaration carries one, and 0 is the
            // only value it allows, which is the word that means "the dropdown
            // decides" and never a cost. A sentence naming an absent maximum
            // would be false of exactly this case. It is written beside a
            // dropdown because that is where a 0 minimum is legal at all; with
            // no dropdown the minimum rule answers first. The Go half holds the
            // same case and the same sentence.
            name: "a Count setting beside a dropdown whose maximum is zero",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a"]);
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 0, NumericSpec::between(0.0, 0.0));
                let seconds = l.int_setting("seconds", 0, NumericSpec::between(0.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(cheap_tier(tier)),
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting count backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach",
        },
        Case {
            // BESIDE A DROPDOWN THE RULE INVERTS, because 0 is what a number
            // says instead of the reserved word: a declared default of
            // anything else would price the research out of a field the
            // player never touched and leave the tier saying nothing.
            name: "a Count setting beside a dropdown whose default is not zero",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a"]);
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(0.0, 100.0));
                let seconds = l.int_setting("seconds", 0, NumericSpec::between(0.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(cheap_tier(tier)),
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting count backs a research count beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
        },
        Case {
            name: "a Count setting beside a dropdown whose minimum is not zero",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a"]);
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting(
                    "count",
                    0,
                    NumericSpec {
                        min: None,
                        max: Some(100.0),
                    },
                );
                let seconds = l.int_setting("seconds", 0, NumericSpec::between(0.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(cheap_tier(tier)),
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting count backs a research count beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
        },
        Case {
            name: "a Seconds setting beside a dropdown whose default is not zero",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a"]);
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 0, NumericSpec::between(0.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(0.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(cheap_tier(tier)),
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting seconds backs a research time beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
        },
        Case {
            name: "a text setting nothing reads",
            data: false,
            build: |l: &mut Lib| {
                l.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
            },
            want: "fkrecipes: the setting parts is declared and nothing reads it; a text setting must be bound to one recipe or technology",
        },
        Case {
            name: "a text setting two recipes read",
            data: false,
            build: |l: &mut Lib| {
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let plate = l.item("steel-plate", ItemSpec::default());
                let list = l.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting parts is read by more than one recipe or technology; a text setting serves exactly one",
        },
        Case {
            // The sentence about packs the game does not have is reserved for
            // a list that named some and lost them all.
            name: "a packs setting that prices nothing",
            data: false,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", Vec::new());
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
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
            },
            want: "fkrecipes: the packs setting packs declares no science pack; research takes at least one",
        },
        Case {
            name: "a pack amount the engine refuses",
            data: false,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 0)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
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
            },
            want: "fkrecipes: the packs setting packs has a science pack amount below 1, which the engine refuses",
        },
        Case {
            name: "a pack rung that can never resolve",
            data: false,
            build: |l: &mut Lib| {
                let packs = l.packs_setting(
                    "packs",
                    vec![Pack::named(1, "automation-science-pack", &[""])],
                );
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(1.0, 60.0));
                l.technology(
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
            },
            want: "fkrecipes: the packs setting packs names a science pack with an empty name",
        },
        Case {
            // The renderer turns a non-finite amount into text that does not
            // parse back, and the settings stage renders the DECLARED list, so
            // the check that guards the typed path has to guard this one too.
            name: "a declared fluid amount that is not a number",
            data: false,
            build: |l: &mut Lib| {
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let list = l.ingredients_setting(
                    "parts",
                    vec![Ingredient::fluid(f64::INFINITY, "water", &[])],
                );
                l.recipe(
                    plate,
                    RecipeSpec {
                        category: "chemistry".into(),
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the ingredients setting parts declares a fluid amount that is not a finite number",
        },
        Case {
            name: "a declared fluid amount above the measured ceiling",
            data: false,
            build: |l: &mut Lib| {
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let list =
                    l.ingredients_setting("parts", vec![Ingredient::fluid(1e302, "water", &[])]);
                l.recipe(
                    plate,
                    RecipeSpec {
                        category: "chemistry".into(),
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the ingredients setting parts takes the fluid water at an amount above 1e301, which the game cannot hold",
        },
        Case {
            // THE DEFAULT THE PLAYER COULD NEVER RESTORE. The description
            // shows the declared list; a rendering the language refuses is a
            // field with a trapdoor in it. A name outside the prototype
            // charset is the shape that reaches it: the renderer writes the
            // tag that would carry such a name, and the tag will not hold it.
            name: "a declared default the language cannot read back",
            data: false,
            build: |l: &mut Lib| {
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let list =
                    l.ingredients_setting("parts", vec![Ingredient::named(1, "iron plate", &[])]);
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: r#"fkrecipes: the ingredients setting parts, entry 1 ("1 [item=iron plate]"): a tag is [item=name] or [fluid=name]"#,
        },
        Case {
            // THE ITEM CEILING IS THE AUTHOR'S TOO. The engine holds an item
            // amount in a u16 and refuses 65536 (measured), so a declared
            // list is held to exactly what the language holds a typed one to,
            // and the sentence names what the recipe takes.
            name: "a declared ingredient amount above the item ceiling",
            data: true,
            build: |l: &mut Lib| {
                let rivet = l.item("steel-rivet", ItemSpec::default());
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients: vec![Ingredient::named(70000, "iron-plate", &[])],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-rivet takes 70000 of iron-plate, and an item amount goes up to 65535",
        },
        Case {
            // The same rule over an ingredient naming THIS PLAN'S OWN item,
            // which carries no ladder to name: the sentence takes the item's
            // declared name instead, and the handle is proved before it is
            // read.
            name: "a declared amount above the ceiling of this plan's own item",
            data: true,
            build: |l: &mut Lib| {
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients: vec![Ingredient::of(rivet, 65536)],
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe hardened-steel-plate takes 65536 of steel-rivet, and an item amount goes up to 65535",
        },
        Case {
            // The text setting's declared list is held to it as well, before
            // the renderer ever sees the amount.
            name: "a declared text default above the item ceiling",
            data: false,
            build: |l: &mut Lib| {
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let list = l
                    .ingredients_setting("parts", vec![Ingredient::named(70000, "iron-plate", &[])]);
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the ingredients setting parts takes 70000 of iron-plate, and an item amount goes up to 65535",
        },
        Case {
            // A MINIMUM OF EXACTLY ZERO IS NOT A MINIMUM ABOVE ZERO, and the
            // engine refuses a unit time of 0. The boundary is the whole point
            // of the check: a declared 0 reads back as 0.
            name: "a Seconds setting whose declared minimum is exactly zero",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.int_setting("seconds", 15, NumericSpec::between(0.0, 60.0));
                l.technology(
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
            },
            want: "fkrecipes: the setting seconds backs a research time but declares no minimum of at least 1 (the engine refuses a unit time of 0)",
        },
        Case {
            // A fluid in a crafting recipe is the RECIPE's rule, so it is
            // asked where the recipe is known and it names the setting that
            // carries the fluid.
            name: "a declared fluid the bound recipe cannot take",
            data: true,
            build: |l: &mut Lib| {
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let list =
                    l.ingredients_setting("parts", vec![Ingredient::fluid(10.0, "water", &[])]);
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the ingredients setting parts takes the fluid water, and a recipe in the crafting category takes items only",
        },
        Case {
            // THE CATEGORY TRAVELS DOWN THE CUSTOM ARM TOO. A text setting a
            // dropdown hands to the player is bound to that dropdown's recipe
            // exactly as IngredientsFrom is, so its declared fluid is asked
            // the same question about the same category.
            name: "a bound text setting's declared fluid the recipe cannot take",
            data: true,
            build: |l: &mut Lib| {
                let medium =
                    l.dropdown_setting_needing_locale("medium", "water", &["water"]);
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let list =
                    l.ingredients_setting("parts", vec![Ingredient::fluid(10.0, "water", &[])]);
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                            }],
                        }),
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the ingredients setting parts takes the fluid water, and a recipe in the crafting category takes items only",
        },
        Case {
            // The round trip is what makes the description a text the player
            // can copy back into the field, and it is what answers a TEXT
            // SETTING's declared duplicate: the declaration checks do not see
            // one and the language does. A plain list and a dropdown preset
            // have no round trip, so those are refused by
            // `validate_no_duplicates` with a sentence of their own
            // (`tests::data`); either way the resolver's merge never meets a
            // duplicate that was in the declaration.
            //
            // BOTH PLANNERS, because both run the text-setting rules. The two
            // rows are one case asked twice, which is the shape this table has
            // for a refusal neither stage may miss.
            name: "a declared default naming one thing twice",
            data: false,
            build: duplicate_default,
            want: "fkrecipes: the ingredients setting parts: entries 1 and 2 both name steel-plate",
        },
        Case {
            name: "a declared default naming one thing twice, at the data stage",
            data: true,
            build: duplicate_default,
            want: "fkrecipes: the ingredients setting parts: entries 1 and 2 both name steel-plate",
        },
        Case {
            // ONE DROPDOWN COMPOSES ONE DESCRIPTION, so a second recipe's text
            // setting would silently replace the first's preset list.
            name: "a dropdown taking a text setting from two recipes",
            data: true,
            build: |l: &mut Lib| {
                let medium = l.dropdown_setting_needing_locale("medium", "water", &["water"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let first =
                    l.ingredients_setting("first", vec![Ingredient::named(1, "iron-plate", &[])]);
                let second =
                    l.ingredients_setting("second", vec![Ingredient::named(2, "iron-plate", &[])]);
                let arm = || IngredientChoices {
                    setting: medium,
                    choices: vec![IngredientChoice {
                        value: "water".into(),
                        ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                    }],
                };
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_by: Some(arm()),
                        ingredients_from: Some(first),
                        ..Default::default()
                    },
                );
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(arm()),
                        ingredients_from: Some(second),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting steelworks-medium takes a text setting from more than one recipe; one dropdown composes one description",
        },
        Case {
            name: "a dropdown taking a text setting from two technologies",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a"]);
                let first_count = l.int_setting("first-count", 0, NumericSpec::between(0.0, 100.0));
                let first_seconds =
                    l.int_setting("first-seconds", 0, NumericSpec::between(0.0, 60.0));
                let second_count =
                    l.int_setting("second-count", 0, NumericSpec::between(0.0, 100.0));
                let second_seconds =
                    l.int_setting("second-seconds", 0, NumericSpec::between(0.0, 60.0));
                let first = l.packs_setting("first", vec![Pack::new("automation-science-pack", 1)]);
                let second =
                    l.packs_setting("second", vec![Pack::new("automation-science-pack", 2)]);
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(cheap_tier(tier)),
                        cost_from: Some(CustomCost {
                            packs: first,
                            count: first_count,
                            seconds: first_seconds,
                        }),
                        ..Default::default()
                    },
                );
                l.technology(
                    "hardened-edges",
                    TechSpec {
                        cost_by: Some(cheap_tier(tier)),
                        cost_from: Some(CustomCost {
                            packs: second,
                            count: second_count,
                            seconds: second_seconds,
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting steelworks-tier takes a text setting from more than one technology; one dropdown composes one description",
        },
    ];

    for c in cases {
        let mut lib = Lib::new();
        (c.build)(&mut lib);
        let got = if c.data {
            lib.plan_data(&base_world()).err()
        } else {
            lib.plan_settings(&settings_world()).err()
        };
        assert_eq!(got.as_deref(), Some(c.want), "{}", c.name);
    }
}

/// A RESEARCH COUNT OR TIME SERVES EXACTLY ONE DECLARATION, and the composed
/// description is why: it names ONE dropdown as the thing deciding while the
/// field is 0, so a setting two technologies priced themselves with would be
/// described by whichever of them composed last.
///
/// A NUMBER NO RESEARCH COST READS IS STILL SHARED FREELY. One double behind
/// two recipes' crafting time is a mod-wide speed dial and nothing ever says
/// anything about it, so the last case here is ACCEPTED and the rule stays
/// about the numbers a custom cost claims.
///
/// BOTH PLANNERS, because both run the binding rules: the settings stage
/// composes the very descriptions the rule protects.
#[test]
fn a_research_number_serves_exactly_one_declaration() {
    // One count read by the CostFrom of two different technologies.
    let shared_count = || {
        let mut lib = Lib::new();
        let count = lib.int_setting("research-count", 30, NumericSpec::between(1.0, 100000.0));
        let chain_seconds = lib.int_setting("chain-seconds", 10, NumericSpec::between(1.0, 600.0));
        let tips_seconds = lib.int_setting("tips-seconds", 15, NumericSpec::between(1.0, 600.0));
        let chain_packs =
            lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
        let tips_packs =
            lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
        lib.technology(
            "chain-forging",
            TechSpec {
                cost_from: Some(CustomCost {
                    packs: chain_packs,
                    count,
                    seconds: chain_seconds,
                }),
                after: "steel-processing".into(),
                ..Default::default()
            },
        );
        lib.technology(
            "hardened-tips",
            TechSpec {
                cost_from: Some(CustomCost {
                    packs: tips_packs,
                    count,
                    seconds: tips_seconds,
                }),
                ..Default::default()
            },
        );
        lib
    };
    let both_planners = |lib: Lib, want: &str, case: &str| {
        assert_eq!(
            lib.plan_data(&base_world()).err().as_deref(),
            Some(want),
            "{}, at the data stage",
            case
        );
        assert_eq!(
            lib.plan_settings(&settings_world()).err().as_deref(),
            Some(want),
            "{}, at the settings stage",
            case
        );
    };
    both_planners(
        shared_count(),
        "fkrecipes: the setting research-count is read as a research count or time by more than one declaration; a custom cost's number serves exactly one",
        "a count two technologies price themselves with",
    );

    // THE MIXED PAIR IS NOT REPRESENTABLE ANY MORE. One double backing a
    // recipe's crafting time and a research time at once was the case this
    // rule was written for; `CustomCost::seconds` is an `IntSettingRef` now
    // and `craft_time_from` is a `DoubleSettingRef`, so no handle fits both
    // slots and the two technologies above are the whole remaining domain.

    // TWO RECIPES, ONE CRAFTING TIME, AND NOTHING IS REFUSED: no research cost
    // claims this double, so no line ever says it was ignored.
    let mut speed_dial = Lib::new();
    let axe = speed_dial.item("steel-axe", ItemSpec::default());
    let rivet = speed_dial.item("steel-rivet", ItemSpec::default());
    let forging = speed_dial.double_setting("forging-time", 2.5, NumericSpec::default());
    for item in [axe, rivet] {
        speed_dial.recipe(
            item,
            RecipeSpec {
                craft_time_from: forging,
                ..Default::default()
            },
        );
    }
    speed_dial
        .plan_data(&base_world())
        .expect("two recipes sharing one crafting time were refused");
    speed_dial
        .plan_settings(&settings_world())
        .expect("two recipes sharing one crafting time were refused");
}

/// THE TEXT RULE ANSWERS FIRST when a plan breaks both bound-exactly-once
/// rules. It is the older sentence and the one an author reads faster, and the
/// order is a parity pin: the two halves walk one plan and must name the same
/// setting.
#[test]
fn an_unbound_text_setting_is_named_before_a_shared_research_number() {
    let mut lib = Lib::new();
    lib.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
    let count = lib.int_setting("research-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("chain-seconds", 10, NumericSpec::between(1.0, 600.0));
    let packs = lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
    let more = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    for (name, p) in [("chain-forging", packs), ("hardened-tips", more)] {
        lib.technology(
            name,
            TechSpec {
                cost_from: Some(CustomCost {
                    packs: p,
                    count,
                    seconds,
                }),
                after: "steel-processing".into(),
                ..Default::default()
            },
        );
    }

    assert_eq!(
        lib.plan_data(&base_world()).err().as_deref(),
        Some("fkrecipes: the setting parts is declared and nothing reads it; a text setting must be bound to one recipe or technology")
    );
}

/// A PRESET THAT HAPPENS TO BE SPELLED `custom` STAYS A PRESET. The refusal
/// beside it is for a value with NOTHING BEHIND IT; a mod that already ships a
/// preset called custom keeps it, because renaming it would reset every player
/// who had chosen it, which is the stored-preference loss the whole migration
/// path exists to avoid. Both twins, because both dropdowns carry the rule.
#[test]
fn a_dropdown_whose_choices_cover_custom_needs_no_arm() {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![
                    IngredientChoice {
                        value: "water".into(),
                        ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                    },
                    IngredientChoice {
                        value: "custom".into(),
                        ingredients: vec![Ingredient::named(2, "iron-plate", &[])],
                    },
                ],
            }),
            ..Default::default()
        },
    );
    let tier = lib.dropdown_setting_needing_locale("tier", "a", &["a", "custom"]);
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_by: Some(crate::plan::CostChoices {
                setting: tier,
                choices: vec![
                    crate::plan::CostChoice {
                        value: "a".into(),
                        sources: strings(&["logistics-2"]),
                    },
                    crate::plan::CostChoice {
                        value: "custom".into(),
                        sources: strings(&["logistics-3"]),
                    },
                ],
                fallback: UnitSpec {
                    count: 200,
                    seconds: 30.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
                },
            }),
            ..Default::default()
        },
    );

    lib.plan_settings(&settings_world())
        .expect("the settings plan refused a preset spelled custom");
    lib.plan_data(&base_world())
        .expect("the data plan refused a preset spelled custom");
}

/// A RECIPE THE BINDING WALK STEPS PAST COMPOSES NOTHING, and this is the
/// shape that made it matter: `Ingredients` beside `IngredientsBy` is the data
/// planner's "pick one", so nothing validated the choices, and a settings
/// stage that rendered them anyway would read an item handle nobody proved.
/// In this half that is a subtraction below zero on the index; in the Go
/// mirror it is another plan's item name in the player's tooltip.
#[test]
fn a_stepped_past_recipe_composes_no_description() {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("medium", "water", &["water"]);
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![IngredientChoice {
                    value: "water".into(),
                    // THE HANDLE NOBODY ISSUED: a defaulted ItemRef is index
                    // zero, one below this plan's first item.
                    ingredients: vec![Ingredient::of(Default::default(), 1)],
                }],
            }),
            ..Default::default()
        },
    );

    // Ops or a refusal, never a trap. The dropdown carries no composed
    // description, because there was nothing this stage was allowed to read.
    let ops = lib
        .plan_settings(&settings_world())
        .expect("the settings plan refused");
    let dropdown = match &ops[0] {
        crate::op::Op::Extend(proto) => proto.clone(),
        _ => panic!("the first op is not the dropdown"),
    };
    assert_eq!(
        field(&dropdown, "name"),
        Some(Value::string("steelworks-medium"))
    );
    assert_eq!(
        field(&dropdown, "localised_description"),
        None,
        "a description was composed out of a choice nothing validated"
    );

    assert_eq!(
        lib.plan_data(&base_world()).err().as_deref(),
        Some(
            "fkrecipes: the recipe steel-rivet names both Ingredients and IngredientsBy; pick one"
        )
    );
}

/// A text setting pushed straight into a plan, which is the only way to build
/// one whose constructor never ran.
///
/// THE PUBLIC SURFACE HAS NO SUCH DOOR: every text setting a consumer can
/// declare goes through `ingredients_setting` or `packs_setting`, and those
/// install the language table the planners read it with. The guard below would
/// therefore be a branch with no witness, which this repository does not ship,
/// so the witness reaches it from inside the crate.
fn forge_text_setting(lib: &mut Lib, kind: SettingKind, name: &str) -> usize {
    lib.settings.push(SettingDecl {
        kind,
        name: String::from(name),
        legacy: false,
        order: String::new(),
        order_prefix: String::new(),
        def_bool: false,
        def_num: 0.0,
        def_int: 0,
        def_str: String::new(),
        def_ings: vec![Ingredient::named(2, "iron-plate", &[])],
        def_packs: vec![Pack::new("automation-science-pack", 1)],
        spec: NumericSpec::default(),
        values: Vec::new(),
    });
    lib.settings.len()
}

/// THE SEAM'S GUARD. The parser and the renderer are reached through a table
/// the text-setting constructors install, so that a plan which declares no
/// text setting links none of the language; a text setting that arrived
/// without the table has nothing to read it with, and both planners say so
/// rather than dereferencing what is not there.
///
/// The sentence names the GO constructors in both halves, because there is one
/// corpus of messages and it is compared byte for byte.
#[test]
fn a_text_setting_without_the_language_is_refused() {
    let mut lib = Lib::new();
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    let index = forge_text_setting(&mut lib, SettingKind::Ingredients, "rivet-ingredients");
    // Bound to a recipe, so the binding rules pass and the guard is what
    // answers rather than "nothing reads it".
    let handle = IngredientsSettingRef { lib: lib.id, index };
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_from: Some(handle),
            ..Default::default()
        },
    );

    let want = "fkrecipes: the text setting rivet-ingredients was declared without the ingredient language; declare it through IngredientsSetting or PacksSetting";
    assert_eq!(
        lib.plan_settings(&settings_world()).err().as_deref(),
        Some(want)
    );
    assert_eq!(lib.plan_data(&base_world()).err().as_deref(), Some(want));
}

/// The same guard, one table further on: a PACKS setting needs the custom-cost
/// resolver as well, and only the packs constructor installs it. The plan here
/// declares a real ingredients setting first, so the language IS installed and
/// what is missing is the resolver alone.
#[test]
fn a_packs_setting_without_the_custom_cost_resolver_is_refused() {
    let mut lib = Lib::new();
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    let list = lib.ingredients_setting(
        "rivet-ingredients",
        vec![Ingredient::named(2, "steel-plate", &[])],
    );
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_from: Some(list),
            ..Default::default()
        },
    );
    let index = forge_text_setting(&mut lib, SettingKind::Packs, "tips-packs");
    let packs = PacksSettingRef { lib: lib.id, index };
    let count = lib.int_setting("tips-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 15, NumericSpec::between(1.0, 600.0));
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

    let want = "fkrecipes: the text setting tips-packs was declared without the ingredient language; declare it through IngredientsSetting or PacksSetting";
    assert_eq!(
        lib.plan_settings(&settings_world()).err().as_deref(),
        Some(want)
    );
    assert_eq!(lib.plan_data(&base_world()).err().as_deref(), Some(want));
}

/// A TEXT HANDLE THAT NAMES ANOTHER KIND OF SETTING IS REFUSED, not followed.
///
/// The id and the index alone would say this handle is an ingredients setting,
/// and following it would reach the language for a plan the guard read as
/// having no text setting at all: the guard decides on the setting's KIND, so
/// the validator has to decide on the same thing. The two conditions are one
/// condition, which is what the customizer's own design review asked for the
/// first time this class was found.
#[test]
fn an_ingredients_handle_naming_another_kind_is_refused() {
    let mut lib = Lib::new();
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
    let handle = IngredientsSettingRef {
        lib: lib.id,
        index: 1,
    };
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_from: Some(handle),
            ..Default::default()
        },
    );

    let want = "fkrecipes: the recipe steel-rivet reads its ingredients from a setting that this plan never declared";
    assert_eq!(
        lib.plan_settings(&settings_world()).err().as_deref(),
        Some(want)
    );
    assert_eq!(lib.plan_data(&base_world()).err().as_deref(), Some(want));
}

/// The packs twin of [`an_ingredients_handle_naming_another_kind_is_refused`],
/// and the same one condition: following this handle would reach the
/// custom-cost resolver a plan with no packs setting never installed.
#[test]
fn a_packs_handle_naming_another_kind_is_refused() {
    let mut lib = Lib::new();
    lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
    let count = lib.int_setting("tips-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 15, NumericSpec::between(1.0, 600.0));
    let packs = PacksSettingRef {
        lib: lib.id,
        index: 1,
    };
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

    let want = "fkrecipes: the technology hardened-tips reads its science packs from a setting that this plan never declared";
    assert_eq!(
        lib.plan_settings(&settings_world()).err().as_deref(),
        Some(want)
    );
    assert_eq!(lib.plan_data(&base_world()).err().as_deref(), Some(want));
}

/// WHAT THE SEAM IS FOR, asserted on the plan itself: a plan that declares no
/// text setting carries neither table, so nothing it can do reaches the
/// language and a link-time elimination pass keeps no parser, no renderer, no
/// amount formatter and no custom-cost resolver. THOSE FOUR, and not "nothing
/// to keep": the description prose the settings planner composes for a text
/// setting is named from a runtime branch every guest compiles in, so that
/// prose ships whether or not a text setting is declared. Measured, under "The
/// claim gets its adjective" in Fix round 1b of
/// `agents/implementation-notes.md`. The two constructors install what their
/// own readers need and no more.
#[test]
fn a_plan_installs_only_the_tables_its_settings_need() {
    let mut lib = Lib::new();
    let part = lib.legacy_item("bbb-balancer-part", ItemSpec::default());
    let cost = lib.legacy_dropdown_setting_needing_locale(
        "bbb-recipe-cost",
        "vanilla",
        &["vanilla", "cheap"],
        "a",
    );
    lib.legacy_recipe(
        part,
        "bbb-balancer-part",
        RecipeSpec {
            ingredients_by: Some(IngredientChoices {
                setting: cost,
                choices: vec![IngredientChoice {
                    value: "vanilla".into(),
                    ingredients: vec![Ingredient::named(4, "iron-plate", &[])],
                }],
            }),
            ..Default::default()
        },
    );
    lib.plan_settings(&settings_world())
        .expect("the dropdown-only plan refused");
    assert!(
        lib.language.is_none(),
        "a plan with no text setting installed the language"
    );
    assert!(
        lib.custom_cost.is_none(),
        "a plan with no packs setting installed the custom-cost resolver"
    );

    let mut ings = Lib::new();
    ings.ingredients_setting(
        "rivet-ingredients",
        vec![Ingredient::named(2, "iron-plate", &[])],
    );
    assert!(
        ings.language.is_some(),
        "an ingredients setting installed no language"
    );
    assert!(
        ings.custom_cost.is_none(),
        "an ingredients setting installed the custom-cost resolver it cannot reach"
    );

    let mut packs = Lib::new();
    packs.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    assert!(
        packs.language.is_some(),
        "a packs setting installed no language"
    );
    assert!(
        packs.custom_cost.is_some(),
        "a packs setting installed no custom-cost resolver"
    );
}

// ---------------------------------------------------------------------------
// The data stage.
// ---------------------------------------------------------------------------

/// A recipe whose whole list is one text setting. The declared list carries a
/// ladder that resolves and one that does not, so the untouched path shows
/// both halves of the author's tolerance.
fn rivet_plan() -> Lib {
    let mut lib = Lib::new();
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    let list = lib.ingredients_setting(
        "rivet-ingredients",
        vec![
            Ingredient::named(2, "tungsten-plate", &["iron-plate"]),
            Ingredient::named(1, "tungsten-carbide", &["titanium-plate"]),
        ],
    );
    lib.recipe(
        rivet,
        RecipeSpec {
            ingredients_from: Some(list),
            ..Default::default()
        },
    );
    lib
}

/// THE WORD IS THE PRE-EXISTING PATH. A player who never typed gets the
/// author's declared list with its ladders, drops and all, and not one line of
/// log says the text was read: there is nothing to report, because nothing was
/// written.
#[test]
fn an_untouched_text_is_the_declared_list_with_its_ladders() {
    let w = base_world().with_setting("steelworks-rivet-ingredients", Value::string("default"));
    let ops = rivet_plan().plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steel-rivet: none of tungsten-carbide, titanium-plate is present, so the ingredient is dropped",
            r#"extend {type="item", name="steelworks-steel-rivet", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-steel-rivet", enabled=true, ingredients=[{type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-steel-rivet", amount=1}]}"#,
        ],
    );
}

/// A SETTING THAT CANNOT BE READ IS THE WORD TOO, with the line every other
/// bound setting writes. Spaces around it are the parser's business: auto_trim
/// is a GUI behaviour and mod-settings.dat keeps what it was given (measured).
#[test]
fn an_unreadable_text_takes_the_declared_list() {
    let ops = rivet_plan().plan_data(&base_world()).expect("plan refused");
    assert_eq!(
        transcript(&ops)[0],
        "log fkrecipes: the setting steelworks-rivet-ingredients was not readable, so its default applies"
    );

    let padded =
        base_world().with_setting("steelworks-rivet-ingredients", Value::string("  default  "));
    let ops = rivet_plan().plan_data(&padded).expect("plan refused");
    assert_eq!(
        transcript(&ops)[0],
        "log fkrecipes: steel-rivet: none of tungsten-carbide, titanium-plate is present, so the ingredient is dropped",
        "a padded word was not read as the word"
    );
}

/// AN EDITED TEXT IS TAKEN AS WRITTEN, in the order it was typed, with one
/// line recording what was read. Nothing is substituted and no ladder is
/// walked: the ladders are the author's tolerance and this list is the
/// player's instruction.
#[test]
fn an_edited_text_is_emitted_in_the_order_it_was_typed() {
    let w = base_world().with_setting(
        "steelworks-rivet-ingredients",
        Value::string("3 copper-plate, iron-plate x2"),
    );
    let ops = rivet_plan().plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-steel-rivet takes its ingredients from steelworks-rivet-ingredients: 3 copper-plate, 2 iron-plate",
            r#"extend {type="item", name="steelworks-steel-rivet", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-steel-rivet", enabled=true, ingredients=[{type="item", name="copper-plate", amount=3}, {type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-steel-rivet", amount=1}]}"#,
        ],
    );
}

/// A TEXT THE LANGUAGE REFUSES LOADS THE MOD, and this is the headline of the
/// round. The player gets the author's own list WITH ITS LADDERS, drops and
/// all, and one ERROR line carrying the language's sentence VERBATIM: the
/// setting, the entry and the problem, exactly as the corpus pins it, with the
/// shared prefix trimmed off because the line it sits in already opens with
/// one. The stage is the host's to prefix, so nothing here carries one.
///
/// MEASURED (Factorio 2.0.77, build 84539): the refusal this replaces was
/// permanent. The engine rewrites mod-settings.dat on every successful load and
/// on no failed one, the client's error dialog cannot reach the Mod Settings
/// screen, and disabling the mod does not drop its stored settings. See
/// `player_fallback`.
#[test]
fn a_language_refusal_becomes_a_fallback_line() {
    let w = base_world().with_setting(
        "steelworks-rivet-ingredients",
        Value::string("2 iron-plate, 3 unobtainium"),
    );
    let ops = rivet_plan().plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            &with_recipe_tail("log fkrecipes: ERROR: steelworks-rivet-ingredients, entry 2 (\"3 unobtainium\"): no item or fluid is named unobtainium. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."),
            "log fkrecipes: steel-rivet: none of tungsten-carbide, titanium-plate is present, so the ingredient is dropped",
            r#"extend {type="item", name="steelworks-steel-rivet", stack_size=50}"#,
            &(String::from(r#"extend {type="recipe", name="steelworks-steel-rivet", "#)
                + &note_in("steelworks-rivet-ingredients", true)
                + r#"enabled=true, ingredients=[{type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-steel-rivet", amount=1}]}"#),
        ],
    );
}

/// A stored value that is not a string takes the author's list with ONE line
/// saying so. The engine resets a wrong-typed one to the default before any
/// stage runs (measured: "Value must be a string" and exit 0), so this is a
/// hand-edited file; the line is what says so, and it says it without stopping
/// the game.
///
/// THE WHOLE TRANSCRIPT, not just the line. What the fallback LANDS ON is the
/// claim: the recipe has to come out on the author's declared list with its
/// ladders walked and its drop line printed, and a first-line assertion would
/// stay green over a recipe with no ingredients at all. The Go half pins the
/// same shape.
#[test]
fn a_text_setting_holding_something_else_falls_back() {
    let w = base_world().with_setting("steelworks-rivet-ingredients", Value::Num(3.0));
    let ops = rivet_plan().plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            &with_recipe_tail("log fkrecipes: ERROR: steelworks-rivet-ingredients is not text. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."),
            "log fkrecipes: steel-rivet: none of tungsten-carbide, titanium-plate is present, so the ingredient is dropped",
            r#"extend {type="item", name="steelworks-steel-rivet", stack_size=50}"#,
            &(String::from(r#"extend {type="recipe", name="steelworks-steel-rivet", "#)
                + &note_in("steelworks-rivet-ingredients", true)
                + r#"enabled=true, ingredients=[{type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-steel-rivet", amount=1}]}"#),
        ],
    );
}

/// A recipe with presets AND a text: the dropdown chooses, and the text
/// applies under exactly one of its values.
fn quench_plan() -> Lib {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());
    let text = lib.ingredients_setting(
        "quench-ingredients",
        vec![Ingredient::named(2, "steel-plate", &[])],
    );
    lib.recipe(
        plate,
        RecipeSpec {
            category: "crafting-with-fluid".into(),
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![
                    IngredientChoice {
                        value: "water".into(),
                        ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
                    },
                    IngredientChoice {
                        value: "oil".into(),
                        ingredients: vec![Ingredient::named(3, "steel-plate", &[])],
                    },
                ],
            }),
            ingredients_from: Some(text),
            ..Default::default()
        },
    );
    lib
}

/// THE TEXT IS THE SWITCH: anything but the word takes the list over, and the
/// line says which choice was set aside so a player who forgot the field reads
/// why the dropdown did nothing.
#[test]
fn a_text_beside_a_dropdown_takes_the_list_over() {
    let w = base_world()
        .with_setting("steelworks-quench-medium", Value::string("oil"))
        .with_setting(
            "steelworks-quench-ingredients",
            Value::string("1 steel-plate, 0.5 [fluid=water]"),
        );
    let ops = quench_plan().plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-steel-plate takes its ingredients from steelworks-quench-ingredients: 1 steel-plate, 0.5 [fluid=water]; the steelworks-quench-medium choice oil is set aside",
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=1}, {type="fluid", name="water", amount=5.0000000000000000e-1}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
        ],
    );
}

/// A TEXT THE LANGUAGE REFUSES FALLS BACK EXACTLY AS THE WORD BEHAVES: the
/// dropdown decides, and the ERROR line names the field the player fixes. The
/// clause about a choice being set aside must NOT appear, because none was.
#[test]
fn a_refused_text_beside_a_dropdown_falls_back_to_the_dropdown() {
    let w = base_world()
        .with_setting("steelworks-quench-medium", Value::string("oil"))
        .with_setting(
            "steelworks-quench-ingredients",
            Value::string("4 unobtanium"),
        );
    let ops = quench_plan().plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            &with_recipe_tail("log fkrecipes: ERROR: steelworks-quench-ingredients, entry 1 (\"4 unobtanium\"): no item or fluid is named unobtanium. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."),
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            &(String::from(r#"extend {type="recipe", name="steelworks-hardened-steel-plate", "#)
                + &note_in("steelworks-quench-ingredients", true)
                + r#"category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=3}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#),
        ],
    );

    // The word lets the dropdown decide, and a player who never typed hears
    // nothing at all.
    let quiet = base_world()
        .with_setting("steelworks-quench-medium", Value::string("oil"))
        .with_setting("steelworks-quench-ingredients", Value::string("default"));
    let ops = quench_plan().plan_data(&quiet).expect("plan refused");
    assert_eq!(
        transcript(&ops)[0],
        r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
        "an untouched text beside a dropdown said something"
    );
}

/// A STORED VALUE THE DROPDOWN DOES NOT LIST IS REFUSED. The engine resets one
/// to the default before any stage runs (measured), so this is a hand-edited
/// file; what it replaces is a recipe made of nothing with no line to say so.
#[test]
fn a_stored_value_outside_a_dropdown_is_refused() {
    let w = base_world().with_setting("steelworks-quench-medium", Value::string("brine"));
    assert_eq!(
        quench_plan().plan_data(&w).err().as_deref(),
        Some("fkrecipes: steelworks-quench-medium holds \"brine\", which is not one of its values")
    );
}

/// A research cost with a TIER BESIDE IT: the dropdown chooses a source, and
/// the three settings overwrite that source's numbers one field at a time.
fn tips_plan() -> Lib {
    let mut lib = Lib::new();
    let tier =
        lib.dropdown_setting_needing_locale("tips-research-tier", "cheap", &["cheap", "formula"]);
    let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_by: Some(crate::plan::CostChoices {
                setting: tier,
                choices: vec![
                    crate::plan::CostChoice {
                        value: "cheap".into(),
                        sources: strings(&["logistics-2"]),
                    },
                    crate::plan::CostChoice {
                        value: "formula".into(),
                        sources: strings(&["mining-productivity-4"]),
                    },
                ],
                fallback: UnitSpec {
                    count: 200,
                    seconds: 30.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
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
}

/// The world the tier tests read under, with the three fields answered so no
/// unreadable line stands between them and what they draw.
fn tier_world(chosen: &str, count: f64, seconds: f64, packs: &str) -> crate::tests::FixtureWorld {
    base_world()
        .with_setting("steelworks-tips-research-tier", Value::string(chosen))
        .with_setting("steelworks-tips-count", Value::Num(count))
        .with_setting("steelworks-tips-seconds", Value::Num(seconds))
        .with_setting("steelworks-tips-packs", Value::string(packs))
}

/// The unit the `cheap` tier copies out of the fixture world.
const TIER_UNIT: &str = r#"unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}"#;

/// EVERY FIELD AT ITS DEFAULT LEAVES THE TIER ALONE, byte for byte, which is
/// the load a player who never opened the settings screen gets: no cost line,
/// no clause, the source's own unit and the source as the prerequisite.
#[test]
fn every_research_field_at_its_default_leaves_the_tier_alone() {
    let ops = tips_plan()
        .plan_data(&tier_world("cheap", 0.0, 0.0, "default"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[&alloc::format!(
            r#"extend {{type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], {}}}"#,
            TIER_UNIT
        )],
    );
}

/// ALL THREE MOVED IS THE WHOLE COST THE PLAYER'S, and the tier still places
/// the technology: the source that would have paid for it is the rung it hangs
/// off whatever the settings say.
#[test]
fn all_three_research_fields_override_the_tier() {
    let ops = tips_plan()
        .plan_data(&tier_world("cheap", 40.0, 20.0, "2 logistic-science-pack"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 40, time 20, packs 2 logistic-science-pack; the steelworks-tips-research-tier choice cheap supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit={count=40, ingredients=[["logistic-science-pack", 2]], time=20}}"#,
        ],
    );
}

/// ONE FIELD MOVED IS THE ONE THE OLD SHAPE COULD NOT EXPRESS. The count is the
/// player's and the time and the packs are the tier's, in one unit, and the
/// clause says so rather than pretending the whole cost was overridden.
#[test]
fn a_partial_custom_cost_takes_the_rest_from_the_tier() {
    let ops = tips_plan()
        .plan_data(&tier_world("cheap", 40.0, 0.0, "default"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 40, time 30, packs 1 automation-science-pack, 1 logistic-science-pack; the steelworks-tips-research-tier choice cheap supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit={count=40, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}"#,
        ],
    );
}

/// THE TIME ALONE IS THE MIRROR OF THE CASE ABOVE, and it is what says the
/// merge is per field rather than count-shaped: the count stays the tier's
/// number.
#[test]
fn a_time_alone_overrides_the_tier() {
    let ops = tips_plan()
        .plan_data(&tier_world("cheap", 0.0, 45.0, "default"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 200, time 45, packs 1 automation-science-pack, 1 logistic-science-pack; the steelworks-tips-research-tier choice cheap supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=45}}"#,
        ],
    );
}

/// A PACK TEXT ALONE OVERRIDES THE INGREDIENTS AND NOTHING ELSE, so the tier's
/// count and time are what the line reports and what the unit carries.
#[test]
fn a_pack_text_alone_overrides_the_tier() {
    let ops = tips_plan()
        .plan_data(&tier_world("cheap", 0.0, 0.0, "3 automation-science-pack"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 200, time 30, packs 3 automation-science-pack; the steelworks-tips-research-tier choice cheap supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit={count=200, ingredients=[["automation-science-pack", 3]], time=30}}"#,
        ],
    );
}

/// A REFUSED PACK TEXT FALLS BACK EXACTLY AS THE WORD BEHAVES, which beside a
/// tier means the TIER'S packs rather than the setting's declared list: the
/// fallback lands where a player who typed nothing lands.
#[test]
fn a_refused_pack_text_beside_a_tier_takes_the_tiers_packs() {
    let ops = tips_plan()
        .plan_data(&tier_world("cheap", 40.0, 0.0, "2 unobtainium"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: ERROR: steelworks-tips-packs, entry 1 (\"2 unobtainium\"): no science pack is named unobtainium. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.",
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 40, time 30, packs 1 automation-science-pack, 1 logistic-science-pack; the steelworks-tips-research-tier choice cheap supplies what the settings leave at default",
            &(String::from(r#"extend {type="technology", name="steelworks-hardened-tips", "#)
                + &note_in("steelworks-tips-packs", false)
                + r#"prerequisites=["logistics-2"], unit={count=40, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}"#),
        ],
    );
}

/// A NUMBER THE ENGINE WOULD NOT TAKE FALLS BACK TO ITS DECLARED DEFAULT, which
/// beside a tier is 0, which means the tier decides. So a stored value the
/// library cannot use behaves exactly as an untouched field does, and the only
/// thing that changes is the ERROR line.
#[test]
fn a_bad_number_beside_a_tier_leaves_the_tier_deciding() {
    let ops = tips_plan()
        .plan_data(&tier_world("cheap", f64::NAN, 0.0, "default"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: ERROR: steelworks-tips-count holds a value that is not a finite number. The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.",
            &alloc::format!(
                r#"extend {{type="technology", name="steelworks-hardened-tips", {}prerequisites=["logistics-2"], {}}}"#,
                note_in("steelworks-tips-count", false),
                TIER_UNIT
            ),
        ],
    );
}

/// A COUNT THE PLAYER TYPED REPLACES A count_formula, because the engine prices
/// a unit carrying both by the FORMULA and the number would be read by nobody.
/// One line says which setting took it, and everything else the tier carried,
/// including the key no version of this library knows about and the level cap
/// beside the unit, comes through untouched.
#[test]
fn a_count_replaces_the_tiers_count_formula() {
    let ops = tips_plan()
        .plan_data(&tier_world("formula", 40.0, 0.0, "default"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: hardened-tips: steelworks-tips-count replaces the count_formula the formula cost carries",
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 40, time 60, packs 1 automation-science-pack, 1 logistic-science-pack, 1 chemical-science-pack; the steelworks-tips-research-tier choice formula supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["mining-productivity-4"], unit={ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60, count=40}, max_level="infinite"}"#,
        ],
    );
}

/// AND A TIME ALONE LEAVES THE FORMULA WHERE IT IS, which is what says the drop
/// is about the count and not about a player having touched anything at all.
#[test]
fn a_time_beside_a_count_formula_leaves_it_alone() {
    let ops = tips_plan()
        .plan_data(&tier_world("formula", 0.0, 45.0, "default"))
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count by formula, time 45, packs 1 automation-science-pack, 1 logistic-science-pack, 1 chemical-science-pack; the steelworks-tips-research-tier choice formula supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["mining-productivity-4"], unit={count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=45}, max_level="infinite"}"#,
        ],
    );
}

/// THE FALLBACK COST IS A TIER LIKE ANY OTHER. No source in the chosen ladder
/// carries a unit, so the author's own fallback is what the settings write over
/// and what they leave alone; the line saying why comes first, because it is
/// the reason the numbers under it are the fallback's.
#[test]
fn a_setting_overrides_the_fallback_tier() {
    let w = base_world()
        .without_tech("logistics-2")
        .with_setting("steelworks-tips-research-tier", Value::string("cheap"))
        .with_setting("steelworks-tips-count", Value::Num(45.0))
        .with_setting("steelworks-tips-seconds", Value::Num(0.0))
        .with_setting("steelworks-tips-packs", Value::string("default"));
    let ops = tips_plan().plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: hardened-tips: no source for the cheap cost carries a unit, so the fallback cost applies and the technology has no prerequisite",
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 45, time 30, packs 1 automation-science-pack; the steelworks-tips-research-tier choice cheap supplies what the settings leave at default",
            r#"extend {type="technology", name="steelworks-hardened-tips", unit={count=45, time=30, ingredients=[["automation-science-pack", 1]]}}"#,
        ],
    );
}

/// A research cost with NO TIER beside it: the three settings are the whole
/// price, and the word `default` in each of them means that setting's own
/// declaration.
fn chain_plan() -> Lib {
    let mut lib = Lib::new();
    let packs = lib.packs_setting(
        "chain-packs",
        vec![
            Pack::new("automation-science-pack", 1),
            Pack::named(1, "military-science-pack", &["logistic-science-pack"]),
        ],
    );
    let count = lib.int_setting("chain-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("chain-seconds", 15, NumericSpec::between(1.0, 600.0));
    lib.technology(
        "chain-forging",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            after: "steel-processing".into(),
            ..Default::default()
        },
    );
    lib
}

/// CostFrom is Unit with its numbers in the player's hands, so the ORDINARY
/// placement fields place it: the splice and the drop behave as they do
/// anywhere else.
#[test]
fn cost_from_is_placed_by_the_ordinary_fields() {
    let mut lib = Lib::new();
    let packs = lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("chain-count", 20, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("chain-seconds", 10, NumericSpec::between(1.0, 600.0));
    lib.technology(
        "chain-forging",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            after: "steel-processing".into(),
            ..Default::default()
        },
    );

    let w = base_world().with_setting("steelworks-chain-packs", Value::string("default"));
    let ops = lib.plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-chain-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-chain-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 20, time 10, packs 1 automation-science-pack",
            r#"extend {type="technology", name="steelworks-chain-forging", prerequisites=["steel-processing"], unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}"#,
        ],
    );

    // The anchor is gone: dropped and logged, never guessed at.
    let ops = lib
        .plan_data(&w.without_tech("steel-processing"))
        .expect("plan refused");
    assert_eq!(
        transcript(&ops)[3],
        "log fkrecipes: chain-forging: steel-processing is absent, so the prerequisite is dropped"
    );
}

/// A unit whose packs ALL drop is refused by name, whether the packs came from
/// the author's declaration or from the word that stands for it.
#[test]
fn a_pack_text_that_resolves_to_nothing_is_refused() {
    let w = base_world().with_setting("steelworks-chain-packs", Value::string("default"));
    let bare = FixtureWorld {
        tools: Vec::new(),
        ..w
    };
    assert_eq!(
        chain_plan().plan_data(&bare).err().as_deref(),
        Some(packless_refusal(
            "chain-forging",
            &[
                "automation-science-pack",
                "military-science-pack",
                "logistic-science-pack"
            ]
        ))
        .as_deref()
    );
}

/// THE THREE NUMBERS THAT ARRIVE FROM OUTSIDE, and two of them are here: a
/// research count and a research time come back from settings on every load,
/// and nothing about the DECLARATION constrains what a World hands over.
///
/// The declared minima keep the ENGINE from producing one of these (measured:
/// a stored value outside a setting's own bounds is reset to that setting's
/// default rather than clamped), so every case below is a fixture World or a
/// hand-edited file. A NUMBER IS A FIELD THE PLAYER OWNS, so each takes the
/// setting's DECLARED DEFAULT with one line naming the setting that answered,
/// rather than stopping the load: the unit that comes out is count 30, time 15.
#[test]
fn a_research_number_the_world_cannot_answer_falls_back() {
    struct Case {
        name: &'static str,
        // BOTH NUMBERS ARE STATED IN EVERY CASE, so the transcript is the one
        // fallback line and nothing else: a setting left out would draw the
        // unreadable line as well and the case under test would read as two.
        count: f64,
        seconds: f64,
        want: &'static str,
        /// The field the note on the emitted technology names, which is the
        /// FIRST the walk set aside: the count is read before the time, so a
        /// case that spoiled both would still name the count.
        setting: &'static str,
    }

    let cases = [
        Case {
            name: "a count that is not a number",
            count: f64::NAN,
            seconds: 15.0,
            want: "steelworks-chain-count holds a value that is not a finite number",
            setting: "steelworks-chain-count",
        },
        Case {
            name: "a count below one",
            count: 0.0,
            seconds: 15.0,
            want: "steelworks-chain-count holds a research count below 1",
            setting: "steelworks-chain-count",
        },
        Case {
            name: "a time that is not a number",
            count: 30.0,
            seconds: f64::INFINITY,
            want: "steelworks-chain-seconds holds a value that is not a finite number",
            setting: "steelworks-chain-seconds",
        },
        Case {
            name: "a time at zero",
            count: 30.0,
            seconds: 0.0,
            want: "steelworks-chain-seconds holds a research time at or below zero",
            setting: "steelworks-chain-seconds",
        },
    ];

    for c in cases {
        let w = base_world()
            .with_setting("steelworks-chain-packs", Value::string("default"))
            .with_setting("steelworks-chain-count", Value::Num(c.count))
            .with_setting("steelworks-chain-seconds", Value::Num(c.seconds));
        let ops = chain_plan().plan_data(&w).expect("plan refused");
        assert_lines_named(
            &transcript(&ops),
            &[
                &format!("log fkrecipes: ERROR: {}. The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.", c.want),
                "log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 30, time 15, packs 1 automation-science-pack, 1 logistic-science-pack",
                &(String::from(r#"extend {type="technology", name="steelworks-chain-forging", "#)
                    + &note_in(c.setting, false)
                    + r#"prerequisites=["steel-processing"], unit={count=30, time=15, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]]}}"#),
            ],
            c.name,
        );
    }
}

/// TWO DECLARED DEFAULTS WRONG AT ONCE, AND THE COUNT IS THE ONE THAT ANSWERS.
///
/// THIS IS THE ONLY WITNESS TO THAT ORDER. The post-condition walks the count
/// and then the seconds and keeps the first answer; every other test declares
/// at most one bad default, so swapping its two arms changed no sentence
/// anywhere and the order was free to drift between the halves. A world wrong
/// in both places is what pins it. The Go half pins the same plan.
///
/// NOTHING IS TYPED HERE, so the refusal carries no fallback note: both numbers
/// ARE the declared defaults rather than values something fell back onto, and
/// an unreadable setting is not a stored value a player can go and correct.
#[test]
fn refused_cost_numbers_answer_the_count_before_the_seconds() {
    let mut lib = Lib::new();
    let packs = lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
    // Both outside what the engine takes, and both refused by
    // `validate_settings` at the settings stage: `plan_data` reaches them only
    // on its own.
    let count = lib.int_setting(
        "chain-count",
        0,
        NumericSpec {
            min: Some(1.0),
            max: Some(100000.0),
        },
    );
    let seconds = lib.int_setting(
        "chain-seconds",
        0,
        NumericSpec {
            min: Some(1.0),
            max: Some(600.0),
        },
    );
    lib.technology(
        "chain-forging",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            ..Default::default()
        },
    );

    assert_eq!(
        lib.plan_data(&base_world()).err().as_deref(),
        Some("fkrecipes: steelworks-chain-count declares a default research count below 1")
    );
}

/// AND THE DECLARED DEFAULT THE FALLBACK LANDS ON IS STILL HELD TO THE ENGINE'S
/// RULE, which is the author's half of the same pair.
///
/// `validate_settings` refuses a default outside its setting's own bounds at the
/// SETTINGS stage, and the engine runs that stage before the data stage, so this
/// world exists only for a host test that calls `plan_data` on its own. It is a
/// refusal because a declaration is not a typed value, and it is what keeps the
/// invariant that no unit this library emits carries a count the engine refuses.
#[test]
fn a_declared_cost_default_the_engine_would_not_take_is_refused() {
    let plan = || {
        let mut lib = Lib::new();
        let packs = lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
        // A minimum of 1, which `validate_custom_cost` demands, beside a
        // declared default of 0, which `validate_settings` refuses.
        let count = lib.int_setting(
            "chain-count",
            0,
            NumericSpec {
                min: Some(1.0),
                max: Some(100000.0),
            },
        );
        let seconds = lib.int_setting("chain-seconds", 10, NumericSpec::between(1.0, 600.0));
        lib.technology(
            "chain-forging",
            TechSpec {
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
    // The settings stage is where this belongs, and it says so.
    assert_eq!(
        plan().plan_settings(&settings_world()).err().as_deref(),
        Some("fkrecipes: the numeric setting chain-count declares a default outside its own minimum and maximum")
    );

    // And the data stage, reached on its own, refuses rather than emitting a
    // unit the engine would not take. The stored value falls back first, and
    // the sentence is about the DECLARED default it fell back onto: the NaN the
    // setting answered is gone, and the 0 the plan wrote is what is left. The
    // note is the player's half of the same refusal, because a stored value WAS
    // set aside on the way here.
    let w = base_world()
        .with_setting("steelworks-chain-count", Value::Num(f64::NAN))
        .with_setting("steelworks-chain-seconds", Value::Num(10.0))
        .with_setting(
            "steelworks-chain-packs",
            Value::string("1 automation-science-pack"),
        );
    assert_eq!(
        plan().plan_data(&w).err().as_deref(),
        Some(with_fallback_fact(
            "fkrecipes: steelworks-chain-count declares a default research count below 1",
            "steelworks-chain-count"
        ))
        .as_deref()
    );
}

/// ALL THREE FIELDS WRONG AT ONCE, AND ALL THREE ANSWERED, which is a PARITY
/// pin before it is anything else: a custom cost reads three fields the player
/// owns and a hand-edited file can leave every one of them wrong. The two
/// halves used to have to agree on which single sentence came out of such a
/// world; with a line per field there is nothing to choose between, and the
/// ordering that is left is the walk's own, count then seconds then the pack
/// text, which is also the order the cost line names them.
///
/// THE STORED VALUES ARE THE GO HALF'S, BYTE FOR BYTE: the same pack text, the
/// same count, the same seconds, so the three sentences are the same three
/// sentences. The two plans behind them are not identical (this one's settings
/// are named chain-* and it declares two packs), which is why the cost line and
/// the unit differ; what a parity pin has to hold still is the input and the
/// wording, and those are what match.
#[test]
fn every_bad_field_of_a_custom_cost_answers() {
    let w = base_world()
        .with_setting("steelworks-chain-packs", Value::string("2 unobtainium"))
        .with_setting("steelworks-chain-count", Value::Num(0.0))
        .with_setting("steelworks-chain-seconds", Value::Num(f64::NAN));
    let ops = chain_plan().plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: ERROR: steelworks-chain-count holds a research count below 1. The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.",
            "log fkrecipes: ERROR: steelworks-chain-seconds holds a value that is not a finite number. The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.",
            "log fkrecipes: ERROR: steelworks-chain-packs, entry 1 (\"2 unobtainium\"): no science pack is named unobtainium. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.",
            "log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 30, time 15, packs 1 automation-science-pack, 1 logistic-science-pack",
            &(String::from(r#"extend {type="technology", name="steelworks-chain-forging", "#)
                + &note_in("steelworks-chain-count", false)
                + r#"prerequisites=["steel-processing"], unit={count=30, time=15, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]]}}"#),
        ],
    );
}

/// THE TWO SIDES, OVER ONE PLAN THAT IS WRONG ON BOTH AT ONCE. The player's
/// text names nothing the game has and the player's crafting time is at the
/// engine floor; the mod is priced in a science pack this game does not carry.
/// Neither player field stops the load: both fall back to what the author
/// declared and both say so, in the walk's own order (the crafting time is read
/// before the ingredients). What stops the load is the pack, which is the
/// author's own declaration against the modpack.
///
/// THIS IS THE SEPARATION WRITTEN AS ONE TEST, and the claim it holds is the
/// narrow one: a value the player TYPED never introduces a refusal that a
/// player who never typed would not also have hit. It is not "a player can
/// never be refused". The same modpack refuses the same plan with the fields
/// untouched, which is the second world below, and the only difference the
/// typing makes is one added FACT: the log ops never reach the host on a
/// refused load, so the refusal itself is the only place left to say that a
/// stored value was set aside. It says that and nothing else, because the
/// screen the old sentence routed to cannot be reached from the error dialog.
/// See `Resolution::fallback_fact`. The Go half pins the same worlds.
#[test]
fn player_fields_fall_back_while_the_author_channel_still_refuses() {
    let plan = || {
        let mut lib = Lib::new();
        let rivet = lib.item("steel-rivet", ItemSpec::default());
        let forging = lib.double_setting("forging-time", 3.0, NumericSpec::default());
        let list = lib.ingredients_setting(
            "rivet-ingredients",
            vec![Ingredient::named(1, "iron-plate", &[])],
        );
        lib.recipe(
            rivet,
            RecipeSpec {
                craft_time_from: forging,
                ingredients_from: Some(list),
                ..Default::default()
            },
        );
        lib.technology(
            "steel-riveting",
            TechSpec {
                // A pack the game does not have, so the ladder drops it and
                // the unit is left with nothing to pay it with.
                unit: Some(UnitSpec {
                    count: 50,
                    seconds: 15.0,
                    packs: vec![Pack::new("military-science-pack", 1)],
                }),
                ..Default::default()
            },
        );
        lib
    };
    let world = |text: &str, craft_time: f64| {
        base_world()
            .with_setting("steelworks-rivet-ingredients", Value::string(text))
            .with_setting("steelworks-forging-time", Value::Num(craft_time))
    };

    // All three at once: the packs are what stops the load, because the other
    // two are the player's and neither one refuses any more.
    assert_eq!(
        plan()
            .plan_data(&world("2 unobtainium", 0.001))
            .err()
            .as_deref(),
        Some(with_fallback_fact(
            &packless_refusal("steel-riveting", &["military-science-pack"]),
            "steelworks-forging-time"
        ))
        .as_deref()
    );

    // AND THE SAME REFUSAL WITH NOTHING TYPED, which is what makes the added
    // fact a fact about THIS player rather than boilerplate: a player who never
    // opened the settings screen meets the identical modpack problem, and is
    // told only about the mod. The fact names the crafting time rather than the
    // text because the crafting time is read first; the pair is the walk's
    // order, which is the same every run. Neither sentence sends anybody to a
    // screen the error dialog cannot reach.
    assert_eq!(
        plan().plan_data(&base_world()).err().as_deref(),
        Some(packless_refusal(
            "steel-riveting",
            &["military-science-pack"]
        ))
        .as_deref()
    );

    // The same plan with the pack put back loads, and the two player fields are
    // the whole log: the declared list, the declared crafting time, two lines.
    let ok = FixtureWorld {
        tools: strings(&["military-science-pack"]),
        ..world("2 unobtainium", 0.001)
    };
    let ops = plan().plan_data(&ok).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: ERROR: the recipe steel-rivet reads its crafting time from steelworks-forging-time, which answers at or below the engine floor (energy_required can't be <= 0.001). The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.",
            &with_recipe_tail("log fkrecipes: ERROR: steelworks-rivet-ingredients, entry 1 (\"2 unobtainium\"): no item or fluid is named unobtainium. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."),
            r#"extend {type="item", name="steelworks-steel-rivet", stack_size=50}"#,
            &(String::from(r#"extend {type="recipe", name="steelworks-steel-rivet", "#)
                + &note_in("steelworks-forging-time", false)
                + r#"energy_required=3, enabled=true, ingredients=[{type="item", name="iron-plate", amount=1}], results=[{type="item", name="steelworks-steel-rivet", amount=1}]}"#),
            r#"extend {type="technology", name="steelworks-steel-riveting", unit={count=50, time=15, ingredients=[["military-science-pack", 1]]}}"#,
        ],
    );
}

/// AND THE CHANNELS THAT ARE LEFT KEEP THEIR ORDER. A dropdown holding a value
/// it does not offer is carried out of the walk and answered before the packs,
/// which is the same "carried refusal first" rule the merged-amount ceiling
/// rides on. Both are hand-edited files or author declarations, never a
/// player's typing.
///
/// THIS IS WHAT THE OLD ORDERING WITNESS BECAME. The pair it used to hold apart
/// were a language refusal and a check behind it; the language refusal is a
/// fallback now, so the two channels left are these, and without a test over
/// them the carried refusal could be moved below the three checks and the whole
/// suite would stay green. The Go half pins the same two cases.
#[test]
fn a_carried_refusal_is_reported_before_the_checks_behind_it() {
    let plan = || {
        let mut lib = Lib::new();
        let axe = lib.item("steel-axe", ItemSpec::default());
        let style = lib.dropdown_setting_needing_locale("axe-style", "plain", &["plain", "fancy"]);
        lib.recipe(
            axe,
            RecipeSpec {
                name: "steel-axe-forging".into(),
                ingredients_by: Some(IngredientChoices {
                    setting: style,
                    choices: vec![
                        IngredientChoice {
                            value: "plain".into(),
                            ingredients: vec![Ingredient::named(1, "steel-plate", &[])],
                        },
                        IngredientChoice {
                            value: "fancy".into(),
                            ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
                        },
                    ],
                }),
                ..Default::default()
            },
        );
        lib.technology(
            "steel-axes",
            TechSpec {
                unit: Some(UnitSpec {
                    count: 1,
                    seconds: 1.0,
                    packs: vec![Pack::new("military-science-pack", 1)],
                }),
                ..Default::default()
            },
        );
        lib
    };

    // The pack is absent in both cases, so the second channel is armed
    // throughout and only the one in front of it is repaired.
    assert_eq!(
        plan()
            .plan_data(&base_world().with_setting("steelworks-axe-style", Value::string("gilded")))
            .err()
            .as_deref(),
        Some("fkrecipes: steelworks-axe-style holds \"gilded\", which is not one of its values"),
        "the carried refusal did not answer first"
    );
    assert_eq!(
        plan()
            .plan_data(&base_world().with_setting("steelworks-axe-style", Value::string("fancy")))
            .err()
            .as_deref(),
        Some(packless_refusal("steel-axes", &["military-science-pack"])).as_deref(),
        "the packs did not answer once the carried refusal was repaired"
    );
}

/// `default,` IS THE WORD, so nothing was edited. The language tolerates one
/// trailing comma everywhere else, and a player who left one behind is told
/// their text is ignored only if the LANGUAGE would not have read it as the
/// mod's own list: the ignored-text line and the data path ask the same
/// function, so the two cannot disagree about the same bytes.
#[test]
fn a_tolerated_trailing_comma_is_not_an_edit() {
    let ignored = base_world()
        .with_setting("steelworks-quench-medium", Value::string("oil"))
        .with_setting("steelworks-quench-ingredients", Value::string("default,"));
    let ops = quench_plan().plan_data(&ignored).expect("plan refused");
    assert_eq!(
        transcript(&ops)[0],
        r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
        "an untouched text with a trailing comma was reported as an edit"
    );

    // The other half of the same claim: the dropdown's own preset is what the
    // recipe comes out of, with no line saying a typed list was read.
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=3}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
        ],
    );
}

/// A plan whose OWN ITEMS are what the description invites the player to type.
///
/// The declared list is a LADDER naming this plan's own emitted item, which is
/// not how an author names their own item (that is what `Ingredient::of` is
/// for): it is here so the word `default` can show that the ladder still walks
/// the GAME. The overlay is for what the player typed and nothing else.
fn own_item_plan() -> Lib {
    let mut lib = Lib::new();
    lib.item("steel-rivet", ItemSpec::default());
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());
    let list = lib.ingredients_setting(
        "plate-ingredients",
        vec![Ingredient::named(4, "steelworks-steel-rivet", &[])],
    );
    lib.recipe(
        plate,
        RecipeSpec {
            ingredients_from: Some(list),
            ..Default::default()
        },
    );
    lib
}

/// A TEXT MAY NAME THIS PLAN'S OWN ITEMS. MEASURED (2.0.77): a list copied
/// straight out of the setting's own description refused the load with "no
/// item or fluid is named fkrecipes-example-steel-rivet", because the data
/// planner resolves texts before its own items reach data.raw. The one list
/// the description showed was the one list the player could not type.
#[test]
fn an_edited_text_may_name_this_plans_own_items() {
    let w = base_world().with_setting(
        "steelworks-plate-ingredients",
        Value::string("1 steelworks-steel-rivet, 2 iron-plate"),
    );
    let ops = own_item_plan().plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-steel-plate takes its ingredients from steelworks-plate-ingredients: 1 steelworks-steel-rivet, 2 iron-plate",
            r#"extend {type="item", name="steelworks-steel-rivet", stack_size=50}"#,
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steelworks-steel-rivet", amount=1}, {type="item", name="iron-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
        ],
    );
}

/// THE SUGGESTION FOLD SEES THEM TOO, which is what makes the overlay a World
/// rather than a special case in the resolver: a player who typed the
/// description's name with the capitals of a display name is offered the name
/// the game will actually have.
#[test]
fn the_suggestion_fold_offers_a_plans_own_item() {
    let w = base_world().with_setting(
        "steelworks-plate-ingredients",
        Value::string("1 Steelworks-Steel-Rivet"),
    );
    let ops = own_item_plan().plan_data(&w).expect("plan refused");

    // THE SUGGESTION SURVIVES THE FALLBACK, which is the whole value of it: the
    // line the player reads still names the name that is really there.
    assert_eq!(
        transcript(&ops)[0],
        with_recipe_tail("log fkrecipes: ERROR: steelworks-plate-ingredients, entry 1 (\"1 Steelworks-Steel-Rivet\"): no item or fluid is named Steelworks-Steel-Rivet; did you mean steelworks-steel-rivet. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.")
    );
}

/// THE WORD IS UNCHANGED. A declared ladder is the AUTHOR'S tolerance for a
/// modpack and is walked against the game as it stands, so a rung naming this
/// plan's own item is still dropped: the overlay is what a PLAYER'S text is
/// read under and nothing else.
#[test]
fn the_declared_default_path_does_not_see_the_overlay() {
    let w = base_world().with_setting("steelworks-plate-ingredients", Value::string("default"));
    let ops = own_item_plan().plan_data(&w).expect("plan refused");
    assert_eq!(
        transcript(&ops)[0],
        "log fkrecipes: hardened-steel-plate: none of steelworks-steel-rivet is present, so the ingredient is dropped"
    );
}

/// A PACK TEXT NAMING ONE OF THIS PLAN'S ITEMS HEARS THE TRUE ANSWER. The
/// overlay adds items and not tools, because a plan's own items are never
/// tools, so the near-miss sentence is the one about a science pack rather
/// than one about a name the game does not have.
#[test]
fn a_pack_text_naming_a_plan_item_is_told_it_is_an_item() {
    let mut lib = Lib::new();
    lib.item("steel-rivet", ItemSpec::default());
    let packs = lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("chain-count", 20, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("chain-seconds", 10, NumericSpec::between(1.0, 600.0));
    lib.technology(
        "chain-forging",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            ..Default::default()
        },
    );

    let w = base_world().with_setting(
        "steelworks-chain-packs",
        Value::string("1 steelworks-steel-rivet"),
    );
    let ops = lib.plan_data(&w).expect("plan refused");

    assert_eq!(
        transcript(&ops)[2],
        "log fkrecipes: ERROR: steelworks-chain-packs, entry 1 (\"1 steelworks-steel-rivet\"): steelworks-steel-rivet is an item, not a science pack. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."
    );
}

/// THE CATEGORY TRAVELS DOWN THE CUSTOM ARM, and this is the half that says
/// so: the same declared fluid that is refused under a `crafting` recipe is
/// legal under a `crafting-with-fluid` one, because the rule is about the
/// RECIPE the dropdown hands the player rather than about the setting. Without
/// the binding carrying the category, this plan would be refused for a fluid
/// its recipe takes perfectly well.
#[test]
fn a_custom_arms_declared_fluid_is_legal_where_the_recipe_takes_one() {
    let mut lib = Lib::new();
    let medium = lib.dropdown_setting_needing_locale("medium", "water", &["water"]);
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());
    let list = lib.ingredients_setting("parts", vec![Ingredient::fluid(10.0, "water", &[])]);
    lib.recipe(
        plate,
        RecipeSpec {
            category: "crafting-with-fluid".into(),
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![IngredientChoice {
                    value: "water".into(),
                    ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                }],
            }),
            ingredients_from: Some(list),
            ..Default::default()
        },
    );

    lib.plan_settings(&settings_world())
        .expect("the settings plan refused a fluid the recipe takes");
    lib.plan_data(&base_world())
        .expect("the data plan refused a fluid the recipe takes");
}

// ---------------------------------------------------------------------------
// A STORED VALUE WHOSE BYTES ARE NOT TEXT.
//
// fkdata hands both halves the engine's own bytes and rewrites nothing, so a
// stored value can be a sequence a Rust `String` cannot hold; `Value::Bytes`
// is what one arrives as, and these are the surfaces that meet it. The Go
// mirror has no such arm, because a Go `string` IS a byte string, so what is
// pinned here is that both halves take the same DECISION over the same bytes,
// not that the two models look alike.
//
// EVERY ONE OF THEM IS UNREACHABLE THROUGH THE SETTINGS SCREEN (measured: the
// engine resets a stored value that is not one of a dropdown's values, and the
// text field cannot produce invalid bytes), which is exactly why they are
// tested: they exist for a mod-settings.dat somebody edited by hand, and
// nobody will find them by playing.
// ---------------------------------------------------------------------------

/// A TEXT SETTING TAKES THE LANGUAGE'S OWN REFUSAL, not a shape refusal of the
/// planner's. The bytes go to the parser unread, the way a text setting's
/// bytes always do, and the parser's not-text guard answers: this half asks
/// `core::str::from_utf8` where the Go half asks `utf8.ValidString`, and the
/// corpus holds the two to that one sentence. Deciding it here instead would
/// be a second answer to one question, in one half only.
#[test]
fn a_text_setting_holding_bytes_that_are_not_text_is_answered_by_the_language() {
    let w = base_world().with_setting(
        "steelworks-rivet-ingredients",
        Value::bytes(b"2 iron-\xffplate"),
    );
    let ops = rivet_plan().plan_data(&w).expect("plan refused");

    assert_eq!(
        transcript(&ops)[0],
        with_recipe_tail("log fkrecipes: ERROR: steelworks-rivet-ingredients contains characters that are not text; retype the list. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.")
    );
}

/// A DROPDOWN TAKES THE SAME REFUSAL AN UNLISTED VALUE TAKES, because that is
/// what it is: an offered value comes from the author's own source and is
/// text, so no offered value can be these bytes.
///
/// THE QUOTED VALUE IS THE ONE PLACE THE TWO HALVES' SENTENCES DIFFER, and it
/// is a property of the refusal channel rather than of the decision: this
/// half's messages are `String`s bound for `fkdata::raise`, which takes a
/// `&str`, so the bytes are rendered lossily to be quoted; the Go half quotes
/// them raw. The DECISION is identical, and it is the decision a player's load
/// stands on.
#[test]
fn a_dropdown_holding_bytes_that_are_not_text_is_refused() {
    let w = base_world().with_setting("steelworks-quench-medium", Value::bytes(b"br\xffine"));
    assert_eq!(
        quench_plan().plan_data(&w).err().as_deref(),
        Some("fkrecipes: steelworks-quench-medium holds \"br\u{fffd}ine\", which is not one of its values")
    );
}

/// A NUMBER SETTING CANNOT READ ONE, and does not try: bytes are not a number,
/// so the read degrades exactly as an absent setting does, with the one line
/// every unreadable setting writes. It is the same answer the Go half gives
/// for the same reason, where the value's kind is not a number either.
///
/// BOTH NUMERIC READS ARE HERE because they are two pieces of code: a research
/// cost's count and time come through `read_num_setting`, and a recipe's
/// crafting time is read where the recipe is planned.
#[test]
fn a_number_setting_holding_bytes_is_unreadable_and_takes_its_default() {
    let w = base_world()
        .with_setting("steelworks-chain-count", Value::bytes(b"45\xff"))
        .with_setting("steelworks-chain-seconds", Value::bytes(b"12.5\xff"))
        .with_setting("steelworks-chain-packs", Value::string("default"));
    let ops = chain_plan().plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-chain-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-chain-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 30, time 15, packs 1 automation-science-pack, 1 logistic-science-pack",
            r#"extend {type="technology", name="steelworks-chain-forging", prerequisites=["steel-processing"], unit={count=30, time=15, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]]}}"#,
        ],
    );

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
    let w = base_world().with_setting("steelworks-axe-craft-time", Value::bytes(b"4\xff"));
    let ops = lib.plan_data(&w).expect("plan refused");
    assert_eq!(
        transcript(&ops)[0],
        "log fkrecipes: the setting steelworks-axe-craft-time was not readable, so its default applies"
    );
}

/// A TEXT WHOSE BYTES ARE NOT TEXT FALLS BACK BESIDE A DROPDOWN TOO, and the
/// dropdown is what decides: the language's own sentence rides in the ERROR
/// line and the chosen preset is what the recipe comes out of, which is exactly
/// where a player who typed nothing lands.
#[test]
fn a_text_beside_a_dropdown_whose_bytes_are_not_text_falls_back() {
    let w = base_world()
        .with_setting("steelworks-quench-medium", Value::string("oil"))
        .with_setting(
            "steelworks-quench-ingredients",
            Value::bytes(b"2 iron-\xffplate"),
        );
    let ops = quench_plan().plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            &with_recipe_tail("log fkrecipes: ERROR: steelworks-quench-ingredients contains characters that are not text; retype the list. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."),
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            &(String::from(r#"extend {type="recipe", name="steelworks-hardened-steel-plate", "#)
                + &note_in("steelworks-quench-ingredients", true)
                + r#"category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=3}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#),
        ],
    );
}

// ---------------------------------------------------------------------------
// The fallback line itself.
// ---------------------------------------------------------------------------

/// EVERY SENTENCE THIS LAYER BUILDS OPENS WITH THE SHARED PREFIX, EXACTLY ONCE,
/// and this is the half of that property which is not the corpus.
///
/// WHAT EACH ASSERTION CATCHES, because the two are about different failures
/// and neither is "the line looks wrong". `strip_prefix` is applied
/// unconditionally, so a sentence that forgot the prefix produces a perfectly
/// well formed line; what it breaks is the sentence's OTHER use, as a refusal
/// raised on its own, where a message with no library name on it is one nobody
/// can trace. That is the `starts_with` assertion. The second assertion is the
/// opposite mistake: a sentence that carries the prefix TWICE survives a single
/// strip and reaches the player as "fkrecipes: ERROR: fkrecipes: ...".
///
/// EVERY PRODUCER IS HERE BECAUSE EVERY ONE IS A FUNCTION, and the faults are
/// taken from the fault functions rather than written down, so a rule a number
/// can fail with no sentence behind it comes out as an empty string and fails
/// the first assertion. A sentence spelled inline at a call site would be one
/// this test cannot see, and the reviews would have to catch it instead.
#[test]
fn fallback_sentences_carry_the_prefix() {
    use crate::data::{
        count_fault, craft_time_fault, declared_craft_time_problem, declared_number_problem,
        not_text_sentence, player_fallback, seconds_fault, stored_craft_time_problem,
        stored_number_problem, MESSAGE_PREFIX,
    };

    let sentences = [
        ("not text", not_text_sentence("mymod-parts")),
        (
            "a stored count that is not finite",
            stored_number_problem("mymod-count", count_fault(f64::NAN)),
        ),
        (
            "a stored count below 1",
            stored_number_problem("mymod-count", count_fault(0.0)),
        ),
        (
            "a stored time that is not finite",
            stored_number_problem("mymod-seconds", seconds_fault(f64::NAN)),
        ),
        (
            "a stored time at or below zero",
            stored_number_problem("mymod-seconds", seconds_fault(0.0)),
        ),
        (
            "a declared count that is not finite",
            declared_number_problem("mymod-count", count_fault(f64::NAN)),
        ),
        (
            "a declared count below 1",
            declared_number_problem("mymod-count", count_fault(0.0)),
        ),
        (
            "a declared time that is not finite",
            declared_number_problem("mymod-seconds", seconds_fault(f64::NAN)),
        ),
        (
            "a declared time at or below zero",
            declared_number_problem("mymod-seconds", seconds_fault(0.0)),
        ),
        (
            "a stored crafting time that is not finite",
            stored_craft_time_problem("axe", "mymod-craft-time", craft_time_fault(f64::NAN)),
        ),
        (
            "a stored crafting time at the floor",
            stored_craft_time_problem("axe", "mymod-craft-time", craft_time_fault(0.0)),
        ),
        (
            "a declared crafting time that is not finite",
            declared_craft_time_problem("axe", "mymod-craft-time", craft_time_fault(f64::NAN)),
        ),
        (
            "a declared crafting time at the floor",
            declared_craft_time_problem("axe", "mymod-craft-time", craft_time_fault(0.0)),
        ),
    ];
    for (name, text) in sentences {
        assert!(
            text.starts_with(MESSAGE_PREFIX),
            "{}: a sentence the fallback composer quotes does not open with {:?}: {}",
            name,
            MESSAGE_PREFIX,
            text
        );
        let line = player_fallback(&text, "text", "");
        assert!(
            !line.contains(&alloc::format!(
                "{}ERROR: {}",
                MESSAGE_PREFIX,
                MESSAGE_PREFIX
            )),
            "{}: the prefix was not stripped, so the line carries it twice: {}",
            name,
            line
        );
    }
}

/// AND THE TWO FIELD WORDS ARE THE WHOLE SHAPE OF A LINE, written out once so
/// the sentence itself is pinned here as well as inside every transcript that
/// carries one. The Go half holds the same two strings.
#[test]
fn player_fallback_line_shape() {
    use crate::data::{number_fallback, player_fallback, text_fallback};

    assert_eq!(
        player_fallback("fkrecipes: mymod-parts is not text", "text", ""),
        "fkrecipes: ERROR: mymod-parts is not text. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."
    );
    assert_eq!(
        number_fallback("fkrecipes: mymod-count holds a research count below 1"),
        "fkrecipes: ERROR: mymod-count holds a research count below 1. The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart."
    );
    assert_eq!(
        text_fallback("fkrecipes: mymod-parts is not text"),
        player_fallback("fkrecipes: mymod-parts is not text", "text", ""),
        "text_fallback and player_fallback disagree about the field word"
    );
}

/// A RECIPE'S INGREDIENT TEXT IS THE ONE FALLBACK WITH A TAIL, and the tail is
/// the engine's own cost rather than anything this library does. The pack text
/// and the two numbers keep the line byte for byte, which is what says the tail
/// is chosen by the prototype the setting is bound to and not by the message.
/// The Go half holds the same strings.
#[test]
fn a_recipe_text_fallback_names_what_changing_a_recipe_costs() {
    use crate::data::{
        number_fallback, recipe_text_fallback, text_fallback, text_fallback_for, NoteTarget,
    };

    let reason = "fkrecipes: mymod-parts is not text";
    let want = alloc::format!("{} Changing a recipe empties an assembling machine's input slots of anything the new list does not use.", text_fallback(reason));
    assert_eq!(recipe_text_fallback(reason), want);
    assert_eq!(
        text_fallback_for(
            NoteTarget {
                tech: false,
                index: 0
            },
            reason
        ),
        want,
        "a recipe target did not pick the recipe line"
    );
    assert_eq!(
        text_fallback_for(
            NoteTarget {
                tech: true,
                index: 0
            },
            reason
        ),
        text_fallback(reason),
        "a technology target picked up a recipe's tail"
    );
    assert!(
        !number_fallback(reason).contains("assembling machine"),
        "a number fallback carries the recipe tail"
    );
}

/// THE NOTE IS THE OTHER READER OF THE SAME SENTENCE, and the two shapes are
/// written out here so a change to either is a change to this test. The Go half
/// holds the same two strings.
///
/// THE TAIL IS SCOPED BY WHAT MOVED, NOT BY THE PROTOTYPE KIND. A recipe whose
/// CRAFTING TIME fell back gets the head alone, because its ingredient list is
/// byte for byte what it would have been; only a fallback that changes the list
/// itself can empty an assembler. See `Resolution::note_on`, and
/// `player_fields_fall_back_while_the_author_channel_still_refuses` for the recipe that proves it end to
/// end.
#[test]
fn fallback_note_shape() {
    use crate::data::fallback_note;

    let head = "The stored value of mymod-parts could not be used, so this mod's own choice applies instead. The reason is in the log.";
    assert_eq!(fallback_note("mymod-parts", false), head);
    assert_eq!(
        fallback_note("mymod-parts", true),
        alloc::format!("{} Changing a recipe empties an assembling machine's input slots of anything the new list does not use.", head)
    );
}

/// THE NOTE JOINS THE AUTHOR'S OWN DESCRIPTION rather than replacing it, and an
/// ITEM NEVER CARRIES ONE.
///
/// THREE SHAPES AND THE THIRD ONE IS THE DEFAULT, which is what the other
/// transcripts in this file already show: a recipe with no description of its
/// own carries the note alone in the ordinary two-element form. Here the recipe
/// and the technology both declare one, so the emitted description is the
/// author's sentence and then the library's on a line of its own; and the item
/// beside them declares one too and carries it UNCHANGED, because the fallback
/// is about what a recipe makes and what a technology costs and an item
/// prototype is neither. The Go half holds the same transcript.
#[test]
fn a_fallback_note_joins_the_authors_own_description() {
    let mut lib = Lib::new();
    let rivet = lib.item(
        "steel-rivet",
        ItemSpec {
            description: String::from("A small steel rivet."),
            ..Default::default()
        },
    );
    let parts = lib.ingredients_setting(
        "rivet-ingredients",
        vec![Ingredient::named(2, "steel-plate", &[])],
    );
    let packs = lib.packs_setting("rivet-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("rivet-count", 20, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.int_setting("rivet-seconds", 10, NumericSpec::between(1.0, 600.0));
    lib.recipe(
        rivet,
        RecipeSpec {
            name: String::from("steel-rivet-forging"),
            description: String::from("Forged from plate."),
            ingredients_from: Some(parts),
            ..Default::default()
        },
    );
    lib.technology(
        "riveting",
        TechSpec {
            description: String::from("Teaches riveting."),
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            ..Default::default()
        },
    );

    let w = base_world()
        .with_setting(
            "steelworks-rivet-ingredients",
            Value::string("2 unobtainium"),
        )
        .with_setting("steelworks-rivet-packs", Value::string("1 unobtainium"))
        .with_setting("steelworks-rivet-count", Value::Num(20.0))
        .with_setting("steelworks-rivet-seconds", Value::Num(10.0));
    let ops = lib.plan_data(&w).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            &with_recipe_tail("log fkrecipes: ERROR: steelworks-rivet-ingredients, entry 1 (\"2 unobtainium\"): no item or fluid is named unobtainium. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart."),
            "log fkrecipes: ERROR: steelworks-rivet-packs, entry 1 (\"1 unobtainium\"): no science pack is named unobtainium. The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.",
            "log fkrecipes: steelworks-riveting takes its research cost from steelworks-rivet-packs: count 20, time 10, packs 1 automation-science-pack",
            r#"extend {type="item", name="steelworks-steel-rivet", localised_description=["", "A small steel rivet."], stack_size=50}"#,
            &(String::from(r#"extend {type="recipe", name="steelworks-steel-rivet-forging", localised_description=["", "Forged from plate.", "#)
                + &chunked_params(&(String::from("\n") + &crate::data::fallback_note("steelworks-rivet-ingredients", true)))
                + r#"], enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-steel-rivet", amount=1}]}"#),
            &(String::from(r#"extend {type="technology", name="steelworks-riveting", localised_description=["", "Teaches riveting.", "#)
                + &chunked_params(&(String::from("\n") + &crate::data::fallback_note("steelworks-rivet-packs", false)))
                + r#"], unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}"#),
        ],
    );
}

// ---------------------------------------------------------------------------
// The alternatives form: every composed locale reference, and the rule that
// the raw fallback is last.
// ---------------------------------------------------------------------------

/// Every locale section this library composes a key under. Written out rather
/// than derived, because the property being asserted is "these three and
/// nothing else goes out bare", and a fourth section added to the library is a
/// fourth entry somebody has to type here on purpose.
const COMPOSED_LOCALE_SECTIONS: &[&str] = &[
    "mod-setting-description.",
    "string-mod-setting.",
    "technology-name.",
];

/// The section a string names, or `None` when it is ordinary prose.
fn composed_locale_section(s: &str) -> Option<&'static str> {
    COMPOSED_LOCALE_SECTIONS
        .iter()
        .find(|sec| s.starts_with(**sec))
        .copied()
}

/// Whether a value is the one-element table a locale key is referenced
/// through: `{"section.key"}`.
fn is_key_table(v: &Value) -> bool {
    match v {
        Value::Arr(items) => match items.as_slice() {
            [Value::Str(s)] => composed_locale_section(s).is_some(),
            _ => false,
        },
        _ => false,
    }
}

/// Walks one composed value and reports every place the alternatives rule is
/// broken, along with what it saw.
///
/// THE TWO RULES ARE THE TWO MEASURED FACTS. A key table that is not wrapped is
/// a reference that costs the whole tooltip where the game does not define it;
/// and inside a wrapper a plain string is always a SUCCESSFUL alternative, so
/// one anywhere but the last slot short circuits every alternative after it and
/// the key is never consulted, while a wrapper not ending in a plain string
/// falls back onto its last alternative's own `Unknown key: "..."` marker. Both
/// are silent on a headless run, which is why they are held by a source
/// property here rather than by a golden alone.
fn locale_ref_faults(v: &Value, out: &mut Vec<String>) {
    let items = match v {
        Value::Map(pairs) => {
            for (_, val) in pairs {
                locale_ref_faults(val, out);
            }
            return;
        }
        Value::Arr(items) => items,
        _ => return,
    };
    let wrapper = matches!(items.first(), Some(Value::Str(s)) if s == "?");
    if wrapper {
        if items.len() < 3 {
            out.push(alloc::format!(
                "a wrapper offers fewer than two alternatives: {:?}",
                v
            ));
        } else {
            if items[1..items.len() - 1]
                .iter()
                .any(|a| !matches!(a, Value::Arr(_)))
            {
                out.push(alloc::format!(
                    "a raw fallback sits before the last alternative, which short circuits every alternative after it: {:?}",
                    v
                ));
            }
            if !matches!(items.last(), Some(Value::Str(_))) {
                out.push(alloc::format!(
                    "a wrapper does not end in a raw fallback, so a game defining none of its keys renders the last one's Unknown key marker: {:?}",
                    v
                ));
            }
        }
    }
    for (i, item) in items.iter().enumerate() {
        if let Value::Str(s) = item {
            if composed_locale_section(s).is_some() && !(i == 0 && items.len() == 1) {
                out.push(alloc::format!(
                    "the locale key {} is not referenced through a key table: {:?}",
                    s,
                    v
                ));
            }
            continue;
        }
        if is_key_table(item) && !(wrapper && i == 1) {
            out.push(alloc::format!(
                "the composed reference {:?} is a bare key rather than the alternatives form: {:?}",
                item,
                v
            ));
        }
        locale_ref_faults(item, out);
    }
}

/// How many references each section contributed, so a walk that found nothing
/// cannot read as a walk that found nothing wrong.
fn locale_ref_counts(v: &Value, into: &mut [usize; 3]) {
    let items = match v {
        Value::Map(pairs) => {
            for (_, val) in pairs {
                locale_ref_counts(val, into);
            }
            return;
        }
        Value::Arr(items) => items,
        _ => return,
    };
    if let [Value::Str(s)] = items.as_slice() {
        if let Some(sec) = composed_locale_section(s) {
            let i = COMPOSED_LOCALE_SECTIONS
                .iter()
                .position(|c| *c == sec)
                .expect("the section came from the same list");
            into[i] += 1;
        }
    }
    for item in items {
        locale_ref_counts(item, into);
    }
}

/// A plan that composes all three key shapes: a recipe behind an ingredient
/// dropdown with a text setting beside it, and a technology behind a cost
/// dropdown whose presets name technologies the GAME owns.
/// A plan that reaches ALL THREE composed key shapes at the settings stage and
/// carries a literal description at the data stage, which is exactly what
/// `every_composed_locale_reference_is_wrapped` walks.
///
/// IT IS THE SAME FIXTURE AS THE GO HALF'S `everyComposedShape`, declaration for
/// declaration, and the two walks run it against the same two worlds. They were
/// different fixtures until the round that added the description-count witness,
/// and the difference hid the same defect twice: neither plan composed a single
/// data-stage description, so both halves asserted a count of zero over nothing
/// at all.
fn every_composed_shape() -> Lib {
    let mut lib = Lib::new();
    let plate = lib.item("hardened-steel-plate", ItemSpec::default());
    let medium = lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
    let quench = lib.ingredients_setting(
        "quench-ingredients",
        vec![Ingredient::named(2, "steel-plate", &[])],
    );
    let tier =
        lib.dropdown_setting_needing_locale("tips-research-tier", "projectile", &["projectile"]);
    let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tips-count", 0, NumericSpec::between(0.0, 100000.0));
    let seconds = lib.int_setting("tips-seconds", 0, NumericSpec::between(0.0, 600.0));
    lib.recipe(
        plate,
        RecipeSpec {
            ingredients_by: Some(IngredientChoices {
                setting: medium,
                choices: vec![
                    IngredientChoice {
                        value: "water".into(),
                        ingredients: vec![Ingredient::named(2, "steel-plate", &[])],
                    },
                    IngredientChoice {
                        value: "oil".into(),
                        ingredients: vec![Ingredient::named(3, "steel-plate", &[])],
                    },
                ],
            }),
            ingredients_from: Some(quench),
            // A LITERAL DESCRIPTION, because the data stage's half of the
            // wrapper test needs something to walk: without one this plan
            // composes no `localised_description` at all and every assertion
            // over it is vacuous. It is also the shape decision C fixed, plain
            // English with no key, which is what the zero counts assert.
            description: String::from("Quenched in whatever the medium setting says."),
            ..Default::default()
        },
    );
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_by: Some(crate::plan::CostChoices {
                setting: tier,
                choices: vec![crate::plan::CostChoice {
                    value: "projectile".into(),
                    sources: strings(&["logistics-2"]),
                }],
                fallback: UnitSpec {
                    count: 200,
                    seconds: 30.0,
                    packs: vec![Pack::new("automation-science-pack", 1)],
                },
            }),
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
            }),
            description: String::from("Priced from the tier the research setting says."),
            ..Default::default()
        },
    );
    lib
}

/// EVERY COMPOSED LOCALE REFERENCE GOES THROUGH THE ALTERNATIVES FORM, and the
/// raw fallback is LAST in each one.
///
/// THIS IS THE PROPERTY A GOLDEN CANNOT HOLD. A golden pins the shapes somebody
/// thought to write down; this walks everything both stages emit for a plan
/// that exercises all three key shapes and asserts the rule over each. It is
/// also the only reader of the rule that can see a REORDERED wrapper:
/// `["?", "raw", {key}]` renders identically to `["?", {key}, "raw"]` in every
/// dump and in every transcript, because the engine's own dump holds the table
/// verbatim, and differs only on a client, where the raw string wins every time
/// and the key is never consulted.
///
/// BOTH STAGES, because the data stage composes descriptions too. What it must
/// carry is the OPPOSITE property: the prototype notes are English literals
/// with no key at all, and the counts below say so.
#[test]
fn every_composed_locale_reference_is_wrapped() {
    let lib = every_composed_shape();

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    let mut faults = Vec::new();
    let mut counts = [0usize; 3];
    for op in &ops {
        for v in op_values(op) {
            locale_ref_faults(v, &mut faults);
            locale_ref_counts(v, &mut counts);
        }
    }
    assert!(faults.is_empty(), "settings: {}", faults.join("\n"));
    for (i, sec) in COMPOSED_LOCALE_SECTIONS.iter().enumerate() {
        assert!(
            counts[i] > 0,
            "the walk saw no {} reference, so it proves nothing about one",
            sec
        );
    }

    let ops = every_composed_shape()
        .plan_data(&base_world())
        .expect("plan refused");
    let mut faults = Vec::new();
    let mut counts = [0usize; 3];
    let mut described = 0usize;
    for op in &ops {
        for v in op_values(op) {
            locale_ref_faults(v, &mut faults);
            locale_ref_counts(v, &mut counts);
            described += localised_descriptions(v);
        }
    }
    assert!(faults.is_empty(), "data: {}", faults.join("\n"));
    // THE ZERO BELOW IS ONLY WORTH SOMETHING IF THERE WAS SOMETHING TO FIND. A
    // stage that stopped composing descriptions altogether would satisfy every
    // assertion in this half, so the walk says how many it saw first.
    assert!(
        described > 0,
        "the data stage composed no localised_description, so the counts below prove nothing"
    );
    // THE DATA STAGE COMPOSES NO KEY AT ALL, which is the other half of the
    // measurement: an undefined key anywhere in a recipe's composition deletes
    // the whole description, the library's own literal sentences included, so
    // the prototype notes are English literals and this count is zero.
    for (i, sec) in COMPOSED_LOCALE_SECTIONS.iter().enumerate() {
        assert_eq!(
            counts[i], 0,
            "the data stage composed {} {} references; its notes are literal text",
            counts[i], sec
        );
    }
}

/// How many `localised_description` fields a value carries, which is what turns
/// "no bare key was found" into "descriptions were composed and no bare key was
/// in them". Without it the data-stage half of the wrapper test is green over a
/// stage that stopped composing descriptions at all.
pub(crate) fn localised_descriptions(v: &Value) -> usize {
    match v {
        Value::Map(entries) => entries
            .iter()
            .map(|(k, val)| usize::from(k == "localised_description") + localised_descriptions(val))
            .sum(),
        Value::Arr(items) => items.iter().map(localised_descriptions).sum(),
        _ => 0,
    }
}

/// Every value one op carries, so a walk over a plan is a walk over these.
///
/// EXHAUSTIVE ON PURPOSE, with no wildcard arm: a fourth `Op` variant carrying a
/// `Value` would be a shape the wrapper walk silently stopped covering, and the
/// walk's whole claim is that it sees everything both stages emit. `Log` holds a
/// `String` and no value, so it contributes none; a new variant is a build error
/// here, which is where the decision belongs.
fn op_values(op: &crate::op::Op) -> Vec<&Value> {
    match op {
        crate::op::Op::Extend(proto) => vec![proto],
        crate::op::Op::Set(_, value) => vec![value],
        crate::op::Op::Log(_) => Vec::new(),
    }
}
