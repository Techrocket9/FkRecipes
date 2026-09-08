use alloc::string::String;
use alloc::vec::Vec;

use crate::value::{kv, Value};
use crate::world::{stage_kind, Named, StageKind, World};

/// The host stand-in for the game: a slice of base Factorio big enough to
/// exercise every branch, and small enough to read. Vectors, not hash maps,
/// for the same reason the library uses vectors.
pub(crate) struct FixtureWorld {
    pub(crate) mod_name: String,
    pub(crate) settings: Vec<(String, Value)>,
    pub(crate) items: Vec<String>,
    pub(crate) fluids: Vec<String>,
    pub(crate) tools: Vec<String>,
    pub(crate) recipes: Vec<String>,
    pub(crate) techs: Vec<FixtureTech>,

    pub(crate) entities: Vec<String>,
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

impl Named for FixtureWorld {
    fn mod_name(&self) -> String {
        self.mod_name.clone()
    }
}

impl World for FixtureWorld {
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

    fn entity_exists(&self, name: &str) -> bool {
        self.entities.iter().any(|e| e.as_str() == name)
    }

    fn item_exists(&self, name: &str) -> bool {
        self.items.iter().any(|it| it.as_str() == name)
    }

    // Both of these are DEFAULT methods on the trait, overridden here because
    // this fixture reaches them. A consumer's fixture that never declares a
    // fluid or a research cost keeps compiling without them, which is the
    // property the defaults exist for.
    fn fluid_exists(&self, name: &str) -> bool {
        self.fluids.iter().any(|f| f.as_str() == name)
    }

    fn tool_exists(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.as_str() == name)
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

    pub(crate) fn with_mod_name(mut self, name: &str) -> FixtureWorld {
        self.mod_name = String::from(name);
        self
    }

    pub(crate) fn with_tech(mut self, t: FixtureTech) -> FixtureWorld {
        self.techs.push(t);
        self
    }

    pub(crate) fn with_entity(mut self, name: &str) -> FixtureWorld {
        self.entities.push(String::from(name));
        self
    }

    pub(crate) fn with_item(mut self, name: &str) -> FixtureWorld {
        self.items.push(String::from(name));
        self
    }

    pub(crate) fn without_fluid(mut self, name: &str) -> FixtureWorld {
        self.fluids.retain(|f| f.as_str() != name);
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
        settings: Vec::new(),
        loose_max_level: Value::Nil,
        // One entity, so a place_result can resolve as well as fail. It is a
        // DERIVED type rather than a plain "entity": that is what the real
        // probe has to walk, and it is the type BBB's own item names.
        entities: strings(&["steel-chest"]),
        nil_unit_for: Vec::new(),
        nil_max_level_for: Vec::new(),
        recipes: strings(&["electronic-circuit", "iron-gear-wheel", "steel-plate"]),
        // The two fluids a fluid-taking recipe can name, and the science
        // packs the game treats as tools. A pack is an ITEM as well, which is
        // why the item list carries the same two names: the engine's tool
        // type is one of the 21 item types.
        fluids: strings(&["steam", "water"]),
        tools: strings(&["automation-science-pack", "logistic-science-pack"]),
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

/// What a settings-stage plan is handed. The settings stage runs before
/// data.raw exists and the planner asks it for nothing but the mod name, so
/// this is base_world under a name that says which plan is being held up to
/// the light at the call site.
pub(crate) fn settings_world() -> FixtureWorld {
    base_world()
}

/// THE STAGE DISPATCH, held in the pure half so a test can reach it.
///
/// The interesting half of this table is the bottom: the two stage names FkLua
/// leaves unwired today. `emit`'s first shape was "settings plans settings,
/// EVERYTHING ELSE plans data", which maps both of them to the data plan and
/// would run `plan_data` at a settings stage where data.raw does not exist.
/// This table is what makes the day they are wired a deliberate change rather
/// than a silent misroute.
#[test]
fn stage_kind_maps_every_name() {
    let cases: &[(&str, Option<StageKind>)] = &[
        ("settings", Some(StageKind::Settings)),
        ("data", Some(StageKind::Data)),
        ("data-updates", Some(StageKind::Data)),
        ("data-final-fixes", Some(StageKind::Data)),
        // fkdata's own name for an id it does not recognise.
        ("unknown", None),
        // Deliberately unwired upstream. Neither is a data stage, and neither
        // is one this crate plans for until somebody decides what it means.
        ("settings-updates", None),
        ("settings-final-fixes", None),
        // Not a stage at all.
        ("", None),
        ("Data", None),
        ("data ", None),
    ];
    for (name, want) in cases {
        assert_eq!(stage_kind(name), *want, "stage_kind({:?})", name);
    }
}

/// Every stage fkdata can name is either planned for or refused BY NAME, with
/// nothing falling through a default. This is the anti-vacuity half: the table
/// above could be trimmed to two rows and still pass, and this could not.
#[test]
fn every_fkdata_stage_name_is_decided() {
    // The names fkdata's StageId::name returns, read from the guest crate this
    // library depends on. A name added there and not here is the gap this test
    // exists to find.
    for name in [
        "settings",
        "data",
        "data-updates",
        "data-final-fixes",
        "unknown",
    ] {
        let got = stage_kind(name);
        if name == "unknown" {
            assert!(
                got.is_none(),
                "stage_kind({:?}) plans for a stage fkdata could not identify",
                name
            );
            continue;
        }
        let want = if name == "settings" {
            StageKind::Settings
        } else {
            StageKind::Data
        };
        assert_eq!(
            got,
            Some(want),
            "stage_kind({:?}) does not route to the plan that stage calls for",
            name
        );
    }
}

/// THE SENTENCE THE EMIT LAYER RAISES FOR A NAME IT CANNOT READ AS TEXT.
///
/// It lives in the pure half for the reason `stage_kind` does: the only caller
/// is wasm-gated, and a message written there is one no test can reach. This
/// is the whole line a player would see under the host's stage prefix.
///
/// THE HEX IS THE POINT of the rendering. The bytes are what a reader has to
/// go and look for in another mod's source, a terminal makes what it likes of
/// them, and a lossy rewrite would print a length that is not the value's.
/// fkdata's own `text` helper refuses the same way at the surfaces the ENGINE
/// constrains, which is where the shape comes from.
#[test]
fn the_not_text_refusal_names_the_surface_and_prints_the_bytes() {
    assert_eq!(
        crate::value::not_text("a technology name in data.raw", &[0x6b, 0x2d, 0x80]),
        "fkrecipes: a technology name in data.raw is not valid UTF-8, and this library reads it as text rather than rewriting it: the bytes are 6b2d80"
    );
    // Every nibble, so a table indexed the wrong way round cannot pass, and
    // the empty case, which is what an empty prototype name would print.
    assert_eq!(
        crate::value::not_text(
            "a prerequisite of the technology steel-processing",
            &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
        ),
        "fkrecipes: a prerequisite of the technology steel-processing is not valid UTF-8, and this library reads it as text rather than rewriting it: the bytes are 0123456789abcdef"
    );
    assert_eq!(
        crate::value::not_text("a technology name in data.raw", &[]),
        "fkrecipes: a technology name in data.raw is not valid UTF-8, and this library reads it as text rather than rewriting it: the bytes are "
    );
}
