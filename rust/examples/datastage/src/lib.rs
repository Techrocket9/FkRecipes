//! The Rust example guest: a small steelworks expansion that exercises every
//! verb this library has, and the Rust arm of the mirror harness.
//!
//! IT IS THE GO MIRROR'S TWIN. go/examples/datastage is the same declarations
//! in the same order, and scripts/run-mirror.sh packages both, runs each mod's
//! settings and data stages under lua52f against one strict stand-in, and
//! compares the two transcripts byte for byte. A difference between the two
//! files may only be the language.
//!
//! ```text
//! cargo build --release --target wasm32-unknown-unknown -p datastage
//! fklua mod --data-module datastage.wasm --name fkrecipes-example ...
//! ```
//!
//! The body is wasm-only: fkrecipes' emit half and fkdata itself exist only
//! there, so a host build of this workspace member stays empty.
#![cfg_attr(target_family = "wasm", no_std)]

#[cfg(target_family = "wasm")]
extern crate alloc;

#[cfg(target_family = "wasm")]
mod guest {
    use alloc::string::String;
    use alloc::vec;
    use fkrecipes::{
        kv, CostChoice, CostChoices, Ingredient, IngredientChoice, IngredientChoices, ItemSpec,
        Lib, NumericSpec, Pack, RecipeSpec, TechSpec, UnitSpec, Value,
    };

