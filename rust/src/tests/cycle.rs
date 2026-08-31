use crate::cycle::CYCLE_PATH_CAP;
use crate::plan::{Ingredient, ItemSpec, Lib, Pack, RecipeSpec, TechSpec, UnitSpec};
use crate::tests::*;
use crate::value::Value;
use alloc::string::String;
use alloc::vec::Vec;

// The engine's own answer to a cycle is eight words with no name and no path.
// These are the tests that earn the library's answer.

fn axe_plan() -> Lib {
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
            cost_of: "steel-processing".into(),
            after: "steel-processing".into(),
            unlocks: vec![rec],
            ..Default::default()
        },
    );
    lib
}

#[test]
fn cycle_direct() {
    // Somebody's overhaul made two belt technologies require each other.
    let w = base_world().with_prereqs("logistics-2", &["logistics", "logistics-3"]);

    match axe_plan().plan_data(&w) {
        Ok(_) => panic!("the plan was accepted, want a cycle refusal"),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: at the data stage, a prerequisite cycle: logistics-2 -> logistics-3 -> logistics-2"
        ),
    }
}

#[test]
fn cycle_transitive_through_existing_edges() {
    // Three hops, none of them ours, and the path names all of them.
    let w = base_world().with_prereqs("logistics", &["logistics-3"]);

    match axe_plan().plan_data(&w) {
        Ok(_) => panic!("the plan was accepted, want a cycle refusal"),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: at the data stage, a prerequisite cycle: logistics -> logistics-3 -> logistics-2 -> logistics"
        ),
    }
}

/// The splice is the interesting one: the tree is sound until the plan's own
/// rewrite closes the ring, which is exactly what no other tool catches.
#[test]
fn cycle_created_by_insert_between() {
    // logistics-3 -> logistics-2 -> steel-processing, and nothing points
    // back: a sound tree.
    let w = base_world().with_prereqs("logistics-2", &["logistics", "steel-processing"]);

    let mut lib = Lib::new();
    // Splicing between logistics-3 and steel-processing makes
    // steel-processing require the new technology, which requires
    // logistics-3, which already leads back to steel-processing.
    lib.technology(
        "steel-axes",
        TechSpec {
            cost_of: "electronics".into(),
            after: "logistics-3".into(),
            before: "steel-processing".into(),
            ..Default::default()
        },
    );

    match lib.plan_data(&w) {
        Ok(_) => panic!("the plan was accepted, want a cycle refusal"),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: at the data stage, a prerequisite cycle: logistics-2 -> steel-processing -> steelworks-steel-axes -> logistics-3 -> logistics-2"
        ),
    }
}

/// The overlay must not invent a cycle out of a healthy splice: the edge the
/// rewrite REMOVES is gone from the walk, not just the ones it adds.
#[test]
fn no_cycle_for_a_healthy_splice() {
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

    let ops = lib.plan_data(&base_world()).expect("plan refused");

    assert_lines(
        &transcript(&ops),
        &[
            r#"extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}"#,
            r#"set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]"#,
        ],
    );
}

/// The blind spot the overwrite refusal closes: a planned name that already
/// exists put a SECOND node of that name in the overlay, the incoming edges
/// bound to the stale one, and the walk stepped past the ring.
#[test]
fn overwrite_refusal_closes_the_cycle_blind_spot() {
    // data.raw already carries a steelworks-widgetry that steel-processing
    // requires; the plan declares widgetry and splices it in front of
    // steel-processing, which closes a ring through the stale node.
    let w = base_world()
        .with_tech(FixtureTech {
            name: String::from("steelworks-widgetry"),
            prereqs: strings(&["logistics-3"]),
            unit: unit_of(50, 15.0, &["automation-science-pack"]),
            max_level: Value::Nil,
            trigger: false,
        })
        .with_prereqs("steel-processing", &["steelworks-widgetry"]);

    let mut lib = Lib::new();
    lib.technology(
        "widgetry",
        TechSpec {
            cost_of: "electronics".into(),
            after: "logistics-3".into(),
            before: "steel-processing".into(),
            ..Default::default()
        },
    );

    match lib.plan_data(&w) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => assert_eq!(
            got,
            "fkrecipes: at the data stage, the technology steelworks-widgetry already exists in data.raw; this plan would overwrite it"
        ),
    }
}

fn ring_tech_name(i: usize) -> String {
    let digits = [
        b'0' + (i / 100 % 10) as u8,
        b'0' + (i / 10 % 10) as u8,
        b'0' + (i % 10) as u8,
    ];
    let mut name = String::from("conveyor-tier-");
    for d in digits {
        name.push(d as char);
    }
    name
}

/// A ring of technologies, sorted by name, each requiring the next.
fn ring_world(n: usize) -> FixtureWorld {
    let mut w = FixtureWorld {
        mod_name: String::from("steelworks"),
        stage: String::from("data"),
        settings: Vec::new(),
        loose_max_level: Value::Nil,
        nil_unit_for: Vec::new(),
        nil_max_level_for: Vec::new(),
        recipes: Vec::new(),
        items: strings(&["automation-science-pack"]),
        techs: Vec::new(),
    };
    for i in 0..n {
        w.techs.push(FixtureTech {
            name: ring_tech_name(i),
            prereqs: alloc::vec![ring_tech_name((i + 1) % n)],
            unit: unit_of(10, 5.0, &["automation-science-pack"]),
            max_level: Value::Nil,
            trigger: false,
        });
    }
    w
}

/// A pathological tree can ring thousands of technologies together. The
/// refusal names the first hundred and says how many it did not name, so the
/// message stays something a person can read.
#[test]
fn cycle_path_is_capped() {
    let mut lib = Lib::new();
    lib.technology(
        "steel-axes",
        TechSpec {
            unit: Some(UnitSpec {
                count: 50,
                seconds: 15.0,
                packs: alloc::vec![Pack {
                    name: "automation-science-pack".into(),
                    amount: 1,
                }],
            }),
            ..Default::default()
        },
    );

    let err = match lib.plan_data(&ring_world(150)) {
        Ok(ops) => panic!("the plan was accepted with {} ops", ops.len()),
        Err(got) => got,
    };

    let mut named = Vec::with_capacity(CYCLE_PATH_CAP);
    for i in 0..CYCLE_PATH_CAP {
        named.push(ring_tech_name(i));
    }
    // 150 technologies on the stack plus the one that closes the ring, less
    // the hundred the message names.
    let want = alloc::format!(
        "fkrecipes: at the data stage, a prerequisite cycle: {} -> (and 51 more before it closes)",
        named.join(" -> ")
    );
    assert_eq!(err, want);
}
