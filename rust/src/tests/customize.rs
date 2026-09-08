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

/// A composed description carries REAL newlines, which a raw string literal
/// cannot hold and a quoted one would drown in backslashes: the expectations
/// below write `\n` and this puts the character back before comparing.
fn assert_composed(got: &[String], want: &[&str]) {
    let want: Vec<String> = want.iter().map(|w| w.replace("\\n", "\n")).collect();
    let refs: Vec<&str> = want.iter().map(|s| s.as_str()).collect();
    assert_lines(got, &refs);
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
    let seconds = lib.double_setting("tips-seconds", 15.0, NumericSpec::between(0.5, 600.0));
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
                position: Vec::new(),
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");

    assert_composed(
        &transcript(&ops),
        &[
            r#"extend {type="string-setting", name="steelworks-rivet-ingredients", setting_type="startup", default_value="default", order="aa", auto_trim=true, localised_description=["", ["mod-setting-description.steelworks-rivet-ingredients"], "\ndefault: 2 tungsten-plate, 4 steelworks-steel-rivet, 0.5 [fluid=water]"]}"#,
            r#"extend {type="string-setting", name="steelworks-research-packs", setting_type="startup", default_value="default", order="z", auto_trim=true, localised_description=["", ["mod-setting-description.steelworks-research-packs"], "\ndefault: 1 military-science-pack"]}"#,
            r#"extend {type="int-setting", name="steelworks-tips-count", setting_type="startup", default_value=30, order="ac", minimum_value=1, maximum_value=100000}"#,
            r#"extend {type="double-setting", name="steelworks-tips-seconds", setting_type="startup", default_value=15, order="ad", minimum_value=5.0000000000000000e-1, maximum_value=600}"#,
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
            r#"extend {type="string-setting", name="steelworks-rivet-ingredients", setting_type="startup", default_value="default", order="aa", auto_trim=true, localised_description=["", ["mod-setting-description.steelworks-rivet-ingredients"], "\ndefault: none"]}"#,
        ],
    );
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
    let medium =
        lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil", "custom"]);
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
                custom: Some(text),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops)[..1],
        &[
            r#"extend {type="string-setting", name="steelworks-quench-medium", setting_type="startup", default_value="water", order="aa", allowed_values=["water", "oil", "custom"], localised_description=["", ["mod-setting-description.steelworks-quench-medium"], ["", "\n", ["string-mod-setting.steelworks-quench-medium-water"], ": 2 steel-plate, 10 [fluid=water]"], ["", "\n", ["string-mod-setting.steelworks-quench-medium-oil"], ": 3 steel-plate"]]}"#,
        ],
    );
}

