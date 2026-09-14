//! THE ENGINE'S ELEMENT CEILING, OVER EVERY COMPOSITION THIS LIBRARY CAN WRITE
//! ONTO A DATA-STAGE PROTOTYPE.
//!
//! The rule is measured and recorded at `LOCALISED_ELEMENT_CEILING`: ONE STRING
//! ELEMENT of a localised string on a data prototype is at most 200 BYTES, and
//! the 201st refuses the WHOLE LOAD with a message naming the prototype and the
//! element index. That is a lock-out rather than a degradation, so the property
//! this file asserts is a load-bearing one: nothing this library composes may
//! reach it, on any mod name, with any setting spelling, on any mod set.
//!
//! IT IS A WALK AND NOT A LIST OF SENTENCES, deliberately. A test that measured
//! the six notes one at a time would pass the day a seventh is added and would
//! say nothing about the composition each note lands in: a note joins an
//! author's own description, and the pair is what the engine reads. So the
//! fixture below reaches every composition from the PUBLIC surface, with the
//! longest names the engine's own name ceiling allows substituted into every
//! slot a sentence names, and the assertion walks what `plan_data` actually
//! emitted.
//!
//! THE DATA SIDE ONLY. A setting prototype is not subject to the rule at all
//! (measured; see `LOCALISED_ELEMENT_CEILING`), its composed lines are compared
//! WHOLE by the locale guard, and chunking them would move
//! `testdata/locale/findings.golden`. `plan_settings` is therefore not walked
//! here, and that is the rule rather than an omission.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::op::Op;
use crate::plan::{
    CostChoice, CostChoices, CustomCost, Ingredient, ItemSpec, Lib, NumericSpec, Pack, RecipeSpec,
    TechSpec, UnitSpec,
};
use crate::settings::MAX_LOCALISED_PARAMS;
use crate::tests::customize::localised_descriptions;
use crate::tests::*;
use crate::value::{
    chunk_localised, kv, Value, LOCALISED_CHUNK_BUDGET, LOCALISED_ELEMENT_CEILING, MAX_ITEM_AMOUNT,
};

/// The ENGINE's own limit on a PROTOTYPE NAME, recorded in
/// `agents/customizer-design.md`: 201 bytes refuses the load with `Name field
/// is too large. Max allowed size is: 200.` and 200 loads. It is a DIFFERENT
/// RULE from the element ceiling, with a different message, which is why
/// `assert_name_fits` is a different assertion.
const PROTOTYPE_NAME_CEILING: usize = 200;

/// How long THIS FIXTURE makes its names, and it is a second constant at the
/// same number rather than a reuse of the first, for the reason
/// `LOCALISED_CHUNK_BUDGET` is a second constant beside
/// `LOCALISED_ELEMENT_CEILING`: one belongs to the engine and one to this file,
/// and `assert_name_fits` is the claim that the second is inside the first.
/// Reusing one constant for both would make that assertion true by construction
/// and impossible to red-prove.
const FIXTURE_NAME_BYTES: usize = 200;

/// What the data stage puts in front of every name this plan declares, derived
/// from the fixture World's mod name.
const FIXTURE_PREFIX: &str = "steelworks-";

/// A name this plan declares, sized so the name the library EMITS is exactly
/// [`FIXTURE_NAME_BYTES`].
fn declared_name(stem: &str) -> String {
    pad_name(stem, FIXTURE_NAME_BYTES - FIXTURE_PREFIX.len())
}

/// A name the GAME carries, at the same length. Nothing is prefixed onto a name
/// this library only reads.
fn existing_name(stem: &str) -> String {
    pad_name(stem, FIXTURE_NAME_BYTES)
}

fn pad_name(stem: &str, n: usize) -> String {
    assert!(
        stem.len() < n,
        "the stem {} does not fit in {} bytes",
        stem,
        n
    );
    format!("{}-{}", stem, "q".repeat(n - stem.len() - 1))
}

/// `n` bytes of ordinary spaced words, which is what a consumer's description
/// looks like and what the chunker's SPACE arm walks.
fn fixture_prose(n: usize) -> String {
    let mut out = String::new();
    let mut i = 0;
    while out.len() < n {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("word{}", i));
        i += 1;
    }
    out.truncate(n);
    out
}

/// The one input that reaches the chunker's HARD CUT and its UTF-8 back-off: a
/// single run longer than the budget, with a two-byte character sitting exactly
/// across the byte a blind cut would land on.
fn fixture_unbroken_prose() -> String {
    format!(
        "{}\u{e9}{} and a tail.",
        "z".repeat(LOCALISED_CHUNK_BUDGET - 1),
        "z".repeat(60)
    )
}

/// Walks every prototype the fixture plan emits and asserts the two ceilings a
/// localised string has: 200 bytes per string element, and twenty-one elements
/// per table (the leading `""` plus the twenty parameters `localised_group`
/// fills a level to).
#[test]
fn no_composition_reaches_the_element_ceiling() {
    let (lib, w) = worst_case_plan();

    let ops = lib.plan_data(&w).expect("the fixture plan was refused");

    // THE WALK IS WORTHLESS OVER AN EMPTY STREAM, and a fixture that stopped
    // reaching the notes would leave every assertion below vacuous, so the
    // compositions are counted before they are measured.
    let mut described = 0;
    for op in &ops {
        if let Op::Extend(proto) = op {
            described += localised_descriptions(proto);
        }
    }
    assert_eq!(
        described, 32,
        "the fixture emitted {} localised_description fields, not the 32 it declares; \
         the walk below would prove nothing about the ones it lost",
        described
    );

    for op in &ops {
        match op {
            Op::Extend(proto) => {
                assert_name_fits(proto);
                walk_for_ceilings(&proto_where(proto), proto);
            }
            Op::Set(_, val) => walk_for_ceilings("a spliced field", val),
            Op::Log(_) => {}
        }
    }
}

