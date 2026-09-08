//! The Rust size-measurement guest: a plan shaped like BetterBeltBalancer's
//! before its customizer round, and NOT a mirror arm. It is the twin of
//! go/examples/notext, declaration for declaration.
//!
//! Two legacy dropdowns drive ingredients_by and cost_by over legacy
//! prototypes, and no text setting is declared anywhere, so a build of this
//! guest says whether a consumer who never calls ingredients_setting or
//! packs_setting ships the ingredient language.
//!
//! ```text
//! cargo build --release --target wasm32-unknown-unknown
//! fklua mod --data-module notext.wasm --name better-belt-balancer ...
//! ```
#![cfg_attr(target_family = "wasm", no_std)]

#[cfg(target_family = "wasm")]
extern crate alloc;

#[cfg(target_family = "wasm")]
mod guest {
    use alloc::string::String;
    use alloc::vec;
    use fkrecipes::{
        CostChoice, CostChoices, Ingredient, IngredientChoice, IngredientChoices, ItemSpec, Lib,
        Pack, RecipeSpec, TechSpec, UnitSpec,
    };

    fn plan() -> Lib {
        let mut lib = Lib::new();

        let recipe_cost = lib.legacy_dropdown_setting_needing_locale(
            "bbb-recipe-cost",
            "vanilla",
            &[
                "vanilla",
                "cheap",
                "belt-fast",
                "belt-express",
                "splitter",
                "splitter-express",
            ],
            "a",
        );
        let tech_cost = lib.legacy_dropdown_setting_needing_locale(
            "bbb-tech-cost",
            "logistics",
            &["logistics", "logistics-2", "logistics-3"],
            "b",
        );

        let part = lib.legacy_item(
            "bbb-balancer-part",
            ItemSpec {
                icon: String::from("__better-belt-balancer__/graphics/icons/balancer-part.png"),
                icon_size: 64,
                stack_size: 50,
                subgroup: String::from("belt"),
                order: String::from("c[splitter]-y[bbb-balancer]"),
                ..Default::default()
            },
        );
        let recipe = lib.legacy_recipe(
            part,
            "bbb-balancer-part",
            RecipeSpec {
                craft_time: 1.0,
                order: String::from("c[splitter]-y[bbb-balancer]"),
                ingredients_by: Some(IngredientChoices {
                    setting: recipe_cost,
                    choices: vec![
                        IngredientChoice {
                            value: String::from("vanilla"),
                            ingredients: vec![
                                Ingredient::named(4, "iron-plate", &[]),
                                Ingredient::named(2, "iron-gear-wheel", &[]),
                                Ingredient::named(2, "transport-belt", &[]),
                            ],
                        },
                        IngredientChoice {
                            value: String::from("cheap"),
                            ingredients: vec![
                                Ingredient::named(2, "iron-plate", &[]),
                                Ingredient::named(1, "transport-belt", &[]),
                            ],
                        },
                        IngredientChoice {
                            value: String::from("belt-fast"),
                            ingredients: vec![
                                Ingredient::named(2, "iron-plate", &[]),
                                Ingredient::named(1, "fast-transport-belt", &[]),
                            ],
                        },
                        IngredientChoice {
                            value: String::from("belt-express"),
                            ingredients: vec![
                                Ingredient::named(2, "iron-plate", &[]),
                                Ingredient::named(1, "express-transport-belt", &[]),
                            ],
                        },
                        IngredientChoice {
                            value: String::from("splitter"),
                            ingredients: vec![
                                Ingredient::named(3, "iron-plate", &[]),
                                Ingredient::named(1, "splitter", &[]),
                            ],
                        },
                        IngredientChoice {
                            value: String::from("splitter-express"),
                            ingredients: vec![
                                Ingredient::named(3, "iron-plate", &[]),
                                Ingredient::named(
                                    1,
                                    "express-splitter",
                                    &["fast-splitter", "splitter"],
                                ),
                            ],
                        },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        lib.legacy_technology(
            "bbb-balancer",
            TechSpec {
                icon: String::from("__better-belt-balancer__/graphics/icons/balancer-part.png"),
                icon_size: 64,
                order: String::from("a-b-bbb"),
                unlocks: vec![recipe],
                cost_by: Some(CostChoices {
                    setting: tech_cost,
                    choices: vec![
                        CostChoice {
                            value: String::from("logistics"),
                            sources: vec![String::from("logistics")],
                        },
                        CostChoice {
                            value: String::from("logistics-2"),
                            sources: vec![String::from("logistics-2"), String::from("logistics")],
                        },
                        CostChoice {
                            value: String::from("logistics-3"),
                            sources: vec![
                                String::from("logistics-3"),
                                String::from("logistics-2"),
                                String::from("logistics"),
                            ],
                        },
                    ],
                    fallback: UnitSpec {
                        count: 20,
                        seconds: 15.0,
                        packs: vec![Pack::new("automation-science-pack", 1)],
                    },
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        lib
    }

    #[no_mangle]
    pub extern "C" fn fk_settings() {
        plan().emit();
    }

    #[no_mangle]
    pub extern "C" fn fk_data() {
        plan().emit();
    }
}