/// A RESEARCH preset is a technology whose cost is copied, so what the line
/// says is which one.
#[test]
fn plan_settings_composes_a_cost_dropdown_with_a_custom_arm() {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale(
        "tips-research-tier",
        "projectile",
        &["projectile", "military", "custom"],
    );
    let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tips-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.double_setting("tips-seconds", 15.0, NumericSpec::between(0.5, 600.0));
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
                custom: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                    position: strings(&["military-2"]),
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops)[..1],
        &[
            r#"extend {type="string-setting", name="steelworks-tips-research-tier", setting_type="startup", default_value="projectile", order="aa", allowed_values=["projectile", "military", "custom"], localised_description=["", ["mod-setting-description.steelworks-tips-research-tier"], ["", "\n", ["string-mod-setting.steelworks-tips-research-tier-projectile"], ": cost of tungsten-hardening"], ["", "\n", ["string-mod-setting.steelworks-tips-research-tier-military"], ": cost of logistics-3"]]}"#,
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
    let tier = lib.dropdown_setting_needing_locale("tier", "cheap", &["cheap", "custom"]);
    let packs = lib.packs_setting("tips-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("tips-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.double_setting("tips-seconds", 15.0, NumericSpec::between(0.5, 600.0));
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
                custom: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                    position: strings(&["military-2"]),
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    let ops = lib.plan_settings(&settings_world()).expect("plan refused");
    assert_composed(
        &transcript(&ops)[..1],
        &[
            r#"extend {type="string-setting", name="steelworks-tier", setting_type="startup", default_value="cheap", order="aa", allowed_values=["cheap", "custom"], localised_description=["", ["mod-setting-description.steelworks-tier"], ["", "\n", ["string-mod-setting.steelworks-tier-cheap"], ": the fallback cost"]]}"#,
        ],
    );
}

/// MEASURED (2.0.77): a localised string takes at most 20 parameters and 20
/// levels of nesting, and 21 of either refuses the load naming nothing useful.
/// The consumer's own description key is the first parameter, so 19 presets
/// ride at the top level and the twentieth turns the lot into groups of 19.
#[test]
fn a_composed_description_nests_past_nineteen_presets() {
    let composed = |presets: usize| -> Value {
        let mut lib = Lib::new();
        let mut values: Vec<String> = (0..presets).map(|i| alloc::format!("p{}", i)).collect();
        values.push(String::from("custom"));
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
                    custom: Some(text),
                    ..Default::default()
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

    // Nineteen presets: the key plus nineteen lines is twenty parameters, the
    // measured ceiling, and nothing nests.
    let Value::Arr(flat) = composed(19) else {
        panic!("the description is not a localised string")
    };
    assert_eq!(flat.len(), 21, "nineteen presets did not stay flat");
    let Value::Arr(last) = flat.last().expect("no last parameter") else {
        panic!("the last parameter is not a localised string")
    };
    assert_eq!(
        last.get(1),
        Some(&Value::string("\n")),
        "the last parameter is not a preset line"
    );

    // Twenty: the level keeps the first nineteen parameters and hands the rest
    // to a nested group in the twentieth slot.
    let Value::Arr(nested) = composed(20) else {
        panic!("the description is not a localised string")
    };
    assert_eq!(nested.len(), 21, "the nested form is not one level wide");
    let Value::Arr(group) = nested.last().expect("no last parameter") else {
        panic!("the last parameter is not a localised string")
    };
    // The empty key that concatenates, then the two lines that did not fit.
    assert_eq!(
        group.len(),
        3,
        "the nested group does not hold what the level could not"
    );
    assert!(
        matches!(group[1], Value::Arr(_)),
        "the nested group's member is not a preset line"
    );
}

// ---------------------------------------------------------------------------
// Plan-time refusals.
// ---------------------------------------------------------------------------

/// Everything a binding has to be true of, in both planners: the settings
/// stage renders the same declared list the data stage emits, so a
/// declaration neither can serve is refused by both.
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
            name: "IngredientsFrom beside IngredientsBy",
            data: true,
            build: |l: &mut Lib| {
                let medium = l.dropdown_setting_needing_locale("medium", "water", &["water"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let list = l.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                            }],
                            ..Default::default()
                        }),
                        ingredients_from: Some(list),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-rivet names both IngredientsBy and IngredientsFrom; pick one",
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
            name: "a Custom arm the dropdown does not list",
            data: true,
            build: |l: &mut Lib| {
                let medium = l.dropdown_setting_needing_locale("medium", "water", &["water"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let list = l.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                            }],
                            custom: Some(list),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-rivet names a Custom arm for custom, which the setting steelworks-medium does not offer",
        },
        Case {
            name: "a CustomValue the Choices also cover",
            data: true,
            build: |l: &mut Lib| {
                let medium =
                    l.dropdown_setting_needing_locale("medium", "water", &["water", "oil"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let list = l.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
                l.recipe(
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
                                    value: "oil".into(),
                                    ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                                },
                            ],
                            custom_value: "oil".into(),
                            custom: Some(list),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-rivet gives oil a preset as well as a Custom arm; name the arm's value with CustomValue",
        },
        Case {
            // A VALUE WITH NOTHING BEHIND IT: the dropdown offers custom, no
            // Choice covers it, and no arm answers it, so the player picks it
            // and gets a recipe made of nothing.
            name: "a dropdown listing custom with no Custom arm",
            data: true,
            build: |l: &mut Lib| {
                let medium =
                    l.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                            }],
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting steelworks-medium offers custom, and the recipe steel-rivet names no Custom arm for it",
        },
        Case {
            // The cost twin of the same rule, and of the same exemption below.
            name: "a cost dropdown listing custom with no Custom arm",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a", "custom"]);
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(crate::plan::CostChoices {
                            setting: tier,
                            choices: vec![crate::plan::CostChoice {
                                value: "a".into(),
                                sources: strings(&["logistics-2"]),
                            }],
                            fallback: UnitSpec {
                                count: 200,
                                seconds: 30.0,
                                packs: vec![Pack::new("automation-science-pack", 1)],
                            },
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting steelworks-tier offers custom, and the technology hardened-tips names no Custom arm for it",
        },
        Case {
            name: "a Custom ingredients handle this plan never issued",
            data: true,
            build: |l: &mut Lib| {
                let medium =
                    l.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_by: Some(IngredientChoices {
                            setting: medium,
                            choices: vec![IngredientChoice {
                                value: "water".into(),
                                ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                            }],
                            custom: Some(Default::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe steel-rivet names a Custom ingredients setting that this plan never declared",
        },
        Case {
            name: "CostFrom beside Unit",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
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
                            position: Vec::new(),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips must name exactly one of CostOf, Unit, CostBy or CostFrom",
        },
        Case {
            name: "CostFrom with a Position",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                            position: strings(&["military-2"]),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips names CostFrom with a Position; Position belongs to a Custom arm, and CostFrom is placed by After, Before and AfterTech",
        },
        Case {
            name: "a cost Custom arm with no Position",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a", "custom"]);
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(crate::plan::CostChoices {
                            setting: tier,
                            choices: vec![crate::plan::CostChoice {
                                value: "a".into(),
                                sources: strings(&["logistics-2"]),
                            }],
                            fallback: UnitSpec {
                                count: 200,
                                seconds: 30.0,
                                packs: vec![Pack::new("automation-science-pack", 1)],
                            },
                            custom: Some(CustomCost {
                                packs,
                                count,
                                seconds,
                                position: Vec::new(),
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the technology hardened-tips names a Custom cost arm with no Position; the arm places the technology, so it needs a prerequisite ladder",
        },
        Case {
            name: "a packs handle this plan never issued",
            data: true,
            build: |l: &mut Lib| {
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs: Default::default(),
                            count,
                            seconds,
                            position: Vec::new(),
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
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count: Default::default(),
                            seconds,
                            position: Vec::new(),
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
                            position: Vec::new(),
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
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                            position: Vec::new(),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting count backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
        },
        Case {
            name: "a Seconds setting with no minimum above 0",
            data: true,
            build: |l: &mut Lib| {
                let packs = l.packs_setting("packs", vec![Pack::new("automation-science-pack", 1)]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.double_setting(
                    "seconds",
                    15.0,
                    NumericSpec {
                        min: None,
                        max: Some(60.0),
                    },
                );
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                            position: Vec::new(),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting seconds backs a research time but declares no minimum above 0 (the engine refuses a unit time of 0)",
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
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                            position: Vec::new(),
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
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                            position: Vec::new(),
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
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                            position: Vec::new(),
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
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.0, 60.0));
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_from: Some(CustomCost {
                            packs,
                            count,
                            seconds,
                            position: Vec::new(),
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting seconds backs a research time but declares no minimum above 0 (the engine refuses a unit time of 0)",
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
            name: "a Custom arm's declared fluid the bound recipe cannot take",
            data: true,
            build: |l: &mut Lib| {
                let medium =
                    l.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
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
                            custom: Some(list),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the ingredients setting parts takes the fluid water, and a recipe in the crafting category takes items only",
        },
        Case {
            // ONE DROPDOWN COMPOSES ONE DESCRIPTION, so a second recipe's arm
            // would silently replace the first's preset list.
            name: "a dropdown taking a Custom arm from two recipes",
            data: true,
            build: |l: &mut Lib| {
                let medium =
                    l.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let first = l.ingredients_setting("first", vec![Ingredient::named(1, "iron-plate", &[])]);
                let second = l.ingredients_setting("second", vec![Ingredient::named(2, "iron-plate", &[])]);
                let arm = |text: crate::plan::IngredientsSettingRef| IngredientChoices {
                    setting: medium,
                    choices: vec![IngredientChoice {
                        value: "water".into(),
                        ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                    }],
                    custom: Some(text),
                    ..Default::default()
                };
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_by: Some(arm(first)),
                        ..Default::default()
                    },
                );
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(arm(second)),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting steelworks-medium takes a Custom arm from more than one recipe; one dropdown composes one description",
        },
        Case {
            name: "a dropdown taking a Custom arm from two technologies",
            data: true,
            build: |l: &mut Lib| {
                let tier = l.dropdown_setting_needing_locale("tier", "a", &["a", "custom"]);
                let count = l.int_setting("count", 30, NumericSpec::between(1.0, 100.0));
                let seconds = l.double_setting("seconds", 15.0, NumericSpec::between(0.5, 60.0));
                let first = l.packs_setting("first", vec![Pack::new("automation-science-pack", 1)]);
                let second = l.packs_setting("second", vec![Pack::new("automation-science-pack", 2)]);
                let arm = move |packs: crate::plan::PacksSettingRef| crate::plan::CostChoices {
                    setting: tier,
                    choices: vec![crate::plan::CostChoice {
                        value: "a".into(),
                        sources: strings(&["logistics-2"]),
                    }],
                    fallback: UnitSpec {
                        count: 200,
                        seconds: 30.0,
                        packs: vec![Pack::new("automation-science-pack", 1)],
                    },
                    custom: Some(CustomCost {
                        packs,
                        count,
                        seconds,
                        position: strings(&["military-2"]),
                    }),
                    ..Default::default()
                };
                l.technology(
                    "hardened-tips",
                    TechSpec {
                        cost_by: Some(arm(first)),
                        ..Default::default()
                    },
                );
                l.technology(
                    "hardened-edges",
                    TechSpec {
                        cost_by: Some(arm(second)),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the setting steelworks-tier takes a Custom arm from more than one technology; one dropdown composes one description",
        },
        Case {
            // TWO PROBLEMS, AND THE ARM'S OWN SENTENCE IS THE ANSWER. The
            // two-arm refusal is raised AFTER both walks, so the second arm is
            // still walked and what is wrong with the arm itself is what the
            // author reads; the Go half raises in exactly this order, and this
            // case is the pin that keeps the two halves saying the same thing
            // about the same plan.
            name: "a second Custom arm that is itself ill formed",
            data: true,
            build: |l: &mut Lib| {
                let medium =
                    l.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
                let rivet = l.item("steel-rivet", ItemSpec::default());
                let plate = l.item("hardened-steel-plate", ItemSpec::default());
                let first =
                    l.ingredients_setting("first", vec![Ingredient::named(1, "iron-plate", &[])]);
                let second =
                    l.ingredients_setting("second", vec![Ingredient::named(2, "iron-plate", &[])]);
                let arm =
                    |text: crate::plan::IngredientsSettingRef, value: &str| IngredientChoices {
                        setting: medium,
                        choices: vec![IngredientChoice {
                            value: "water".into(),
                            ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                        }],
                        custom_value: value.into(),
                        custom: Some(text),
                    };
                l.recipe(
                    rivet,
                    RecipeSpec {
                        ingredients_by: Some(arm(first, "")),
                        ..Default::default()
                    },
                );
                l.recipe(
                    plate,
                    RecipeSpec {
                        ingredients_by: Some(arm(second, "handmade")),
                        ..Default::default()
                    },
                );
            },
            want: "fkrecipes: the recipe hardened-steel-plate names a Custom arm for handmade, which the setting steelworks-medium does not offer",
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
                ..Default::default()
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
                ..Default::default()
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
    let medium = lib.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
    let rivet = lib.item("steel-rivet", ItemSpec::default());
    let text = lib.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
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
                custom: Some(text),
                ..Default::default()
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
    let seconds = lib.double_setting("tips-seconds", 15.0, NumericSpec::between(0.5, 600.0));
    lib.technology(
        "hardened-tips",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
                position: Vec::new(),
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
    let seconds = lib.double_setting("tips-seconds", 15.0, NumericSpec::between(0.5, 600.0));
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
                position: Vec::new(),
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
/// language and a link-time elimination pass has nothing to keep. The two
/// constructors install what their own readers need and no more.
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
                ..Default::default()
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

/// The language's refusal is raised VERBATIM: it names the setting, the entry
/// and the problem, and there is nothing this layer can add to it. The stage
/// is the host's to prefix, so nothing here carries one.
#[test]
fn a_language_refusal_is_raised_as_it_was_written() {
    let w = base_world().with_setting(
        "steelworks-rivet-ingredients",
        Value::string("2 iron-plate, 3 unobtainium"),
    );
    assert_eq!(
        rivet_plan().plan_data(&w).err().as_deref(),
        Some("fkrecipes: steelworks-rivet-ingredients, entry 2 (\"3 unobtainium\"): no item or fluid is named unobtainium")
    );
}

/// A stored value that is not a string. The engine resets a wrong-typed one to
/// the default before any stage runs (measured: "Value must be a string" and
/// exit 0), so this is a hand-edited file and guessing is not on the table.
#[test]
fn a_text_setting_holding_something_else_is_refused() {
    let w = base_world().with_setting("steelworks-rivet-ingredients", Value::Num(3.0));
    assert_eq!(
        rivet_plan().plan_data(&w).err().as_deref(),
        Some("fkrecipes: steelworks-rivet-ingredients is not text")
    );
}

/// A recipe with presets AND a text: the dropdown chooses, and the text
/// applies under exactly one of its values.
fn quench_plan() -> Lib {
    let mut lib = Lib::new();
    let medium =
        lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil", "custom"]);
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
                custom: Some(text),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    lib
}

/// The custom value selects the text; every other value selects its preset.
#[test]
fn the_custom_value_selects_the_text() {
    let w = base_world()
        .with_setting("steelworks-quench-medium", Value::string("custom"))
        .with_setting(
            "steelworks-quench-ingredients",
            Value::string("1 steel-plate, 0.5 [fluid=water]"),
        );
    let ops = quench_plan().plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-steel-plate takes its ingredients from steelworks-quench-ingredients: 1 steel-plate, 0.5 [fluid=water]",
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=1}, {type="fluid", name="water", amount=5.0000000000000000e-1}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
        ],
    );
}

/// A TEXT NOBODY IS READING SAYS SO. The player typed a list and left the
/// dropdown on a preset; silence there is the field report "my ingredients did
/// nothing". The line is written before the preset applies, and the text is
/// LOOKED at rather than parsed, so a list that would have been refused is
/// still only a line.
#[test]
fn an_edited_text_under_a_preset_is_ignored_out_loud() {
    let w = base_world()
        .with_setting("steelworks-quench-medium", Value::string("oil"))
        .with_setting(
            "steelworks-quench-ingredients",
            Value::string("nothing the game has"),
        );
    let ops = quench_plan().plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-quench-ingredients is edited, but steelworks-quench-medium is not on custom, so the text is ignored",
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=3}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
        ],
    );

    // The word is not an edit, so a player who never typed hears nothing.
    let quiet = base_world()
        .with_setting("steelworks-quench-medium", Value::string("oil"))
        .with_setting("steelworks-quench-ingredients", Value::string("default"));
    let ops = quench_plan().plan_data(&quiet).expect("plan refused");
    assert_eq!(
        transcript(&ops)[0],
        r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
        "an untouched text under a preset said something"
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

/// A technology priced out of three settings, with a ladder that says where it
/// hangs.
fn tips_plan(position: &[&str]) -> Lib {
    let mut lib = Lib::new();
    let tier = lib.dropdown_setting_needing_locale(
        "tips-research-tier",
        "projectile",
        &["projectile", "custom"],
    );
    let packs = lib.packs_setting(
        "tips-packs",
        vec![
            Pack::new("automation-science-pack", 1),
            Pack::named(1, "military-science-pack", &["logistic-science-pack"]),
        ],
    );
    let count = lib.int_setting("tips-count", 30, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.double_setting("tips-seconds", 15.0, NumericSpec::between(0.5, 600.0));
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
                custom: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                    position: strings(position),
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    lib
}

/// THE UNIT IS THE SHORT TUPLE FORM, count and time from their settings and
/// the packs from the text; the prerequisite comes from the ladder, because
/// there is no source technology to take it from.
#[test]
fn a_custom_research_cost_is_read_from_its_settings() {
    let w = base_world()
        .with_setting("steelworks-tips-research-tier", Value::string("custom"))
        .with_setting("steelworks-tips-count", Value::Num(45.0))
        .with_setting("steelworks-tips-seconds", Value::Num(12.5))
        .with_setting(
            "steelworks-tips-packs",
            Value::string("1 automation-science-pack, 2 logistic-science-pack"),
        );
    let ops = tips_plan(&["mining-productivity-4", "logistics"])
        .plan_data(&w)
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 45, time 12.5, packs 1 automation-science-pack, 2 logistic-science-pack",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["mining-productivity-4"], unit={count=45, time=1.2500000000000000e1, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 2]]}}"#,
        ],
    );
}

/// The word walks the AUTHOR'S ladders, and a pack the game does not have is
/// dropped with its line exactly as a hand-rolled unit's is.
#[test]
fn an_untouched_pack_text_walks_the_declared_ladders() {
    let w = base_world()
        .with_setting("steelworks-tips-research-tier", Value::string("custom"))
        .with_setting("steelworks-tips-packs", Value::string("default"));
    let ops = tips_plan(&["logistics"])
        .plan_data(&w)
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tips-count was not readable, so its default applies",
            "log fkrecipes: the setting steelworks-tips-seconds was not readable, so its default applies",
            "log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 30, time 15, packs 1 automation-science-pack, 1 logistic-science-pack",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics"], unit={count=30, time=15, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]]}}"#,
        ],
    );
}

/// A ladder that finds nothing places the technology nowhere, and says so.
#[test]
fn a_position_ladder_that_finds_nothing_drops_the_prerequisite() {
    let w = base_world()
        .with_setting("steelworks-tips-research-tier", Value::string("custom"))
        .with_setting("steelworks-tips-packs", Value::string("default"));
    let ops = tips_plan(&["military-2", "military"])
        .plan_data(&w)
        .expect("plan refused");
    let lines = transcript(&ops);

    assert_eq!(
        lines[3],
        "log fkrecipes: hardened-tips: none of military-2, military is present, so the technology has no prerequisite"
    );
    assert!(
        !lines[4].contains("prerequisites"),
        "the technology was placed anyway: {}",
        lines[4]
    );
}

/// The text under a cost dropdown is ignored on a preset, the same way an
/// ingredient text is, and the preset's own source is what pays.
#[test]
fn a_cost_text_under_a_preset_is_ignored_out_loud() {
    let w = base_world().with_setting(
        "steelworks-tips-packs",
        Value::string("1 automation-science-pack"),
    );
    let ops = tips_plan(&["logistics"])
        .plan_data(&w)
        .expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies",
            "log fkrecipes: steelworks-tips-packs is edited, but steelworks-tips-research-tier is not on custom, so the text is ignored",
            r#"extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}"#,
        ],
    );
}

/// CostFrom is Unit with its numbers in the player's hands, so the ORDINARY
/// placement fields place it: the splice and the drop behave as they do
/// anywhere else.
#[test]
fn cost_from_is_placed_by_the_ordinary_fields() {
    let mut lib = Lib::new();
    let packs = lib.packs_setting("chain-packs", vec![Pack::new("automation-science-pack", 1)]);
    let count = lib.int_setting("chain-count", 20, NumericSpec::between(1.0, 100000.0));
    let seconds = lib.double_setting("chain-seconds", 10.0, NumericSpec::between(0.5, 600.0));
    lib.technology(
        "chain-forging",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
                position: Vec::new(),
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
    let w = base_world()
        .with_setting("steelworks-tips-research-tier", Value::string("custom"))
        .with_setting("steelworks-tips-packs", Value::string("default"));
    let bare = FixtureWorld {
        tools: Vec::new(),
        ..w
    };
    assert_eq!(
        tips_plan(&["logistics"]).plan_data(&bare).err().as_deref(),
        Some("fkrecipes: the technology hardened-tips has no science pack the game has; research takes at least one")
    );
}

/// THE THREE NUMBERS THAT ARRIVE FROM OUTSIDE, and two of them are here: a
/// research count and a research time come back from settings on every load,
/// and nothing about the DECLARATION constrains what a World hands over.
///
/// The declared minima keep the ENGINE from producing one of these (measured:
/// a stored value outside a setting's own bounds is reset to that setting's
/// default rather than clamped), so every case below is a fixture World or a
/// hand-edited file. Each is refused by the SETTING that answered, because the
/// technology's own declaration is fine.
#[test]
fn a_research_number_the_world_cannot_answer_is_refused() {
    struct Case {
        name: &'static str,
        setting: &'static str,
        held: f64,
        want: &'static str,
    }

    let cases = [
        Case {
            name: "a count that is not a number",
            setting: "steelworks-tips-count",
            held: f64::NAN,
            want: "fkrecipes: steelworks-tips-count holds a value that is not a finite number",
        },
        Case {
            name: "a count below one",
            setting: "steelworks-tips-count",
            held: 0.0,
            want: "fkrecipes: steelworks-tips-count holds a research count below 1",
        },
        Case {
            name: "a time that is not a number",
            setting: "steelworks-tips-seconds",
            held: f64::INFINITY,
            want: "fkrecipes: steelworks-tips-seconds holds a value that is not a finite number",
        },
        Case {
            name: "a time at zero",
            setting: "steelworks-tips-seconds",
            held: 0.0,
            want: "fkrecipes: steelworks-tips-seconds holds a research time at or below zero",
        },
    ];

    for c in cases {
        let w = base_world()
            .with_setting("steelworks-tips-research-tier", Value::string("custom"))
            .with_setting("steelworks-tips-packs", Value::string("default"))
            .with_setting(c.setting, Value::Num(c.held));
        assert_eq!(
            tips_plan(&["logistics"]).plan_data(&w).err().as_deref(),
            Some(c.want),
            "{}",
            c.name
        );
    }
}

/// THE REFUSAL CHANNELS IN THEIR FIXED ORDER. Resolution is where the PLAYER'S
/// OWN TEXT is read, so a list this library cannot read is the earliest thing
/// the pass met and it is reported ahead of every check behind it; the
/// crafting-time floor comes next, and the unit with no pack left after that.
/// One plan carries all three problems and loses them one at a time.
#[test]
fn a_resolution_refusal_is_reported_before_the_checks_behind_it() {
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

    // All three at once: the language's refusal is the one reported.
    assert_eq!(
        plan().plan_data(&world("2 unobtainium", 0.001)).err().as_deref(),
        Some("fkrecipes: steelworks-rivet-ingredients, entry 1 (\"2 unobtainium\"): no item or fluid is named unobtainium")
    );
    // The text fixed: the crafting time is next.
    assert_eq!(
        plan().plan_data(&world("2 iron-plate", 0.001)).err().as_deref(),
        Some("fkrecipes: the recipe steel-rivet reads its crafting time from steelworks-forging-time, which answers at or below the engine floor (energy_required can't be <= 0.001)")
    );
    // And the crafting time fixed: the packs, which is the last of the three.
    assert_eq!(
        plan().plan_data(&world("2 iron-plate", 2.5)).err().as_deref(),
        Some("fkrecipes: the technology steel-riveting has no science pack the game has; research takes at least one")
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

    // The other half of the same claim: under the custom value those bytes
    // resolve to the declared list, with no line saying a list was read.
    let read = base_world()
        .with_setting("steelworks-quench-medium", Value::string("custom"))
        .with_setting("steelworks-quench-ingredients", Value::string("default,"));
    let ops = quench_plan().plan_data(&read).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}"#,
            r#"extend {type="recipe", name="steelworks-hardened-steel-plate", category="crafting-with-fluid", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}"#,
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
    assert_eq!(
        own_item_plan().plan_data(&w).err().as_deref(),
        Some("fkrecipes: steelworks-plate-ingredients, entry 1 (\"1 Steelworks-Steel-Rivet\"): no item or fluid is named Steelworks-Steel-Rivet; did you mean steelworks-steel-rivet")
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
    let seconds = lib.double_setting("chain-seconds", 10.0, NumericSpec::between(0.5, 600.0));
    lib.technology(
        "chain-forging",
        TechSpec {
            cost_from: Some(CustomCost {
                packs,
                count,
                seconds,
                position: Vec::new(),
            }),
            ..Default::default()
        },
    );

    let w = base_world().with_setting(
        "steelworks-chain-packs",
        Value::string("1 steelworks-steel-rivet"),
    );
    assert_eq!(
        lib.plan_data(&w).err().as_deref(),
        Some("fkrecipes: steelworks-chain-packs, entry 1 (\"1 steelworks-steel-rivet\"): steelworks-steel-rivet is an item, not a science pack")
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
    let medium = lib.dropdown_setting_needing_locale("medium", "water", &["water", "custom"]);
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
                custom: Some(list),
                ..Default::default()
            }),
            ..Default::default()
        },
    );

    lib.plan_settings(&settings_world())
        .expect("the settings plan refused a fluid the recipe takes");
    lib.plan_data(&base_world())
        .expect("the data plan refused a fluid the recipe takes");
}