/// A SEPARATE RULE WITH A SEPARATE MESSAGE, and it is separate on purpose.
///
/// The element ceiling below is about localised strings and nothing else: the
/// engine carries non-localised strings far past 200 bytes without complaint
/// (base's own utility-constants holds a 2697-byte one), so a walk that
/// measured every string in a prototype would refuse what the engine loads and
/// would cite the wrong sentence doing it. A prototype NAME has its own limit
/// at the same number and its own refusal: see [`PROTOTYPE_NAME_CEILING`].
///
/// WHAT IT IS FOR IS THE FIXTURE. Every name below is padded to
/// [`FIXTURE_NAME_BYTES`] so that every sentence is composed at its worst case,
/// and this is the check that the padding did not walk off the end of what the
/// engine takes: a fixture whose names the engine would refuse is a fixture
/// proving nothing about a load that can happen.
fn assert_name_fits(proto: &Value) {
    let name = match field(proto, "name") {
        Some(Value::Str(s)) => s,
        _ => return,
    };
    assert!(
        name.len() <= PROTOTYPE_NAME_CEILING,
        "{} is {} bytes, over the engine's PROTOTYPE NAME ceiling of {} \
         (a different rule from the element ceiling: `Name field is too large.`)",
        proto_where(proto),
        name.len(),
        PROTOTYPE_NAME_CEILING
    );
}

/// Names a prototype the way the engine's own refusal does, so a failure here
/// is greppable against a real load failure.
fn proto_where(proto: &Value) -> String {
    let kind = field(proto, "type").unwrap_or(Value::string(""));
    let name = field(proto, "name").unwrap_or(Value::string(""));
    format!("ROOT.{}.{}", as_str(&kind), as_str(&name))
}

fn as_str(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        _ => String::new(),
    }
}

/// Recurses into every Value a prototype carries. The `localised` flag turns on
/// under a `localised_name` or a `localised_description` and stays on below it.
///
/// BOTH RULES ARE SCOPED BY THAT FLAG, and that is the whole point of it. The
/// engine's element ceiling is a rule about LOCALISED STRINGS: a prototype is
/// free to carry a longer string anywhere else, and base's own
/// utility-constants does. A walk that measured every string would fail a
/// consumer's long order or icon field citing a rule the engine does not apply
/// there. The two named fields are exactly what
/// `testdata/mirror/standin.lua`'s `check_localised` and the in-game gate's own
/// assertion measure, so the three enforcement points say one thing. A
/// prototype name is policed at the same number by a different rule and has its
/// own assertion: see `assert_name_fits`.
fn walk_for_ceilings(where_: &str, proto: &Value) {
    walk_value_for_ceilings(where_, "", proto, false);
}

fn walk_value_for_ceilings(where_: &str, path: &str, v: &Value, localised: bool) {
    match v {
        Value::Str(s) => assert!(
            !localised || s.len() <= LOCALISED_ELEMENT_CEILING,
            "{}{} is {} bytes, over the engine's element ceiling of {}:\n  {:?}",
            where_,
            path,
            s.len(),
            LOCALISED_ELEMENT_CEILING,
            s
        ),
        Value::Arr(items) => {
            assert!(
                !localised || items.len() <= MAX_LOCALISED_PARAMS + 1,
                "{}{} holds {} elements, over the {} a localised string takes",
                where_,
                path,
                items.len(),
                MAX_LOCALISED_PARAMS + 1
            );
            for (i, item) in items.iter().enumerate() {
                walk_value_for_ceilings(where_, &format!("{}[{}]", path, i), item, localised);
            }
        }
        Value::Map(entries) => {
            for (k, val) in entries {
                let under = localised || k == "localised_name" || k == "localised_description";
                walk_value_for_ceilings(where_, &format!("{}.{}", path, k), val, under);
            }
        }
        _ => {}
    }
}

