use crate::cycle::CYCLE_PATH_CAP;
use crate::plan::{Ingredient, ItemSpec, Lib, Pack, RecipeSpec, TechSpec, UnitSpec};
use crate::tests::*;
use crate::value::Value;
use crate::world::{Named, World};
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
            "fkrecipes: a prerequisite cycle: logistics-2 -> logistics-3 -> logistics-2"
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
            "fkrecipes: a prerequisite cycle: logistics -> logistics-3 -> logistics-2 -> logistics"
        ),
    }
}

/// The splice is the interesting one: the tree is sound until the plan's own
/// rewrite closes the ring, which is exactly what no other tool catches.
///
/// AND THE SPLICE IS WHAT IS DROPPED, not the load. The ring holds an edge this
/// plan made, so the game keeps playing: steel-processing is left with the
/// prerequisite list it already had, the new technology is still emitted and
/// still requires logistics-3, and its own tooltip says what it lost.
#[test]
fn cycle_created_by_insert_between_drops_the_splice() {
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

    let ops = lib.plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: steel-axes: steel-processing does not require logistics-3, so the new technology is appended to its prerequisites",
            "log fkrecipes: ERROR: steel-axes: making it a prerequisite of steel-processing would loop this game's technology tree (logistics-2 -> steel-processing -> steelworks-steel-axes -> logistics-3 -> logistics-2), so the splice is dropped",
            &(String::from(r#"extend {type="technology", name="steelworks-steel-axes", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-steel-axes")
                + r#", "Making this research a prerequisite of steel-processing would loop this game's technology tree, so it was left out of it. The reason is in the log."], prerequisites=["logistics-3"], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}"#),
        ],
    );
}

