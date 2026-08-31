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
    use fkrecipes::{Ingredient, ItemSpec, Lib, NumericSpec, Pack, RecipeSpec, TechSpec, UnitSpec};

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
        lib.dropdown_setting_needing_locale("quench-medium", "water", &["water", "oil"]);
        let bonuses = lib.bool_setting("bonus-research", true);

        let plate = lib.item(
            "hardened-steel-plate",
            ItemSpec {
                icon: String::from("__fkrecipes-example__/graphics/icons/hardened-steel-plate.png"),
                icon_size: 64,
                stack_size: 100,
                subgroup: String::new(),
                display_name: String::from("Hardened steel plate"),
                description: String::from("Quenched and tempered, for tools that keep an edge."),
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
            },
        );

        let rivets = lib.recipe(
            rivet,
            RecipeSpec {
                craft_time: 0.5,
                result_count: 4,
                ingredients: vec![Ingredient::named(1, "iron-plate", &[])],
                display_name: String::from("Steel rivets"),
                ..Default::default()
            },
        );
        let plates = lib.recipe(
            plate,
            RecipeSpec {
                craft_time_from: forging,
                ingredients: vec![
                    // The ladder: tungsten is another mod's plate and is
                    // absent from the stand-in, so the second rung answers and
                    // the drop of the first is visible in the transcript.
                    Ingredient::named(2, "tungsten-plate", &["steel-plate"]),
                    Ingredient::of(rivet, 4),
                    // An optional hardener only an overhaul pack provides.
                    // Neither candidate is in the stand-in, so the whole
                    // ingredient is DROPPED with a log line rather than
                    // guessed at, which is the other half of the ladder and
                    // the only line the transcript carries from fkdata::log.
                    Ingredient::named(1, "tungsten-carbide", &["titanium-plate"]),
                ],
                name: String::from("hardened-steel-plate-quenching"),
                category: String::from("smelting"),
                display_name: String::from("Hardened steel plate"),
                description: String::from("Quench the plate, then temper it back to workable."),
                ..Default::default()
            },
        );

        // Reclaimed from worn plate, and known from the start: nothing unlocks
        // it, so it is enabled without research, and it names no crafting
        // time, so the engine's own default applies.
        lib.recipe(
            rivet,
            RecipeSpec {
                name: String::from("salvaged-steel-rivet"),
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
        // A bonus line: it unlocks nothing, hangs off nothing, and prices
        // itself from a multi-level technology, so the formula and the level
        // cap come across with the unit.
        lib.technology(
            "hardened-tips",
            TechSpec {
                icon: String::from("__fkrecipes-example__/graphics/technology/hardened-tips.png"),
                icon_size: 128,
                cost_of: String::from("physical-projectile-damage-7"),
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