/// Every composition this library can write onto a data prototype, each one
/// reached from the PUBLIC surface and each one with the longest legal name in
/// the slot its sentence names.
///
/// THIRTY-TWO DESCRIPTIONS, and the count is asserted above:
///
/// - an item with a display name and a description;
/// - a recipe carrying the FALLBACK note, with a description and without;
/// - a recipe carrying the clamped ITEM note, with and without;
/// - a recipe carrying the clamped FLUID note, with and without;
/// - a recipe carrying the INGREDIENTLESS note, with and without;
/// - a technology carrying the FALLBACK note, with and without;
/// - a technology carrying the DROPPED-PACK note, with and without;
/// - a technology carrying the PACKLESS-SOURCE note, with and without;
/// - a technology carrying the UNPRICED-SOURCE note, with and without;
/// - a technology carrying the PACKLESS note, with and without;
/// - a technology carrying the UNREADABLE-SOURCE note, with and without;
/// - a technology carrying the UNREADABLE-COPY note, with and without;
/// - a technology carrying the CYCLE-PREREQUISITE note, with and without;
/// - a technology carrying the CYCLE-SPLICE note, with and without;
/// - a technology carrying the clamped PACK note, with and without;
/// - a recipe and a technology at the longest name whose composed
///   `[<kind>-description]` key FITS the element ceiling, both undescribed;
/// - and an item whose description alone is long enough to NEST.
///
/// THE LAST TWO ROWS ADDED ARE THE ONES THE COUNT ALONE COULD NOT HAVE CAUGHT.
/// `unpriced_source_note` and `unreadable_copy_note` were composed by the
/// library and walked by nothing here, which made the claim over this file
/// ("every composition this library can write onto a data prototype") false
/// while every assertion in it passed. A composition missing from the fixture
/// is invisible to the count, because the count is of what the fixture emits;
/// the only guard is that the fixture is extended in the same commit as the
/// composer. The note SET is now held mechanically by
/// `every_note_call_site_is_accounted_for`, and a composer that appears there
/// and not in this list is the next thing to add.
///
/// THE PAIRS ARE PAIRS BECAUSE THE COMPOSITION IS WHAT IS WALKED, not the note:
/// a note beside an author's description and a note alone are two different
/// localised strings, and only one of them can nest. That is why a further note
/// adds TWO rows here and not one.
///
/// THE CLAMPED PACK NOTE IS THE SAME SENTENCE AS THE CLAMPED ITEM ONE AND IS
/// NOT THE SAME COMPOSITION. `merge_pack` writes `clamped_item_note` BARE, with
/// no destruction sentence, because a research costs no assembling machine
/// anything; `merge_ingredient` writes it with the sentence. Two composers, two
/// lengths, and the bare one had no test in either suite before this row.
///
/// Every RECIPE note carries the destruction sentence, which is the longest of
/// the shapes each can take: the ingredient list is what moved in all three
/// recipe cases.
fn worst_case_plan() -> (Lib, FixtureWorld) {
    let item = existing_name("iron-plate");
    let fluid = existing_name("water");
    let pack = existing_name("automation-science-pack");
    let absent_pack = existing_name("logistic-science-pack");
    let drop_source = existing_name("logistics-2");
    let packless_source = existing_name("steel-processing");
    let unreadable_source = existing_name("electronics");
    // A source the fixture World is never given, which is what the
    // unpriced-source arm needs: every rung of the tier's ladder absent.
    let absent_source = existing_name("nothing-carries-this-cost");

    // A description that forces `localised_group` to NEST: 7200 bytes is forty
    // chunks at the budget, and one level holds twenty.
    let nesting = fixture_prose(7200);

    let mut lib = Lib::new();

    // The item, and the two shapes an item's own prose takes: a display name
    // and a description, both past the budget, and a description long enough
    // to nest on its own.
    lib.item(
        &declared_name("described-item"),
        ItemSpec {
            display_name: fixture_prose(900),
            description: fixture_unbroken_prose(),
            ..Default::default()
        },
    );
    lib.item(
        &declared_name("nesting-item"),
        ItemSpec {
            description: nesting.clone(),
            ..Default::default()
        },
    );

    // THE FALLBACK NOTE ON A RECIPE. Two settings rather than one, so the
    // described and the undescribed recipe are two independent prototypes with
    // two independent notes.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("fallback-recipe-{}", i);
        let result = lib.item(
            &declared_name(&format!("{}-item", stem)),
            ItemSpec::default(),
        );
        let parts = lib.ingredients_setting(
            &declared_name(&format!("{}-setting", stem)),
            alloc::vec![Ingredient::named(1, &item, &[])],
        );
        lib.recipe(
            result,
            RecipeSpec {
                name: declared_name(&stem),
                description: described_prose(describe, &nesting),
                ingredients_from: Some(parts),
                ..Default::default()
            },
        );
    }

    // THE CLAMPED ITEM NOTE, a merge above the engine's 65535.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("clamped-item-recipe-{}", i);
        let result = lib.item(
            &declared_name(&format!("{}-item", stem)),
            ItemSpec::default(),
        );
        lib.recipe(
            result,
            RecipeSpec {
                name: declared_name(&stem),
                description: described_prose(describe, &fixture_prose(400)),
                ingredients: alloc::vec![
                    Ingredient::named(40000, &item, &[]),
                    Ingredient::named(30000, &existing_name("transport-belt"), &[&item]),
                ],
                ..Default::default()
            },
        );
    }

    // THE CLAMPED FLUID NOTE, the same merge above the fluid ceiling.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("clamped-fluid-recipe-{}", i);
        let result = lib.item(
            &declared_name(&format!("{}-item", stem)),
            ItemSpec::default(),
        );
        lib.recipe(
            result,
            RecipeSpec {
                name: declared_name(&stem),
                category: String::from("chemistry"),
                description: described_prose(describe, &fixture_prose(400)),
                ingredients: alloc::vec![
                    Ingredient::fluid(5e300, &fluid, &[]),
                    Ingredient::fluid(6e300, &existing_name("steam"), &[&fluid]),
                ],
                ..Default::default()
            },
        );
    }

    // THE INGREDIENTLESS NOTE: every entry the recipe declares is a ladder this
    // game has no rung of, so the emitted list is empty.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("ingredientless-recipe-{}", i);
        let result = lib.item(
            &declared_name(&format!("{}-item", stem)),
            ItemSpec::default(),
        );
        lib.recipe(
            result,
            RecipeSpec {
                name: declared_name(&stem),
                description: described_prose(describe, &fixture_prose(400)),
                ingredients: alloc::vec![Ingredient::named(
                    1,
                    &existing_name("nothing-has-this-item"),
                    &[]
                )],
                ..Default::default()
            },
        );
    }

    // THE FALLBACK NOTE ON A TECHNOLOGY, through a custom research cost.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("fallback-tech-{}", i);
        let packs = lib.packs_setting(
            &declared_name(&format!("{}-packs", stem)),
            alloc::vec![Pack::named(1, "automation-science-pack", &[])],
        );
        let count = lib.int_setting(
            &declared_name(&format!("{}-count", stem)),
            20,
            NumericSpec::between(1.0, 100000.0),
        );
        let seconds = lib.int_setting(
            &declared_name(&format!("{}-seconds", stem)),
            10,
            NumericSpec::between(1.0, 600.0),
        );
        lib.technology(
            &declared_name(&stem),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                cost_from: Some(CustomCost {
                    packs,
                    count,
                    seconds,
                }),
                ..Default::default()
            },
        );
    }

    // THE DROPPED-PACK NOTE: a copied unit naming a pack this game does not
    // have as a tool.
    for (i, describe) in [false, true].into_iter().enumerate() {
        lib.technology(
            &declared_name(&format!("dropped-pack-tech-{}", i)),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                cost_of: drop_source.clone(),
                ..Default::default()
            },
        );
    }

    // THE PACKLESS-SOURCE NOTE: a tier whose source names no pack this game
    // has, so the author's own declared cost applies.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("packless-tech-{}", i);
        let tier = lib.dropdown_setting_needing_locale(
            &declared_name(&format!("{}-tier", stem)),
            "early",
            &["early"],
        );
        lib.technology(
            &declared_name(&stem),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: alloc::vec![CostChoice {
                        value: String::from("early"),
                        sources: alloc::vec![packless_source.clone()],
                    }],
                    fallback: UnitSpec {
                        count: 7,
                        seconds: 8.0,
                        packs: alloc::vec![Pack::named(2, "automation-science-pack", &[])],
                    },
                }),
                ..Default::default()
            },
        );
    }

    // THE UNPRICED-SOURCE NOTE: a tier whose every source is absent from this
    // game, so nothing was copied at all and the technology is priced by the
    // author's own declared fallback with no prerequisite. The fallback names a
    // pack the game HAS and one amount, so nothing worse than this takes the
    // slot: `packless_at` and `merge_pack` are both offered the slot first, by
    // construction, and the two rows here would be measuring one of those
    // sentences instead if either fired.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("unpriced-tech-{}", i);
        let tier = lib.dropdown_setting_needing_locale(
            &declared_name(&format!("{}-tier", stem)),
            "early",
            &["early"],
        );
        lib.technology(
            &declared_name(&stem),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: alloc::vec![CostChoice {
                        value: String::from("early"),
                        sources: alloc::vec![absent_source.clone()],
                    }],
                    fallback: UnitSpec {
                        count: 7,
                        seconds: 8.0,
                        packs: alloc::vec![Pack::named(2, &pack, &[])],
                    },
                }),
                ..Default::default()
            },
        );
    }

    // THE UNREADABLE-COPY NOTE: the unreadable source again, this time behind a
    // bare `cost_of` with nothing declared to fall back to, so the unit is
    // emitted with an empty ingredient list and the sentence says what could not
    // be read rather than what the game does not have.
    for (i, describe) in [false, true].into_iter().enumerate() {
        lib.technology(
            &declared_name(&format!("unreadable-copy-tech-{}", i)),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                cost_of: unreadable_source.clone(),
                ..Default::default()
            },
        );
    }

    // THE PACKLESS NOTE: a hand-rolled unit whose only pack the game does not
    // have, so the technology is emitted with an empty ingredient list and the
    // tooltip says the research completes for free.
    for (i, describe) in [false, true].into_iter().enumerate() {
        lib.technology(
            &declared_name(&format!("packless-unit-tech-{}", i)),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                unit: Some(UnitSpec {
                    count: 10,
                    seconds: 15.0,
                    packs: alloc::vec![Pack::named(1, &absent_pack, &[])],
                }),
                ..Default::default()
            },
        );
    }

    // THE UNREADABLE-SOURCE NOTE: a tier whose chosen source carries a pack
    // list in neither engine form, so the author's own declared cost applies.
    for (i, describe) in [false, true].into_iter().enumerate() {
        let stem = format!("unreadable-tech-{}", i);
        let tier = lib.dropdown_setting_needing_locale(
            &declared_name(&format!("{}-tier", stem)),
            "early",
            &["early"],
        );
        lib.technology(
            &declared_name(&stem),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                cost_by: Some(CostChoices {
                    setting: tier,
                    choices: alloc::vec![CostChoice {
                        value: String::from("early"),
                        sources: alloc::vec![unreadable_source.clone()],
                    }],
                    fallback: UnitSpec {
                        count: 7,
                        seconds: 8.0,
                        packs: alloc::vec![Pack::named(2, &pack, &[])],
                    },
                }),
                ..Default::default()
            },
        );
    }

    // THE CYCLE-PREREQUISITE NOTE: a technology anchored After a technology the
    // game has already been made to require it, so the plan's own prerequisite
    // closes the ring and is dropped.
    for (i, describe) in [false, true].into_iter().enumerate() {
        lib.technology(
            &declared_name(&format!("cycle-prereq-tech-{}", i)),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                unit: Some(UnitSpec {
                    count: 10,
                    seconds: 15.0,
                    packs: alloc::vec![Pack::named(1, &pack, &[])],
                }),
                after: ring_anchor(i),
                ..Default::default()
            },
        );
    }

    // THE CYCLE-SPLICE NOTE: an InsertBetween whose splice closes the ring,
    // because the anchor it hangs off already leads back to the technology it
    // is spliced into.
    for (i, describe) in [false, true].into_iter().enumerate() {
        lib.technology(
            &declared_name(&format!("cycle-splice-tech-{}", i)),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                unit: Some(UnitSpec {
                    count: 10,
                    seconds: 15.0,
                    packs: alloc::vec![Pack::named(1, &pack, &[])],
                }),
                after: splice_anchor(i),
                before: splice_target(i),
                ..Default::default()
            },
        );
    }

    // THE CLAMPED PACK NOTE: two declared packs whose ladders land on one name,
    // each at the ceiling, so the SUM is a number no author wrote. It is the
    // only pack amount `validate_unit` does not already hold to 65535, and the
    // note it leaves is `clamped_item_note` BARE: `merge_pack` writes no
    // destruction sentence, because a research costs no assembling machine
    // anything.
    for (i, describe) in [false, true].into_iter().enumerate() {
        lib.technology(
            &declared_name(&format!("clamped-pack-tech-{}", i)),
            TechSpec {
                description: described_prose(describe, &fixture_prose(400)),
                unit: Some(UnitSpec {
                    count: 10,
                    seconds: 15.0,
                    packs: alloc::vec![
                        Pack::named(MAX_ITEM_AMOUNT, &pack, &[]),
                        Pack::named(MAX_ITEM_AMOUNT, &absent_pack, &[&pack]),
                    ],
                }),
                ..Default::default()
            },
        );
    }

    // THE COMPOSED DESCRIPTION KEY AT EXACTLY THE ELEMENT CEILING, one of each
    // kind. Every other row here is named at `FIXTURE_NAME_BYTES`, where the
    // composed key is over the ceiling and `description_ref` DROPS it, so
    // without these two the walk would measure the drop twice and a composed
    // key never. The names are the longest whose key fits, which makes the key
    // element the walk reads exactly `LOCALISED_ELEMENT_CEILING` bytes: see
    // `description_key_ceiling` for the arithmetic and for why the case is
    // reachable at all.
    let keyed_recipe = pad_name(
        "keyed-recipe",
        description_key_ceiling("recipe") - FIXTURE_PREFIX.len(),
    );
    let keyed_result = lib.item(&declared_name("keyed-recipe-item"), ItemSpec::default());
    let keyed_parts = lib.ingredients_setting(
        &declared_name("keyed-recipe-setting"),
        alloc::vec![Ingredient::named(1, &item, &[])],
    );
    lib.recipe(
        keyed_result,
        RecipeSpec {
            name: keyed_recipe,
            ingredients_from: Some(keyed_parts),
            ..Default::default()
        },
    );
    lib.technology(
        &pad_name(
            "keyed-tech",
            description_key_ceiling("technology") - FIXTURE_PREFIX.len(),
        ),
        TechSpec {
            unit: Some(UnitSpec {
                count: 10,
                seconds: 15.0,
                packs: alloc::vec![Pack::named(1, &absent_pack, &[])],
            }),
            ..Default::default()
        },
    );

    // The copied unit the DROPPED-PACK note reads names one pack the game has
    // and one it does not; the PACKLESS-SOURCE one names only the pack it does
    // not.
    let mut w = base_world()
        .with_item(&item)
        .with_fluid(&fluid)
        .with_tool(&pack)
        .with_tech(FixtureTech {
            name: drop_source.clone(),
            prereqs: Vec::new(),
            unit: unit_of(200, 30.0, &[&absent_pack, "automation-science-pack"]),
            max_level: Value::Nil,
            trigger: false,
        })
        .with_tech(FixtureTech {
            name: packless_source.clone(),
            prereqs: Vec::new(),
            unit: unit_of(7, 8.0, &[&absent_pack]),
            max_level: Value::Nil,
            trigger: false,
        })
        // A unit whose ingredients array holds an entry in NEITHER engine form.
        .with_tech(FixtureTech {
            name: unreadable_source.clone(),
            prereqs: Vec::new(),
            unit: Value::Map(alloc::vec![
                kv("count", Value::Num(7.0)),
                kv("ingredients", Value::Arr(alloc::vec![Value::string(&pack)])),
                kv("time", Value::Num(8.0)),
            ]),
            max_level: Value::Nil,
            trigger: false,
        });

    // THE TWO RINGS, one per pair, each closed through an existing technology
    // that the fixture makes require the name this plan is about to emit.
    // TWO SEPARATE ANCHORS PER PAIR, because the walk drops ONE edge per ring
    // and a shared anchor would make the two rows one ring.
    for i in 0..2 {
        w = w.with_tech(FixtureTech {
            name: ring_anchor(i),
            prereqs: alloc::vec![format!(
                "{}{}",
                FIXTURE_PREFIX,
                declared_name(&format!("cycle-prereq-tech-{}", i))
            )],
            unit: unit_of(10, 15.0, &[&pack]),
            max_level: Value::Nil,
            trigger: false,
        });
        w = w.with_tech(FixtureTech {
            name: splice_target(i),
            prereqs: Vec::new(),
            unit: unit_of(10, 15.0, &[&pack]),
            max_level: Value::Nil,
            trigger: false,
        });
        w = w.with_tech(FixtureTech {
            name: splice_anchor(i),
            prereqs: alloc::vec![splice_target(i)],
            unit: unit_of(10, 15.0, &[&pack]),
            max_level: Value::Nil,
            trigger: false,
        });
    }

    // The two texts the player typed and the library cannot use. Everything
    // else the settings answer is left absent, which is the ordinary
    // unreadable-setting arm and composes nothing.
    for i in 0..2 {
        w = w.with_setting(
            &format!(
                "{}{}",
                FIXTURE_PREFIX,
                declared_name(&format!("fallback-recipe-{}-setting", i))
            ),
            Value::Str(format!("1 {}", existing_name("nothing-is-named-this"))),
        );
        w = w.with_setting(
            &format!(
                "{}{}",
                FIXTURE_PREFIX,
                declared_name(&format!("fallback-tech-{}-packs", i))
            ),
            Value::Str(format!("1 {}", existing_name("nothing-is-named-this"))),
        );
    }
    w = w.with_setting(
        &format!(
            "{}{}",
            FIXTURE_PREFIX,
            declared_name("keyed-recipe-setting")
        ),
        Value::Str(format!("1 {}", existing_name("nothing-is-named-this"))),
    );
    (lib, w)
}

