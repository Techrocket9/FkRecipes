use alloc::string::String;
use alloc::vec::Vec;

use crate::value::{kv, Value};
use crate::world::World;

/// The host stand-in for the game: a slice of base Factorio big enough to
/// exercise every branch, and small enough to read. Vectors, not hash maps,
/// for the same reason the library uses vectors.
pub(crate) struct FixtureWorld {
    pub(crate) mod_name: String,
    pub(crate) stage: String,
    pub(crate) settings: Vec<(String, Value)>,
    pub(crate) items: Vec<String>,
    pub(crate) recipes: Vec<String>,
    pub(crate) techs: Vec<FixtureTech>,

    pub(crate) nil_unit_for: Vec<String>,
    pub(crate) nil_max_level_for: Vec<String>,

    /// A max_level answered for ANY name, even one no technology carries.
    /// Some Worlds are loose about lookups; the planner must not ask a
    /// question it has no source technology for.
    pub(crate) loose_max_level: Value,
}

pub(crate) struct FixtureTech {
    pub(crate) name: String,
    pub(crate) prereqs: Vec<String>,
    /// `Value::Nil` means the technology carries no unit.
    pub(crate) unit: Value,
    /// `Value::Nil` means the technology has no level cap.
    pub(crate) max_level: Value,
    pub(crate) trigger: bool,
}

impl World for FixtureWorld {
    fn mod_name(&self) -> String {
        self.mod_name.clone()
    }

    fn stage_name(&self) -> String {
        self.stage.clone()
    }

    fn startup_setting(&self, name: &str) -> Option<Value> {
        for (key, val) in &self.settings {
            if key.as_str() == name {
                return Some(val.clone());
            }
        }
        None
    }

    fn tech_names(&self) -> Vec<String> {
        self.techs.iter().map(|t| t.name.clone()).collect()
    }

    fn tech_prereqs(&self, name: &str) -> Vec<String> {
        for t in &self.techs {
            if t.name.as_str() == name {
                return t.prereqs.clone();
            }
        }
        Vec::new()
    }

    fn tech_unit(&self, name: &str) -> Option<Value> {
        // PRESENT and nil: the exact shape from_v produces for a unit whose
        // table carried a numeric key, which is a different answer from "no
        // unit".
        if self.nil_unit_for.iter().any(|n| n.as_str() == name) {
            return Some(Value::Nil);
        }
        for t in &self.techs {
            if t.name.as_str() == name && t.unit != Value::Nil {
                return Some(t.unit.clone());
            }
        }
        None
    }

    fn tech_max_level(&self, name: &str) -> Option<Value> {
        // A read that is PRESENT and nil: what a LuaObject or a table this
        // library cannot carry collapses to on the way in.
        if self.nil_max_level_for.iter().any(|n| n.as_str() == name) {
            return Some(Value::Nil);
        }
        for t in &self.techs {
            if t.name.as_str() == name && t.max_level != Value::Nil {
                return Some(t.max_level.clone());
            }
        }
        if self.loose_max_level != Value::Nil {
            return Some(self.loose_max_level.clone());
        }
        None
    }

    fn tech_has_research_trigger(&self, name: &str) -> bool {
        for t in &self.techs {
            if t.name.as_str() == name {
                return t.trigger;
            }
        }
        false
    }

    fn tech_exists(&self, name: &str) -> bool {
        self.techs.iter().any(|t| t.name.as_str() == name)
    }

    fn item_exists(&self, name: &str) -> bool {
        self.items.iter().any(|it| it.as_str() == name)
    }

    fn recipe_exists(&self, name: &str) -> bool {
        self.recipes.iter().any(|r| r.as_str() == name)
    }
}

// Mutators, so a test says what it changed about the world instead of
// restating the whole world.
impl FixtureWorld {
    pub(crate) fn without_tech(mut self, name: &str) -> FixtureWorld {
        self.techs.retain(|t| t.name.as_str() != name);
        self
    }

    pub(crate) fn without_item(mut self, name: &str) -> FixtureWorld {
        self.items.retain(|it| it.as_str() != name);
        self
    }

    pub(crate) fn with_prereqs(mut self, name: &str, prereqs: &[&str]) -> FixtureWorld {
        for t in self.techs.iter_mut() {
            if t.name.as_str() == name {
                t.prereqs = strings(prereqs);
            }
        }
        self
    }

    pub(crate) fn with_setting(mut self, name: &str, v: Value) -> FixtureWorld {
        self.settings.push((String::from(name), v));
        self
    }

    pub(crate) fn with_nil_unit(mut self, name: &str) -> FixtureWorld {
        self.nil_unit_for.push(String::from(name));
        self
    }

    pub(crate) fn with_nil_max_level(mut self, name: &str) -> FixtureWorld {
        self.nil_max_level_for.push(String::from(name));
        self
    }

    pub(crate) fn with_unit(mut self, name: &str, v: Value) -> FixtureWorld {
        for t in self.techs.iter_mut() {
            if t.name.as_str() == name {
                t.unit = v.clone();
            }
        }
        self
    }

    pub(crate) fn with_stage(mut self, name: &str) -> FixtureWorld {
        self.stage = String::from(name);
        self
    }

    pub(crate) fn with_mod_name(mut self, name: &str) -> FixtureWorld {
        self.mod_name = String::from(name);
        self
    }

    pub(crate) fn with_tech(mut self, t: FixtureTech) -> FixtureWorld {
        self.techs.push(t);
        self
    }

    pub(crate) fn with_item(mut self, name: &str) -> FixtureWorld {
        self.items.push(String::from(name));
        self
    }

