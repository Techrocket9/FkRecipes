//! THE PUBLIC SURFACE, FROM OUTSIDE IT.
//!
//! Everything in `src/tests/` is inside the crate and can reach a private
//! module; a consumer cannot. This file is a separate crate that depends on
//! `fkrecipes` the way a mod does, so what it compiles against is exactly what
//! is exported, and a surface that is missing a piece fails to build here
//! rather than in somebody's mod.
//!
//! WHAT IT IS THE WITNESS FOR. `World` has a supertrait, `Named`. `mod world`
//! is private, so for as long as the crate root re-exported `World` without
//! `Named` the trait was SEALED BY ACCIDENT: this file could name `World`, and
//! implementing it failed with "the trait bound `Fixture: fkrecipes::world::
//! Named` is not satisfied ... `World` is a sealed trait". docs/usage.md
//! promises that a fixture World of your own works in Rust, and this is the
//! test that says so.
//!
//! It drives a whole plan rather than only implementing the traits, because a
//! consumer's reason for writing a fixture is to hold a declaration up to the
//! light: the plan below reads a text setting the fixture answers, once with
//! bytes that are not text and once with a list, and checks both what the
//! library refuses and what it emits.

use fkrecipes::{Ingredient, ItemSpec, Lib, Named, Op, RecipeSpec, Value, World};

/// A consumer's own stand-in for the game: the two presence questions this
/// plan actually asks, one stored setting, and a panic for everything else.
/// That is the shape the trait's additive-growth note is designed around, and
/// writing it is the thing being tested.
struct Fixture {
    /// The value `startup_setting` answers for this plan's one text setting.
    stored: Option<Value>,
}

impl Named for Fixture {
    fn mod_name(&self) -> String {
        String::from("mymod")
    }
}

impl World for Fixture {
    fn startup_setting(&self, name: &str) -> Option<Value> {
        if name == "mymod-parts" {
            return self.stored.clone();
        }
        None
    }

    fn item_exists(&self, name: &str) -> bool {
        name == "iron-plate" || name == "copper-cable"
    }

    fn recipe_exists(&self, _name: &str) -> bool {
        false
    }

    fn tech_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn tech_prereqs(&self, _name: &str) -> Vec<String> {
        Vec::new()
    }

    fn tech_unit(&self, _name: &str) -> Option<Value> {
        None
    }

    fn tech_max_level(&self, _name: &str) -> Option<Value> {
        None
    }

    fn tech_has_research_trigger(&self, _name: &str) -> bool {
        false
    }

    fn tech_exists(&self, _name: &str) -> bool {
        false
    }

    fn entity_exists(&self, _name: &str) -> bool {
        false
    }
}

/// One item, one recipe, and the text setting that prices it: the smallest
/// plan that reads a player's text.
fn plan() -> Lib {
    let mut lib = Lib::new();
    let widget = lib.item("widget", ItemSpec::default());
    let parts = lib.ingredients_setting("parts", vec![Ingredient::named(1, "iron-plate", &[])]);
    lib.recipe(
        widget,
        RecipeSpec {
            ingredients_from: Some(parts),
            ..Default::default()
        },
    );
    lib
}

/// The value under a key of a prototype map. A consumer matching on `Value`
/// writes the wildcard arm `#[non_exhaustive]` asks for, which is what this
/// helper is also demonstrating.
fn field(v: &Value, key: &str) -> Option<Value> {
    match v {
        Value::Map(pairs) => pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, val)| val.clone()),
        _ => None,
    }
}

/// BYTES THAT ARE NOT TEXT TAKE THE LANGUAGE'S REFUSAL, reached from outside
/// the crate. A fixture may answer `Value::Bytes` because the engine really
/// can hold such a value, and the sentence a consumer sees in their own test
/// is the sentence a player would see under the host's stage prefix.
#[test]
fn a_fixture_world_of_your_own_drives_a_plan() {
    let w = Fixture {
        stored: Some(Value::bytes(b"2 iron-\xffplate")),
    };
    assert_eq!(
        plan().plan_data(&w).err().as_deref(),
        Some("fkrecipes: mymod-parts contains characters that are not text; retype the list")
    );
}

/// AND A LIST THE FIXTURE ANSWERS REACHES THE OP STREAM. The same plan, the
/// same fixture, a stored value that parses: the recipe the library emits is
/// made of what the text said, which is the thing a consumer writes a fixture
/// to see.
#[test]
fn a_fixture_worlds_answer_reaches_the_ops() {
    let w = Fixture {
        stored: Some(Value::string("3 iron-plate, 2 copper-cable")),
    };
    let ops = plan().plan_data(&w).expect("the plan refused");

    let mut recipes = 0usize;
    for op in &ops {
        let proto = match op {
            Op::Extend(proto) => proto,
            _ => continue,
        };
        if field(proto, "type") != Some(Value::string("recipe")) {
            continue;
        }
        recipes += 1;
        assert_eq!(field(proto, "name"), Some(Value::string("mymod-widget")));
        assert_eq!(
            field(proto, "ingredients"),
            Some(Value::arr(vec![
                Value::Map(vec![
                    (String::from("type"), Value::string("item")),
                    (String::from("name"), Value::string("iron-plate")),
                    (String::from("amount"), Value::num(3.0)),
                ]),
                Value::Map(vec![
                    (String::from("type"), Value::string("item")),
                    (String::from("name"), Value::string("copper-cable")),
                    (String::from("amount"), Value::num(2.0)),
                ]),
            ])),
            "the fixture's stored list did not reach the recipe"
        );
    }
    assert_eq!(recipes, 1, "the plan emitted no recipe to check");
}