/// The three existing technologies each ring in the fixture is closed through.
/// They are functions rather than constants because each pair needs its OWN
/// ring: see the loop that builds them.
fn ring_anchor(i: usize) -> String {
    existing_name(&format!("ring-anchor-{}", i))
}

fn splice_anchor(i: usize) -> String {
    existing_name(&format!("splice-anchor-{}", i))
}

fn splice_target(i: usize) -> String {
    existing_name(&format!("splice-target-{}", i))
}

/// The with-a-description arm of every pair above, and the empty string is the
/// without arm. One helper rather than an `if` at every site, so the pairs read
/// as pairs.
fn described_prose(describe: bool, prose: &str) -> String {
    if describe {
        String::from(prose)
    } else {
        String::new()
    }
}

/// THE SPLIT ITSELF, WRITTEN OUT BY HAND, which is the one place it is.
///
/// Every transcript in the suite composes its expected parameters through
/// `chunked_params`, so a change to the budget moves one golden rather than
/// thirty. That helper asks the chunker, so it cannot catch the chunker being
/// wrong; this is what does. The arms are the three the chunker has: a text
/// inside the budget, a text that ends its chunks after a space, and a run with
/// no space in it at all, where the cut backs off the middle of a UTF-8
/// character.
#[test]
fn the_chunker_splits_on_spaces_within_the_budget() {
    let e = "\u{e9}"; // two bytes, so a blind cut at the budget lands inside it
    let note = crate::data::fallback_note("steelworks-rivet-ingredients", true);

    let cases: Vec<(&str, String, Vec<String>)> = alloc::vec![
        (
            "a text inside the budget is one chunk and is not touched",
            String::from("Forged from plate."),
            alloc::vec![String::from("Forged from plate.")],
        ),
        (
            "the empty string is one empty chunk",
            String::new(),
            alloc::vec![String::new()],
        ),
        (
            "exactly the budget is still one chunk",
            "z".repeat(180),
            alloc::vec!["z".repeat(180)],
        ),
        (
            "one byte past the budget splits, and the space ENDS the first chunk",
            format!("{} abcde", "z".repeat(175)),
            alloc::vec![format!("{} ", "z".repeat(175)), String::from("abcde")],
        ),
        (
            "a run with no space is cut at the budget",
            "z".repeat(181),
            alloc::vec!["z".repeat(180), String::from("z")],
        ),
        (
            "a cut that would land inside a UTF-8 character backs off to its first byte",
            format!("{}{}abc", "z".repeat(179), e),
            alloc::vec!["z".repeat(179), format!("{}abc", e)],
        ),
        (
            "the note a recipe's ingredient text falls back with",
            note,
            alloc::vec![
                String::from("The stored value of steelworks-rivet-ingredients could not be used, so the game loaded as though that setting had been left alone. The reason is in the log. Changing a recipe "),
                String::from("empties an assembling machine's input slots of anything the new list does not use."),
            ],
        ),
    ];

    for (what, input, want) in cases {
        let got = chunk_localised(&input);
        assert_eq!(
            got.len(),
            want.len(),
            "{}:\n got {} chunks: {:?}\nwant {} chunks: {:?}",
            what,
            got.len(),
            got,
            want.len(),
            want
        );
        for (i, piece) in got.iter().enumerate() {
            assert_eq!(*piece, want[i].as_str(), "{}, chunk {}", what, i);
        }
        // The two properties the assembly rests on, re-asked on every case:
        // the pieces are the input byte for byte, and none of them is over the
        // budget.
        assert_eq!(
            got.concat(),
            input,
            "{}: the chunks do not concatenate to the input",
            what
        );
        for (i, piece) in got.iter().enumerate() {
            assert!(
                piece.len() <= LOCALISED_CHUNK_BUDGET,
                "{}: chunk {} is {} bytes, over the budget of {}",
                what,
                i,
                piece.len(),
                LOCALISED_CHUNK_BUDGET
            );
        }
    }
}

