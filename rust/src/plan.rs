use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

/// Stamps each plan with an identity so a handle carries which plan it came
/// from. An atomic because a `static mut` needs unsafe for the same job;
/// nothing here reaches an op, so it cannot make a plan non-deterministic.
static NEXT_LIB_ID: AtomicU64 = AtomicU64::new(0);

/// One mod's plan. Everything the declaration methods record is an ordinary
/// value in declaration order; nothing is validated, resolved or emitted
/// until `plan_settings` or `plan_data` runs.
/// Built only by [`Lib::new`]. There is deliberately no `Default`: a plan
/// with no id would share that id with every other such plan, which is
/// exactly the cross-plan handle mix-up the id exists to catch. Dropping the
/// derive makes that state unconstructible from safe code outside this crate,
/// and both planning entry points still refuse it, because the Go mirror
/// cannot make a zero struct unrepresentable and the two halves refuse alike.
pub struct Lib {
    pub(crate) id: u64,
    pub(crate) settings: Vec<SettingDecl>,
    pub(crate) items: Vec<ItemDecl>,
    pub(crate) recipes: Vec<RecipeDecl>,
    pub(crate) techs: Vec<TechDecl>,
}

// The handles. Each carries the id of the plan that issued it and a 1-BASED
// index into that plan's matching declaration vector, so a defaulted handle
// means absent, exactly as the Go mirror's zero value does. A consumer cannot
// build one from a number, and a handle borrowed from ANOTHER plan is refused
// rather than silently resolved: two plans both have an index 1, and without
// the id the second plan would emit the first plan's item under its own name.
macro_rules! handle {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
        pub struct $name {
            pub(crate) lib: u64,
            pub(crate) index: usize,
        }
    };
}

handle!(
    /// A declared bool setting, the only handle `TechSpec` takes.
    BoolSettingRef
);
handle!(
    /// A declared int setting. Nothing in the pure planner reads it back yet;
    /// it is the handle the crafting-time binding will take.
    IntSettingRef
);
handle!(
    /// A declared double setting. See [`IntSettingRef`].
    DoubleSettingRef
);
handle!(
    /// A declared dropdown setting. See [`IntSettingRef`].
    DropdownSettingRef
);
handle!(
    /// An item this plan declares.
    ItemRef
);
handle!(
    /// A recipe this plan declares.
    RecipeRef
);
handle!(
    /// A technology this plan declares.
    TechRef
);