    pub(crate) fn with_recipe(mut self, name: &str) -> FixtureWorld {
        self.recipes.push(String::from(name));
        self
    }

    pub(crate) fn answering_max_level_for_any_name(mut self, v: Value) -> FixtureWorld {
        self.loose_max_level = v;
        self
    }

    pub(crate) fn with_max_level(mut self, name: &str, v: Value) -> FixtureWorld {
        for t in self.techs.iter_mut() {
            if t.name.as_str() == name {
                t.max_level = v.clone();
            }
        }
        self
    }
}

pub(crate) fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| String::from(*s)).collect()
}

fn tech(name: &str, prereqs: &[&str], unit: Value) -> FixtureTech {
    FixtureTech {
        name: String::from(name),
        prereqs: strings(prereqs),
        unit,
        max_level: Value::Nil,
        trigger: false,
    }
}

pub(crate) fn unit_of(count: i64, seconds: f64, packs: &[&str]) -> Value {
    let ings = packs
        .iter()
        .map(|p| Value::Arr(alloc::vec![Value::string(p), Value::Num(1.0)]))
        .collect();
    // Sorted by key, because that is the order a unit comes back in: fkdata
    // sorts every dictionary at every level on the way out, so a plan reading
    // data.raw never sees authoring order. What this library EMITS is
    // pre-sort, which is why a hand-rolled unit's golden is count, time,
    // ingredients while a copied one's is alphabetical.
    Value::Map(alloc::vec![
        kv("count", Value::Num(count as f64)),
        kv("ingredients", Value::Arr(ings)),
        kv("time", Value::Num(seconds)),
    ])
}

/// A fresh copy per test: the mutators above rewrite it in place. Technology
/// names are SORTED, which `World::tech_names` promises its caller.
pub(crate) fn base_world() -> FixtureWorld {
    FixtureWorld {
        mod_name: String::from("steelworks"),
        stage: String::from("data"),
        settings: Vec::new(),
        loose_max_level: Value::Nil,
        nil_unit_for: Vec::new(),
        nil_max_level_for: Vec::new(),
        recipes: strings(&["electronic-circuit", "iron-gear-wheel", "steel-plate"]),
        items: strings(&[
            "automation-science-pack",
            "chemical-science-pack",
            "copper-plate",
            "electronic-circuit",
            "iron-gear-wheel",
            "iron-plate",
            "logistic-science-pack",
            "steel-plate",
        ]),
        techs: alloc::vec![
            tech(
                "automation",
                &["electronics"],
                unit_of(250, 30.0, &["automation-science-pack"])
            ),
            tech(
                "electronics",
                &[],
                unit_of(30, 15.0, &["automation-science-pack"])
            ),
            tech(
                "logistics",
                &[],
                unit_of(20, 15.0, &["automation-science-pack"])
            ),
            tech(
                "logistics-2",
                &["logistics", "automation"],
                unit_of(
                    200,
                    30.0,
                    &["automation-science-pack", "logistic-science-pack"]
                )
            ),
            tech(
                "logistics-3",
                &["logistics-2"],
                unit_of(
                    400,
                    60.0,
                    &[
                        "automation-science-pack",
                        "logistic-science-pack",
                        "chemical-science-pack"
                    ]
                )
            ),
            // An infinite technology, the cost_of-verbatim case.
            // count_formula is a string, so copying it needs no evaluator, and
            // the level cap sits on the technology beside the unit, not
            // inside it.
            FixtureTech {
                name: String::from("mining-productivity-4"),
                prereqs: strings(&["logistics-3"]),
                max_level: Value::string("infinite"),
                trigger: false,
                unit: Value::Map(alloc::vec![
                    kv("count_formula", Value::string("2^(L-4)*1000")),
                    kv(
                        "ingredients",
                        Value::Arr(alloc::vec![
                            Value::Arr(alloc::vec![
                                Value::string("automation-science-pack"),
                                Value::Num(1.0)
                            ]),
                            Value::Arr(alloc::vec![
                                Value::string("logistic-science-pack"),
                                Value::Num(1.0)
                            ]),
                            Value::Arr(alloc::vec![
                                Value::string("chemical-science-pack"),
                                Value::Num(1.0)
                            ]),
                        ])
                    ),
                    // A key this library has never heard of, left on the unit
                    // by whichever mod owns the source technology. The copy
                    // carries it through untouched, because the copy is a copy
                    // and not a rebuild from the fields the planner knows.
                    kv("mod_cost_tier", Value::string("mid-game")),
                    kv("time", Value::Num(60.0)),
                ]),
            },
            // A research_trigger technology: no unit at all, which is the
            // measured crash class cost_of has to refuse.
            FixtureTech {
                name: String::from("steam-power"),
                prereqs: Vec::new(),
                unit: Value::Nil,
                max_level: Value::Nil,
                trigger: true,
            },
            tech(
                "steel-processing",
                &[],
                unit_of(50, 15.0, &["automation-science-pack"])
            ),
        ],
    }
}

#[test]
fn fixture_tech_names_are_sorted() {
    let names = base_world().tech_names();
    for i in 1..names.len() {
        assert!(
            names[i - 1] < names[i],
            "fixture technology names are not sorted: {} then {}",
            names[i - 1],
            names[i]
        );
    }
}

/// base_world at the OTHER stage. The settings stage runs before data.raw
/// exists, and the planner asks it for nothing but the mod name and the stage
/// name.
pub(crate) fn settings_world() -> FixtureWorld {
    base_world().with_stage("settings")
}