// ---------------------------------------------------------------------------
// THE AUTHOR'S OWN DESCRIPTION SURVIVES A NOTE.
//
// A prototype's own `localised_description` field WINS OVER the
// `[recipe-description]` or `[technology-description]` entry a `.cfg` defines,
// so a note emitted as `{"", "<note>"}` DISPLACED the description of every
// author who wrote one the ordinary Factorio way. `description_ref` is the
// answer: the note opens with the author's own key behind an empty
// alternative, so the engine renders their sentence and a newline where they
// wrote one and nothing where they did not.
// ---------------------------------------------------------------------------

/// The longest EMITTED prototype name of a kind whose composed
/// `[<kind>-description]` key is exactly [`LOCALISED_ELEMENT_CEILING`] bytes.
///
/// THE ARITHMETIC IS THE WHOLE POINT OF THIS TEST. A key is ONE element by
/// definition and cannot be chunked, the engine polices the key slot at 200
/// bytes like every other element, and the engine's own prototype-name ceiling
/// is 200 bytes with nothing shorter refused anywhere in this library. So
/// `technology-description.` at 23 bytes over a 200-byte name is a 223-byte
/// element the engine refuses: the case is REACHABLE, and `description_ref`
/// drops the key form above the length below rather than composing a load
/// failure.
fn description_key_ceiling(kind: &str) -> usize {
    LOCALISED_ELEMENT_CEILING - format!("{}-description.", kind).len()
}