/// Bounds for an int or double setting. Both are optional: a defaulted
/// `NumericSpec` is an unbounded setting, not one pinned to zero.
#[derive(Clone, Copy, Default)]
pub struct NumericSpec {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl NumericSpec {
    /// The common case, both bounds given.
    pub fn between(low: f64, high: f64) -> NumericSpec {
        NumericSpec {
            min: Some(low),
            max: Some(high),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum SettingKind {
    Bool,
    Int,
    Double,
    Dropdown,
}

pub(crate) struct SettingDecl {
    pub(crate) kind: SettingKind,
    pub(crate) name: String,
    pub(crate) def_bool: bool,
    pub(crate) def_num: f64,
    /// The int setting's default as it was DECLARED. `def_num` has already
    /// been through f64 by the time validation runs, so the one number that
    /// could have rounded on the way in is no longer there to check.
    pub(crate) def_int: i64,
    pub(crate) def_str: String,
    pub(crate) spec: NumericSpec,
    pub(crate) values: Vec<String>,
}

/// A generated item prototype.
#[derive(Clone, Default)]
pub struct ItemSpec {
    pub icon: String,
    /// Zero omits the field; the engine's own default is 64.
    pub icon_size: i64,
    /// Zero means 50.
    pub stack_size: i64,
    pub subgroup: String,
    pub display_name: String,
    pub description: String,
}

pub(crate) struct ItemDecl {
    pub(crate) name: String,
    pub(crate) spec: ItemSpec,
}

/// One line of a recipe. It is built by [`Ingredient::of`] or
/// [`Ingredient::named`] and cannot be built from a bare string any other
/// way: an ingredient the game does not have is a hard load failure naming
/// the consumer's mod, so a name reaches a prototype only after a presence
/// probe.
#[derive(Clone)]
pub struct Ingredient {
    pub(crate) item: ItemRef,
    pub(crate) amount: i64,
    pub(crate) candidates: Vec<String>,
}

impl Ingredient {
    /// Names an item this plan declares. It always resolves: the prototype is
    /// emitted by the same plan.
    pub fn of(it: ItemRef, amount: i64) -> Ingredient {
        Ingredient {
            item: it,
            amount,
            candidates: Vec::new(),
        }
    }

    /// The presence ladder: the candidates are tried in order and the first
    /// one the game actually has is used. If none is present the ingredient
    /// is DROPPED with a log line, never guessed at, because a wrong guess is
    /// somebody else's overhaul pack failing to load.
    pub fn named(amount: i64, first: &str, fallbacks: &[&str]) -> Ingredient {
        let mut candidates = Vec::with_capacity(1 + fallbacks.len());
        candidates.push(String::from(first));
        for f in fallbacks {
            candidates.push(String::from(*f));
        }
        Ingredient {
            item: ItemRef::default(),
            amount,
            candidates,
        }
    }
}

/// A generated recipe prototype.
#[derive(Clone, Default)]
pub struct RecipeSpec {
    /// Empty means the result item's name.
    pub name: String,

    /// Exactly one of `craft_time` and `craft_time_from`, or neither: a fixed
    /// crafting time, or one the PLAYER sets through a generated double
    /// setting. Zero and a defaulted handle both mean "say nothing", and the
    /// engine applies its own default.
    ///
    /// ONLY A DOUBLE SETTING IS BINDABLE, and the handle's type is what says
    /// so: there is no int-setting arm to get wrong, because an
    /// `IntSettingRef` does not fit here. That is the same shape `enabled_by`
    /// uses for a bool.
    pub craft_time: f64,
    pub craft_time_from: DoubleSettingRef,
    pub ingredients: Vec<Ingredient>,
    /// Zero means 1.
    pub result_count: i64,
    pub category: String,
    pub display_name: String,
    pub description: String,
}

pub(crate) struct RecipeDecl {
    pub(crate) name: String,
    pub(crate) result: ItemRef,
    pub(crate) spec: RecipeSpec,
}

/// One science pack of a hand-rolled research cost.
#[derive(Clone, Default)]
pub struct Pack {
    pub name: String,
    pub amount: i64,
}

/// The hand-rolled research cost, the ESCAPE HATCH. Prefer `cost_of`: it
/// takes cost and tree position from one named technology, which is the rule
/// this surface is built to make easy.
#[derive(Clone, Default)]
pub struct UnitSpec {
    pub count: i64,
    pub seconds: f64,
    pub packs: Vec<Pack>,
}

/// A generated technology prototype.
#[derive(Clone, Default)]
pub struct TechSpec {
    pub icon: String,
    pub icon_size: i64,

    /// Exactly one of `cost_of` and `unit`. `cost_of` copies a named
    /// technology's whole unit verbatim, count_formula and all.
    pub cost_of: String,
    pub unit: Option<UnitSpec>,

    /// Tree placement, and exactly one anchor. `after` names a technology the
    /// GAME has; `after_tech` names one THIS PLAN declares, which is how a
    /// plan chains its own research. `after` with `before` splices the new
    /// technology between the two. `before` needs `after`: it splices around
    /// technologies that already exist, so it has nothing to say about a
    /// plan's own.
    pub after: String,
    /// A defaulted handle is no anchor.
    pub after_tech: TechRef,
    pub before: String,

    pub unlocks: Vec<RecipeRef>,
    /// A defaulted handle is no setting at all.
    pub enabled_by: BoolSettingRef,

    pub display_name: String,
    pub description: String,
}

pub(crate) struct TechDecl {
    pub(crate) name: String,
    pub(crate) spec: TechSpec,
}

impl Lib {
    /// Starts an empty plan. The only way to get one.
    // A Default is exactly what must not exist here: it would hand out plans
    // with id 0, which all compare equal and so cross-resolve each other's
    // handles.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Lib {
        Lib {
            id: NEXT_LIB_ID.fetch_add(1, Ordering::Relaxed) + 1,
            settings: Vec::new(),
            items: Vec::new(),
            recipes: Vec::new(),
            techs: Vec::new(),
        }
    }

    /// Declares a startup bool setting. The name is prefixed on the way out;
    /// what is passed here is the bare name.
    pub fn bool_setting(&mut self, name: &str, def: bool) -> BoolSettingRef {
        self.settings.push(SettingDecl {
            kind: SettingKind::Bool,
            name: String::from(name),
            def_bool: def,
            def_num: 0.0,
            def_int: 0,
            def_str: String::new(),
            spec: NumericSpec::default(),
            values: Vec::new(),
        });
        BoolSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares a startup int setting.
    pub fn int_setting(&mut self, name: &str, def: i64, spec: NumericSpec) -> IntSettingRef {
        self.settings.push(SettingDecl {
            kind: SettingKind::Int,
            name: String::from(name),
            def_bool: false,
            def_num: def as f64,
            def_int: def,
            def_str: String::new(),
            spec,
            values: Vec::new(),
        });
        IntSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares a startup double setting.
    pub fn double_setting(&mut self, name: &str, def: f64, spec: NumericSpec) -> DoubleSettingRef {
        self.settings.push(SettingDecl {
            kind: SettingKind::Double,
            name: String::from(name),
            def_bool: false,
            def_num: def,
            def_int: 0,
            def_str: String::new(),
            spec,
            values: Vec::new(),
        });
        DoubleSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares a startup string setting with a fixed list of allowed values.
    ///
    /// THE NAME IS THE WARNING. A bool, int or double setting localises from
    /// its own name; a dropdown's VALUES have no inline mechanism at all, so
    /// every entry in `values` needs a locale line in the CONSUMER's own .cfg
    /// or the player sees a raw key in the settings screen. Nothing this
    /// library emits can supply them. Prefer a bool, int or double setting
    /// when the choice fits one; reach for this when it does not, and ship
    /// the locale entries.
    pub fn dropdown_setting_needing_locale(
        &mut self,
        name: &str,
        def: &str,
        values: &[&str],
    ) -> DropdownSettingRef {
        let mut allowed = Vec::with_capacity(values.len());
        for v in values {
            allowed.push(String::from(*v));
        }
        self.settings.push(SettingDecl {
            kind: SettingKind::Dropdown,
            name: String::from(name),
            def_bool: false,
            def_num: 0.0,
            def_int: 0,
            def_str: String::from(def),
            spec: NumericSpec::default(),
            values: allowed,
        });
        DropdownSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares an item this mod introduces.
    pub fn item(&mut self, name: &str, spec: ItemSpec) -> ItemRef {
        self.items.push(ItemDecl {
            name: String::from(name),
            spec,
        });
        ItemRef {
            lib: self.id,
            index: self.items.len(),
        }
    }

    /// Declares a recipe producing an item this plan declares.
    pub fn recipe(&mut self, result: ItemRef, spec: RecipeSpec) -> RecipeRef {
        let mut name = spec.name.clone();
        if name.is_empty() && self.valid_item(result) {
            name = self.items[result.index - 1].name.clone();
        }
        self.recipes.push(RecipeDecl { name, result, spec });
        RecipeRef {
            lib: self.id,
            index: self.recipes.len(),
        }
    }

    /// Declares a technology this mod introduces.
    pub fn technology(&mut self, name: &str, spec: TechSpec) -> TechRef {
        self.techs.push(TechDecl {
            name: String::from(name),
            spec,
        });
        TechRef {
            lib: self.id,
            index: self.techs.len(),
        }
    }

    // A handle is valid only for the plan that issued it: the id keeps an
    // in-range index from another plan out.

    pub(crate) fn valid_item(&self, r: ItemRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.items.len()
    }

    pub(crate) fn valid_recipe(&self, r: RecipeRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.recipes.len()
    }

    pub(crate) fn valid_tech(&self, r: TechRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.techs.len()
    }

    pub(crate) fn valid_bool_setting(&self, r: BoolSettingRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.settings.len()
    }

    pub(crate) fn valid_double_setting(&self, r: DoubleSettingRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.settings.len()
    }

    /// Marks the double settings some recipe reads its crafting time from. The
    /// settings stage needs it too, which is why the binding lives in the plan
    /// rather than in the data pass: the generated setting's minimum depends
    /// on what it backs.
    ///
    /// A handle from another plan is SKIPPED rather than followed, so a bad
    /// reference cannot mark the wrong setting here; `plan_data` is what
    /// refuses it by name.
    pub(crate) fn craft_time_bound_settings(&self) -> Vec<bool> {
        let mut bound = alloc::vec![false; self.settings.len()];
        for r in &self.recipes {
            if self.valid_double_setting(r.spec.craft_time_from) {
                bound[r.spec.craft_time_from.index - 1] = true;
            }
        }
        bound
    }

    /// The setting's bounds as EMITTED: a craft-time-bound double with no
    /// minimum of its own gets the floor-safe one. The auto-minimum is a real
    /// bound and is validated exactly like a declared one.
    pub(crate) fn effective_numeric_spec(&self, i: usize, bound: &[bool]) -> NumericSpec {
        let mut spec = self.settings[i].spec;
        if bound[i] && spec.min.is_none() {
            spec.min = Some(crate::value::CRAFT_TIME_AUTO_MINIMUM);
        }
        spec
    }
}