    /// Declares the whole mod. Both stages call it, because the module is
    /// instantiated fresh per stage and nothing carries across: the settings
    /// stage needs the recipes to know which double setting backs a crafting
    /// time, and the data stage needs the settings to read them back.
    fn plan() -> Lib {
        let mut lib = Lib::new();

        let hardened = lib.bool_setting("hardened-tools", true);
        lib.int_setting("rivet-batch", 4, NumericSpec::between(1.0, 20.0));
        // A maximum and no minimum: the library generates the floor-safe
        // minimum beside the declared ceiling, so both bounds are in the
        // golden and the single-bound spec is exercised.
        let forging = lib.double_setting(
            "forging-time",
            3.0,
            NumericSpec {
                min: None,
                max: Some(120.0),
            },
        );
        let medium =
            lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
        let bonuses = lib.bool_setting("bonus-research", true);
        // Declared LAST on purpose: the generated order is derived from the
        // declaration index, so a new setting at the end leaves every existing
        // order alone.
        let tier = lib.dropdown_setting_needing_locale(
            "tips-research-tier",
            "projectile",
            &["projectile", "military"],
        );
        // A FLOOR AND NO CEILING, the one NumericSpec arm the goldens did not
        // carry. Organic here: a longer hold keeps tempering, so there is
        // nothing to cap, but below half a second the plate never reaches
        // temperature. Bound as a crafting time too, which is what shows that
        // a DECLARED minimum stands rather than being replaced by the
        // generated floor-safe one: the generated minimum fills in only where
        // the consumer named none.
        let tempering = lib.double_setting(
            "tempering-hold",
            1.5,
            NumericSpec {
                min: Some(0.5),
                max: None,
            },
        );

        let plate = lib.item(
            "hardened-steel-plate",
            ItemSpec {
                icon: String::from("__fkrecipes-example__/graphics/icons/hardened-steel-plate.png"),
                icon_size: 64,
                stack_size: 100,
                subgroup: String::new(),
                display_name: String::from("Hardened steel plate"),
                description: String::from("Quenched and tempered, for tools that keep an edge."),
                ..Default::default()
            },
        );
        let rivet = lib.item(
            "steel-rivet",
            ItemSpec {
                icon: String::from("__fkrecipes-example__/graphics/icons/steel-rivet.png"),
                icon_size: 64,
                stack_size: 200,
                subgroup: String::from("intermediate-product"),
                display_name: String::from("Steel rivet"),
                description: String::new(),
                // A sort key, so the rivets sit beside the plate they fasten
                // rather than wherever the engine's name ordering puts them.
                order: String::from("b[steelworks]-a[rivet]"),
                ..Default::default()
            },
        );

        let rivets = lib.recipe(
            rivet,
            RecipeSpec {
                craft_time: 0.5,
                result_count: 4,
                ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                display_name: String::from("Steel rivets"),
                // A REAL 2.0 RECIPE FIELD this library has no slot for, passed
                // through verbatim. That is what extra is: the library emits
                // what it knows and gets out of the way for the rest, rather
                // than growing a field per prototype property the engine has.
                extra: vec![kv("allow_productivity", Value::Bool(true))],
                ..Default::default()
            },
        );
        let plates = lib.recipe(
            plate,
            RecipeSpec {
                craft_time_from: forging,
                // The player picks what the plate is quenched in, and each
                // medium is a whole ingredient plan rather than one
                // substituted line.
                ingredients_by: Some(IngredientChoices {
                    setting: medium,
                    choices: vec![
                        IngredientChoice {
                            value: String::from("water"),
                            ingredients: vec![
                                // The ladder: tungsten is another mod's plate
                                // and is absent from the stand-in, so the
                                // second rung answers and the drop of the
                                // first is visible in the transcript.
                                Ingredient::named(2, "tungsten-plate", &["steel-plate"]),
                                Ingredient::of(rivet, 4),
                                // An optional hardener only an overhaul pack
                                // provides. Neither candidate is in the
                                // stand-in, so the whole ingredient is DROPPED
                                // with a log line rather than guessed at,
                                // which is the other half of the ladder.
                                Ingredient::named(1, "tungsten-carbide", &["titanium-plate"]),
                            ],
                        },
                        // The organic bath: cheaper in rivets, and it wants an
                        // oil this stand-in does not have, so the drop line
                        // fires on this branch too. Enough survives that the
                        // water plan is not reached for.
                        IngredientChoice {
                            value: String::from("oil"),
                            ingredients: vec![
                                Ingredient::named(2, "steel-plate", &[]),
                                Ingredient::of(rivet, 2),
                                Ingredient::named(1, "light-oil-barrel", &["crude-oil-barrel"]),
                            ],
                        },
                    ],
                }),
                name: String::from("hardened-steel-plate-quenching"),
                category: String::from("smelting"),
                order: String::from("b[steelworks]-b[quenching]"),
                display_name: String::from("Hardened steel plate"),
                description: String::from("Quench the plate, then temper it back to workable."),
                ..Default::default()
            },
        );

        // Reclaimed from worn plate, and known from the start: nothing unlocks
        // it, so it is enabled without research.
        lib.recipe(
            rivet,
            RecipeSpec {
                name: String::from("salvaged-steel-rivet"),
                craft_time_from: tempering,
                result_count: 3,
                ingredients: vec![Ingredient::of(plate, 1)],
                display_name: String::from("Salvaged steel rivets"),
                ..Default::default()
            },
        );

        let hardened_steel = lib.technology(
            "hardened-steel",
            TechSpec {
                icon: String::from("__fkrecipes-example__/graphics/technology/hardened-steel.png"),
                icon_size: 128,
                cost_of: String::from("logistics-2"),
                after: String::from("steel-processing"),
                before: String::from("logistics-2"),
                unlocks: vec![rivets, plates],
                enabled_by: hardened,
                display_name: String::from("Hardened steel"),
                description: String::from("Quenching steel plate to make it hold an edge."),
                ..Default::default()
            },
        );
        lib.technology(
            "steel-riveting",
            TechSpec {
                icon: String::from("__fkrecipes-example__/graphics/technology/steel-riveting.png"),
                icon_size: 128,
                unit: Some(UnitSpec {
                    count: 45,
                    seconds: 20.0,
                    packs: vec![Pack {
                        name: String::from("automation-science-pack"),
                        amount: 1,
                    }],
                }),
                after_tech: hardened_steel,
                display_name: String::from("Steel riveting"),
                ..Default::default()
            },
        );
        // A bonus line the player prices for themselves. Each ladder is walked
        // to the first technology that is actually there and carries a cost,
        // and THE PREREQUISITE MOVES WITH THE UNIT: whichever source pays for
        // this one also becomes the thing it hangs off, so cost and tree
        // position never disagree.
        lib.technology(
            "hardened-tips",
            TechSpec {
                icon: String::from("__fkrecipes-example__/graphics/technology/hardened-tips.png"),
                icon_size: 128,
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: vec![
                        // The first rung is an overhaul pack's technology and
                        // is in neither the stand-in nor the game, so the
                        // ladder steps past it to the multi-level one, whose
                        // count_formula and level cap come across with the
                        // unit.
                        CostChoice {
                            value: String::from("projectile"),
                            sources: vec![
                                String::from("tungsten-hardening"),
                                String::from("physical-projectile-damage-7"),
                            ],
                        },
                        CostChoice {
                            value: String::from("military"),
                            sources: vec![String::from("military-4")],
                        },
                    ],
                    // What applies when a ladder finds nothing at all: the
                    // technology is still researchable, and still says so in
                    // the log.
                    fallback: UnitSpec {
                        count: 200,
                        seconds: 30.0,
                        packs: vec![Pack {
                            name: String::from("automation-science-pack"),
                            amount: 1,
                        }],
                    },
                }),
                enabled_by: bonuses,
                display_name: String::from("Hardened tool tips"),
                description: String::from("Every level puts a harder edge on the same tools."),
                ..Default::default()
            },
        );

        lib
    }

    /// The two stage exports. The library exports NOTHING of its own: these
    /// names belong to the consuming mod, which is this example.
    #[no_mangle]
    pub extern "C" fn fk_settings() {
        plan().emit();
    }

    #[no_mangle]
    pub extern "C" fn fk_data() {
        plan().emit();
    }
}