/// One plan reaching all four of `append_localised`'s cases at once: a recipe
/// and a technology each carrying a note, one of each WITH a declared
/// `description` and one WITHOUT, plus an item that carries no note at all.
///
/// THE TWO KINDS ARE BOTH HERE BECAUSE THE SECTION IS THE PROTOTYPE'S OWN. A
/// composer that typed one kind in as a constant would satisfy a fixture
/// holding only recipes, so the assertions below name `recipe-description` on a
/// recipe and `technology-description` on a technology and would go red one at
/// a time.
fn note_fixture() -> (Lib, FixtureWorld) {
    let mut lib = Lib::new();

    // A recipe whose stored ingredient text the language refuses: a note, and
    // no description of its own.
    let bare = lib.item("bare-rivet", ItemSpec::default());
    let bare_parts = lib.ingredients_setting(
        "bare-ingredients",
        alloc::vec![Ingredient::named(1, "iron-plate", &[])],
    );
    lib.recipe(
        bare,
        RecipeSpec {
            name: String::from("bare-forging"),
            ingredients_from: Some(bare_parts),
            ..Default::default()
        },
    );

    // The same recipe WITH a description, which is the case that must not
    // compose a key: the author's literal already takes the entry's place.
    let described = lib.item("described-rivet", ItemSpec::default());
    let described_parts = lib.ingredients_setting(
        "described-ingredients",
        alloc::vec![Ingredient::named(1, "iron-plate", &[])],
    );
    lib.recipe(
        described,
        RecipeSpec {
            name: String::from("described-forging"),
            description: String::from("Forged from plate."),
            ingredients_from: Some(described_parts),
            ..Default::default()
        },
    );

    // A technology the game has no science pack for: a note, and no
    // description; and its described twin.
    lib.technology(
        "bare-riveting",
        TechSpec {
            unit: Some(UnitSpec {
                count: 10,
                seconds: 15.0,
                packs: alloc::vec![Pack::named(1, "space-science-pack", &[])],
            }),
            ..Default::default()
        },
    );
    lib.technology(
        "described-riveting",
        TechSpec {
            description: String::from("Teaches riveting."),
            unit: Some(UnitSpec {
                count: 10,
                seconds: 15.0,
                packs: alloc::vec![Pack::named(1, "space-science-pack", &[])],
            }),
            ..Default::default()
        },
    );

    // And the two prototypes NOTHING fell back on, which is what proves the
    // unchanged cases are unchanged.
    let quiet = lib.item(
        "quiet-plate",
        ItemSpec {
            description: String::from("An ordinary plate."),
            ..Default::default()
        },
    );
    lib.recipe(
        quiet,
        RecipeSpec {
            name: String::from("quiet-forging"),
            ingredients: alloc::vec![Ingredient::named(1, "iron-plate", &[])],
            ..Default::default()
        },
    );
    lib.technology(
        "quiet-research",
        TechSpec {
            unit: Some(UnitSpec {
                count: 10,
                seconds: 15.0,
                packs: alloc::vec![Pack::named(1, "automation-science-pack", &[])],
            }),
            ..Default::default()
        },
    );

    let w = base_world()
        .with_setting("steelworks-bare-ingredients", Value::string("1 unobtanium"))
        .with_setting(
            "steelworks-described-ingredients",
            Value::string("1 unobtanium"),
        );
    (lib, w)
}