/// THE WALK IS A LOOP AND NOT ONE PASS, because dropping one edge can leave a
/// second ring standing. Two technologies, two rings, two drops, and the plan
/// loads: termination is by construction, since every pass drops one edge this
/// plan made and the plan has finitely many.
#[test]
fn two_rings_are_both_resolved() {
    let mut lib = Lib::new();
    lib.technology(
        "aaa",
        TechSpec {
            cost_of: "electronics".into(),
            after: "logistics-2".into(),
            ..Default::default()
        },
    );
    lib.technology(
        "bbb",
        TechSpec {
            cost_of: "electronics".into(),
            after: "logistics-3".into(),
            ..Default::default()
        },
    );
    let w = base_world()
        .with_prereqs("logistics-2", &["steelworks-aaa"])
        .with_prereqs("logistics-3", &["steelworks-bbb"]);

    let ops = lib.plan_data(&w).expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: ERROR: aaa: requiring logistics-2 would loop this game's technology tree (logistics-2 -> steelworks-aaa -> logistics-2), so the prerequisite is dropped",
            "log fkrecipes: ERROR: bbb: requiring logistics-3 would loop this game's technology tree (logistics-3 -> steelworks-bbb -> logistics-3), so the prerequisite is dropped",
            &(String::from(r#"extend {type="technology", name="steelworks-aaa", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-aaa")
                + r#", "Requiring logistics-2 would loop this game's technology tree, so this research was left without that prerequisite. The reason is in the log."], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}"#),
            &(String::from(r#"extend {type="technology", name="steelworks-bbb", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-bbb")
                + r#", "Requiring logistics-3 would loop this game's technology tree, so this research was left without that prerequisite. The reason is in the log."], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}"#),
        ],
    );
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
            "fkrecipes: the technology steelworks-widgetry already exists in data.raw; this plan would overwrite it"
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
        settings: Vec::new(),
        loose_max_level: Value::Nil,
        entities: Vec::new(),
        nil_unit_for: Vec::new(),
        nil_max_level_for: Vec::new(),
        recipes: Vec::new(),
        items: strings(&["automation-science-pack"]),
        fluids: Vec::new(),
        tools: strings(&["automation-science-pack"]),
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
                packs: alloc::vec![Pack::new("automation-science-pack", 1)],
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
        "fkrecipes: a prerequisite cycle: {} -> (and 51 more before it closes)",
        named.join(" -> ")
    );
    assert_eq!(err, want);
}

/// A DROPPED SPLICE MUST NOT TAKE THE BASE GAME'S OWN EDGE WITH IT, which is
/// the one thing InsertBetween's rewrite makes easy to get wrong: a splice
/// REPLACES the technology it was inserted after, and a SECOND splice into the
/// same target builds its record on a list the anchor is already out of. Taking
/// the dropped name back out of that later record without putting the anchor
/// back emits a prerequisite list with an edge of somebody else's tree silently
/// missing from it.
///
/// THE SHAPE, and every piece of it is load-bearing. automation requires
/// electronics in the base tree. riveting splices BETWEEN them, so automation's
/// list becomes [riveting] and the anchor electronics is out of it. plating
/// splices into automation too and finds nothing to replace, so its record is
/// [riveting, plating]. forging splices into electronics and requires
/// automation, which is what closes the ring the first drop is about.
///
/// WHAT THE WALK DOES, in order: the ring automation -> riveting -> electronics
/// -> forging -> automation is found first and riveting's splice is the first
/// edge in it this plan owns, so it goes and electronics goes back into BOTH
/// records; the ring that is left, automation -> electronics -> forging ->
/// automation, costs forging's splice; and what is emitted is plating's record,
/// which must read [electronics, steelworks-plating]. That is exactly the list a
/// plan with neither dropped splice in it would have produced.
#[test]
fn a_dropped_splice_gives_the_anchor_back_to_the_records_built_on_it() {
    let ops = anchor_plan()
        .plan_data(&base_world())
        .expect("plan refused");
    assert_lines(
        &transcript(&ops),
        &[
            "log fkrecipes: forging: electronics does not require automation, so the new technology is appended to its prerequisites",
            "log fkrecipes: plating: automation does not require logistics, so the new technology is appended to its prerequisites",
            "log fkrecipes: ERROR: riveting: making it a prerequisite of automation would loop this game's technology tree (automation -> steelworks-riveting -> electronics -> steelworks-forging -> automation), so the splice is dropped",
            "log fkrecipes: ERROR: forging: making it a prerequisite of electronics would loop this game's technology tree (automation -> electronics -> steelworks-forging -> automation), so the splice is dropped",
            &(String::from(r#"extend {type="technology", name="steelworks-riveting", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-riveting")
                + r#", "Making this research a prerequisite of automation would loop this game's technology tree, so it was left out of it. The reason is in the log."], prerequisites=["electronics"], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}"#),
            &(String::from(r#"extend {type="technology", name="steelworks-forging", localised_description=["", "#)
                + &description_ref_in("technology", "steelworks-forging")
                + r#", "Making this research a prerequisite of electronics would loop this game's technology tree, so it was left out of it. The reason is in the log."], prerequisites=["automation"], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}"#),
            r#"extend {type="technology", name="steelworks-plating", prerequisites=["logistics"], unit={count=30, ingredients=[["automation-science-pack", 1]], time=15}}"#,
            // THE ASSERTION THE WHOLE TEST IS FOR. electronics is a prerequisite
            // of automation in the base game and no declaration here asked for
            // it to go; a drop that deleted the dropped name instead of
            // substituting the anchor emits ["steelworks-plating"] here.
            r#"set technology.automation.prerequisites = ["electronics", "steelworks-plating"]"#,
        ],
    );
}

/// The plan the test above and the probe below both run: three splices, two
/// rings and three passes.
fn anchor_plan() -> Lib {
    let mut lib = Lib::new();
    for (name, after, before) in [
        ("riveting", "electronics", "automation"),
        ("forging", "automation", "electronics"),
        ("plating", "logistics", "automation"),
    ] {
        lib.technology(
            name,
            TechSpec {
                cost_of: "electronics".into(),
                after: after.into(),
                before: before.into(),
                ..Default::default()
            },
        );
    }
    lib
}

/// Counts `tech_prereqs` questions. It is the only way to hold up "the World is
/// asked once": the walk's answer is the same either way, and what changes is
/// how many times the host was crossed to get it. A `Cell` because the trait
/// takes `&self`, which is what a host-side World is.
struct PrereqProbeWorld {
    inner: FixtureWorld,
    asked: core::cell::Cell<usize>,
}

impl Named for PrereqProbeWorld {
    fn mod_name(&self) -> String {
        self.inner.mod_name.clone()
    }
}

impl World for PrereqProbeWorld {
    fn tech_prereqs(&self, name: &str) -> Vec<String> {
        self.asked.set(self.asked.get() + 1);
        self.inner.tech_prereqs(name)
    }
    fn startup_setting(&self, name: &str) -> Option<Value> {
        self.inner.startup_setting(name)
    }
    fn tech_names(&self) -> Vec<String> {
        self.inner.tech_names()
    }
    fn tech_exists(&self, name: &str) -> bool {
        self.inner.tech_exists(name)
    }
    fn tech_unit(&self, name: &str) -> Option<Value> {
        self.inner.tech_unit(name)
    }
    fn tech_max_level(&self, name: &str) -> Option<Value> {
        self.inner.tech_max_level(name)
    }
    fn tech_has_research_trigger(&self, name: &str) -> bool {
        self.inner.tech_has_research_trigger(name)
    }
    fn item_exists(&self, name: &str) -> bool {
        self.inner.item_exists(name)
    }
    fn fluid_exists(&self, name: &str) -> bool {
        self.inner.fluid_exists(name)
    }
    fn tool_exists(&self, name: &str) -> bool {
        self.inner.tool_exists(name)
    }
    fn recipe_exists(&self, name: &str) -> bool {
        self.inner.recipe_exists(name)
    }
    fn entity_exists(&self, name: &str) -> bool {
        self.inner.entity_exists(name)
    }
}

/// THE WORLD IS ASKED ONCE PER TECHNOLOGY, HOWEVER MANY PASSES THE WALK TAKES.
/// Its prerequisite lists cannot move between passes, only the plan's rewrites
/// can, so re-asking is a host crossing per technology per pass for an answer
/// that is already in hand. The plan above takes THREE passes (two drops and
/// the clean walk that follows them), and the count must still be one per name.
#[test]
fn the_cycle_walk_asks_the_world_its_prerequisites_once() {
    let w = PrereqProbeWorld {
        inner: base_world(),
        asked: core::cell::Cell::new(0),
    };
    anchor_plan().plan_data(&w).expect("plan refused");

    // TWO ASKED BEFORE THE WALK RUNS AT ALL, and they are `current_prereqs`'
    // own reader rather than this one: resolve asks the game for automation's
    // list and for electronics' when it builds the first splice into each. The
    // SECOND splice into automation reads the record the first one left and
    // asks the game nothing, which is that reader's whole job.
    assert_eq!(w.asked.get(), w.tech_names().len() + 2);
}
