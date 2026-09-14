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
    chunk_localised, Value, LOCALISED_CHUNK_BUDGET, LOCALISED_ELEMENT_CEILING, MAX_ITEM_AMOUNT,
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
        described, 16,
        "the fixture emitted {} localised_description fields, not the 16 it declares; \
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
/// SIXTEEN DESCRIPTIONS, and the count is asserted above:
///
/// - an item with a display name and a description;
/// - a recipe carrying the FALLBACK note, with a description and without;
/// - a recipe carrying the clamped ITEM note, with and without;
/// - a recipe carrying the clamped FLUID note, with and without;
/// - a technology carrying the FALLBACK note, with and without;
/// - a technology carrying the DROPPED-PACK note, with and without;
/// - a technology carrying the PACKLESS-SOURCE note, with and without;
/// - a technology carrying the clamped PACK note, with and without;
/// - and an item whose description alone is long enough to NEST.
///
/// THE PAIRS ARE PAIRS BECAUSE THE COMPOSITION IS WHAT IS WALKED, not the note:
/// a note beside an author's description and a note alone are two different
/// localised strings, and only one of them can nest. That is why a seventh note
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
        });

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
    (lib, w)
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
                String::from("The stored value of steelworks-rivet-ingredients could not be used, so this mod's own choice applies instead. The reason is in the log. Changing a recipe empties an assembling "),
                String::from("machine's input slots of anything the new list does not use."),
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