/// Every prototype a plan emits, by emitted name.
fn emitted_protos(lib: &Lib, w: &FixtureWorld) -> Vec<(String, Value)> {
    let ops = lib.plan_data(w).expect("plan refused");
    let mut out = Vec::new();
    for op in &ops {
        if let Op::Extend(proto) = op {
            if let Some(Value::Str(name)) = field(proto, "name") {
                out.push((name, proto.clone()));
            }
        }
    }
    out
}

fn description_of(protos: &[(String, Value)], name: &str) -> Option<String> {
    let proto = protos
        .iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("the plan emitted no prototype named {}", name));
    field(&proto.1, "localised_description").map(|v| render_value(&v))
}

/// A NOTE WITH NO DECLARED DESCRIPTION REFERENCES THE AUTHOR'S OWN ENTRY, and
/// the section is the PROTOTYPE'S: a recipe composes `recipe-description` and a
/// technology `technology-description`.
#[test]
fn a_note_with_no_description_composes_the_prototypes_own_key() {
    let (lib, w) = note_fixture();
    let protos = emitted_protos(&lib, &w);

    for (proto, kind, note) in [
        (
            "steelworks-bare-forging",
            "recipe",
            crate::data::fallback_note("steelworks-bare-ingredients", true),
        ),
        (
            "steelworks-bare-riveting",
            "technology",
            String::from(PACKLESS_TOOLTIP),
        ),
    ] {
        let want = format!(
            r#"["", {}, {}]"#,
            description_ref_in(kind, proto),
            chunked_params(&note)
        );
        assert_eq!(
            description_of(&protos, proto).unwrap_or_default(),
            want,
            "{}",
            proto
        );
    }
}

/// A DECLARED DESCRIPTION BESIDE A NOTE COMPOSES NO KEY AT ALL, which is what
/// says the two cases did not get crossed. The author put their description in
/// the plan, so that literal IS their description.
#[test]
fn a_declared_description_beside_a_note_composes_no_key() {
    let (lib, w) = note_fixture();
    let protos = emitted_protos(&lib, &w);

    for (proto, description, note) in [
        (
            "steelworks-described-forging",
            "Forged from plate.",
            crate::data::fallback_note("steelworks-described-ingredients", true),
        ),
        (
            "steelworks-described-riveting",
            "Teaches riveting.",
            String::from(PACKLESS_TOOLTIP),
        ),
    ] {
        let want = format!(
            r#"["", "{}", {}]"#,
            description,
            chunked_params(&format!("\n{}", note))
        );
        let got = description_of(&protos, proto).unwrap_or_default();
        assert_eq!(got, want, "{}", proto);
        assert!(
            !got.contains("-description."),
            "{} composed a locale key beside the author's own literal: {}",
            proto,
            got
        );
    }
}

/// A PROTOTYPE WITH NO NOTE IS WHAT IT ALWAYS WAS, byte for byte, declared
/// description or not. A golden taken before this change must not move for a
/// load nothing fell back on.
#[test]
fn a_prototype_with_no_note_is_unchanged() {
    let (lib, w) = note_fixture();
    let protos = emitted_protos(&lib, &w);

    // A declared description with no note stays the two-element literal.
    assert_eq!(
        description_of(&protos, "steelworks-quiet-plate").unwrap_or_default(),
        r#"["", "An ordinary plate."]"#,
        "an item with a description and no note"
    );
    // And neither with a note nor a description emits the field at all, so the
    // engine resolves the author's own entry exactly as it always did.
    for name in ["steelworks-quiet-forging", "steelworks-quiet-research"] {
        assert_eq!(
            description_of(&protos, name),
            None,
            "{} emitted a localised_description with neither a description nor a note",
            name
        );
    }
}

/// THE KEY FORM IS COMPOSED UP TO THE ELEMENT CEILING AND DROPPED ABOVE IT, one
/// byte either side, on both kinds.
#[test]
fn the_description_key_is_dropped_where_it_would_not_fit() {
    for kind in ["recipe", "technology"] {
        let fits = description_key_ceiling(kind);
        for (what, bytes, want) in [
            ("the longest name whose key fits", fits, true),
            ("one byte more", fits + 1, false),
        ] {
            let name = pad_name("q", bytes);
            let composed = crate::settings::description_ref(kind, &name);
            assert_eq!(
                composed.is_some(),
                want,
                "{}, {}: a {}-byte name",
                kind,
                what,
                bytes
            );
            // EVERY STEP DOWN TO THE KEY IS UNCONDITIONAL, which is not a
            // style choice: nested `if let`s with no `else` make a differently
            // shaped return skip the assertion below and leave this test green
            // over the very thing it exists to measure. The Go twin indexes
            // straight down and panics on a wrong shape; these arms do the
            // same, and each names the value it actually got.
            let Some(composed) = composed else {
                continue;
            };
            let Value::Arr(outer) = &composed else {
                panic!(
                    "{}, {}: description_ref returned {:?}, not an array",
                    kind, what, composed
                );
            };
            let Value::Arr(group) = &outer[1] else {
                panic!(
                    "{}, {}: the wrapper's second slot is {:?}, not the concatenation group",
                    kind, what, outer[1]
                );
            };
            let Value::Arr(table) = &group[1] else {
                panic!(
                    "{}, {}: the group's second slot is {:?}, not the key table",
                    kind, what, group[1]
                );
            };
            let Value::Str(key) = &table[0] else {
                panic!(
                    "{}, {}: the key table holds {:?}, not a string key",
                    kind, what, table[0]
                );
            };
            assert_eq!(
                key.len(),
                LOCALISED_ELEMENT_CEILING,
                "{}, {}: the key is not exactly the ceiling",
                kind,
                what
            );
        }
        // AND THE FIXTURE PLAN'S OWN NAMES ARE ABOVE IT, which is what the
        // element walk above measures: at `FIXTURE_NAME_BYTES` every composed
        // key would be over the ceiling, so the walk sees the drop rather than
        // a refusal.
        assert!(
            FIXTURE_NAME_BYTES > description_key_ceiling(kind),
            "a {}-byte {} name composes a key that fits, so worst_case_plan no longer \
             reaches the drop arm at all",
            FIXTURE_NAME_BYTES,
            kind
        );
    }
}
