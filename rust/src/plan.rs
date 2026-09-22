use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::data::CustomCostFn;
use crate::ingredient_list::{Language, LANGUAGE};
use crate::value::Value;

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
    /// The ingredient language, installed by the two text-setting
    /// constructors and `None` in a plan that declares no text setting. See
    /// [`Language`]: a plan that never offered the player a list to type must
    /// not ship the code that would have read one.
    pub(crate) language: Option<&'static Language>,
    /// The custom-cost resolver, installed by `packs_decl` and by nothing
    /// else. A [`CustomCost`] names a [`PacksSettingRef`], so a plan that
    /// never declared a packs setting can never reach one.
    pub(crate) custom_cost: Option<CustomCostFn>,
    /// The order prefix in force, set by [`Lib::order_after`] and copied into
    /// every setting declared after it. Empty in a plan that never calls it,
    /// which is what keeps such a plan emitting the bare two letters.
    pub(crate) order_prefix: String,
    /// Whether [`Lib::order_after`] was ever given an empty order. It is
    /// recorded rather than refused on the spot because a declaration method
    /// returns a handle and has nowhere to put a refusal: the settings
    /// validator raises it, before it looks at any setting, so a plan that
    /// declares nothing after the call is refused too.
    pub(crate) empty_order_after: bool,
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
    /// A declared INGREDIENT-LIST text setting: the one a recipe reads what it
    /// is made of from. Its own type rather than a `DropdownSettingRef`,
    /// because the two are read differently and a handle that fits both
    /// sockets is a mistake the compiler could have caught.
    IngredientsSettingRef
);
handle!(
    /// A declared SCIENCE-PACK text setting. See [`IngredientsSettingRef`]:
    /// distinct for the same reason, and a pack list is not an ingredient list
    /// (tools only, and `none` is refused).
    PacksSettingRef
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
    /// A string setting holding an INGREDIENT LIST the player writes. It is a
    /// `string-setting` like a dropdown and nothing like one to read: no
    /// allowed values, the reserved word `default` as its default, and the
    /// language in docs/ingredient-list.md as its grammar.
    Ingredients,
    /// The same setting over a research unit's SCIENCE PACKS.
    Packs,
}

pub(crate) struct SettingDecl {
    pub(crate) kind: SettingKind,
    /// What the consumer declared. It is emitted with the mod prefix in front
    /// of it, unless `legacy` is set, in which case it IS the emitted name.
    pub(crate) name: String,
    /// Marks a setting whose name predates this library. See the Legacy
    /// constructors: the name crosses verbatim and the order string is the
    /// consumer's rather than one derived from declaration order.
    pub(crate) legacy: bool,
    pub(crate) order: String,
    /// The order prefix that was in force when this setting was declared. A
    /// GENERATED setting's emitted order is this followed by the two letters
    /// its declaration index gives it, and EVERY DECLARATION CAPTURES IT,
    /// legacy or not: `emitted_order` answers with a legacy setting's own
    /// declared order before it reads this field at all, so a legacy
    /// declaration carries a value nothing reads, which is cheaper than a
    /// branch in every constructor to keep it empty. See
    /// [`Lib::order_after`].
    pub(crate) order_prefix: String,
    pub(crate) def_bool: bool,
    pub(crate) def_num: f64,
    /// The int setting's default as it was DECLARED. `def_num` has already
    /// been through f64 by the time validation runs, so the one number that
    /// could have rounded on the way in is no longer there to check.
    pub(crate) def_int: i64,
    pub(crate) def_str: String,
    /// A text setting's DECLARED list, which the word `default` stands for.
    /// It is never the setting's `default_value`: that is the word itself, so
    /// a player who never opened the settings screen keeps meaning "the mod's
    /// list" when the mod changes it. The list is written into the setting's
    /// description instead, rendered by the language.
    pub(crate) def_ings: Vec<Ingredient>,
    /// The same for a packs setting.
    pub(crate) def_packs: Vec<Pack>,
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

    /// The sort key inside the subgroup. Empty omits the field and lets the
    /// engine order by name.
    pub order: String,

    /// The entity this item builds, by name. It is PRESENCE PROBED like every
    /// other name this library emits: an item naming an entity the game does
    /// not have aborts the load with the engine's own assignID error rather
    /// than anything this library could soften, so a missing one is refused at
    /// plan time with the name in the message.
    ///
    /// The entity is somebody's: your own mod's hand-rolled one, or another
    /// mod's. This library does not emit entities, so there is nothing here to
    /// prefix and nothing to derive.
    pub place_result: String,

    /// Raw prototype fields, passed through VERBATIM after the ones this
    /// library emits, in declaration order. See [`RecipeSpec::extra`].
    pub extra: Vec<(String, Value)>,
}

pub(crate) struct ItemDecl {
    pub(crate) name: String,
    pub(crate) legacy: bool,
    pub(crate) spec: ItemSpec,
}

impl ItemDecl {
    pub(crate) fn emitted_name(&self, prefix: &str) -> String {
        proto_name(self.legacy, prefix, &self.name)
    }
}

/// The one place a prototype name is decided. A legacy name is whatever the
/// mod ships; everything else derives from the packaged mod.
pub(crate) fn proto_name(legacy: bool, prefix: &str, name: &str) -> String {
    if legacy {
        return String::from(name);
    }
    alloc::format!("{}{}", prefix, name)
}

/// WHAT AN INGREDIENT IS MADE OF, and how much of it, in one value.
///
/// THE KIND AND THE AMOUNT ARE THE SAME FIELD, deliberately. The engine takes
/// a whole count for an item and any positive double for a fluid (measured on
/// 2.0.77: `amount = 1.5` on an item LOADS and dumps as 1.5, which is a
/// runtime meaning no player asked for, while a fluid `amount = 0.5` is
/// ordinary and `amount = 0` refuses with "amount must be larger than 0"). A
/// kind flag beside two numbers has a state where the flag says item and the
/// double says 0.5, and nothing in the type stops it; this way that state is
/// unrepresentable and every reader branches once.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Amount {
    /// An item count. The engine holds it in a u16, so 0 and 65536 are both
    /// refusals; see [`MAX_ITEM_AMOUNT`](crate::value::MAX_ITEM_AMOUNT).
    Item(i64),
    /// A fluid amount, which may be fractional and has no ceiling but the
    /// double's own.
    Fluid(f64),
}

impl Amount {
    /// Which of `data.raw`'s two ingredient families this amount belongs to.
    pub(crate) fn is_fluid(&self) -> bool {
        matches!(self, Amount::Fluid(_))
    }

    /// The number the prototype field carries. Factorio has one number type
    /// and it is a double, so an item count rides here too.
    pub(crate) fn value(&self) -> f64 {
        match self {
            Amount::Item(n) => *n as f64,
            Amount::Fluid(v) => *v,
        }
    }
}

/// One line of a recipe. It is built by [`Ingredient::of`],
/// [`Ingredient::named`] or [`Ingredient::fluid`] and cannot be built from a
/// bare string any other way: an ingredient the game does not have is a hard
/// load failure naming the consumer's mod, so a name reaches a prototype only
/// after a presence probe.
#[derive(Clone)]
pub struct Ingredient {
    pub(crate) item: ItemRef,
    pub(crate) amount: Amount,
    pub(crate) candidates: Vec<String>,
}

impl Ingredient {
    /// Names an item this plan declares. It always resolves: the prototype is
    /// emitted by the same plan.
    pub fn of(it: ItemRef, amount: i64) -> Ingredient {
        Ingredient {
            item: it,
            amount: Amount::Item(amount),
            candidates: Vec::new(),
        }
    }

    /// The presence ladder: the candidates are tried in order and the first
    /// one the game actually has is used. If none is present the ingredient
    /// is DROPPED with a log line, never guessed at, because a wrong guess is
    /// somebody else's overhaul pack failing to load.
    pub fn named(amount: i64, first: &str, fallbacks: &[&str]) -> Ingredient {
        Ingredient {
            item: ItemRef::default(),
            amount: Amount::Item(amount),
            candidates: candidate_list(first, fallbacks),
        }
    }

    /// A FLUID, with the same ladder as [`Ingredient::named`] and one probe
    /// of its own: presence is asked of `data.raw.fluid` rather than of the
    /// item family, because the two are separate namespaces and a fluid found
    /// among the items would be a name the recipe cannot use.
    ///
    /// The amount is a double because the engine's is. A recipe that takes a
    /// fluid may not sit in the `crafting` category: the engine refuses that
    /// combination by name (measured: "Recipe is in 'crafting' category but
    /// has a non-item ingredient 'water' (fluid).") and so does this plan,
    /// before anything is emitted.
    pub fn fluid(amount: f64, first: &str, fallbacks: &[&str]) -> Ingredient {
        Ingredient {
            item: ItemRef::default(),
            amount: Amount::Fluid(amount),
            candidates: candidate_list(first, fallbacks),
        }
    }

    /// Whether this line names a fluid, which decides both the presence probe
    /// and the `type` field of the emitted ingredient.
    pub(crate) fn is_fluid(&self) -> bool {
        self.amount.is_fluid()
    }
}

/// The ladder, in declaration order: the author's first choice, then the
/// fallbacks as written.
fn candidate_list(first: &str, fallbacks: &[&str]) -> Vec<String> {
    let mut candidates = Vec::with_capacity(1 + fallbacks.len());
    candidates.push(String::from(first));
    for f in fallbacks {
        candidates.push(String::from(*f));
    }
    candidates
}

/// One dropdown value and the ingredients it selects.
#[derive(Clone, Default)]
pub struct IngredientChoice {
    pub value: String,
    pub ingredients: Vec<Ingredient>,
}

/// Binds a recipe's ingredients to a dropdown setting: the player picks a
/// value and the matching plan is what the recipe is made of.
///
/// Every plan is resolved by the ordinary ladder rules, so a plan may name
/// things another mod provides. A chosen plan that resolves to nothing falls
/// back to the DEFAULT option's plan, with a line saying so, rather than
/// emitting a recipe made of nothing.
///
/// THE DROPDOWN'S OPTION LIST IS EXACTLY WHAT THE AUTHOR DECLARED, and this
/// library adds no value of its own to it. A recipe that also names
/// [`RecipeSpec::ingredients_from`] hands the list to the player whenever that
/// text does not say `default`, and the dropdown decides whenever it does.
/// That is what makes adopting the customizer on a dropdown a mod already
/// ships an identity: no stored choice changes meaning, and a release that
/// drops the text setting again loses nothing, because the engine RESETS a
/// stored value a dropdown no longer offers and keeps a setting no release
/// declares (both measured on 2.0.77).
#[derive(Clone, Default)]
pub struct IngredientChoices {
    pub setting: DropdownSettingRef,
    pub choices: Vec<IngredientChoice>,

    /// Says this declaration is the one whose presets the dropdown shows,
    /// where several declarations name one dropdown. See
    /// [`CostChoices::describes`] for the whole rule; it is one rule and one
    /// field written twice, because the two bindings are two types.
    pub describes: bool,
}

/// A research cost the PLAYER writes: the science packs as an ingredient list
/// in a text setting, the count and the seconds as numeric settings of their
/// own.
///
/// EVERY NON-DEFAULT FIELD IS LIVE, ONE FIELD AT A TIME. The cost is the
/// player's whenever any of the three is not at its declared default; the ones
/// left at their default come from the technology's [`TechSpec::cost_by`] tier
/// where there is one, and from the settings' own declared defaults where
/// there is not. Nothing here is ever "edited but ignored".
///
/// WHAT THE TWO NUMERIC SETTINGS MUST DECLARE DEPENDS ON WHETHER THERE IS A
/// TIER, and this library refuses a pair that does not. Beside a `cost_by`
/// dropdown each of them declares a default of 0, a minimum of 0 and a
/// maximum: 0 is the word `default` of a number, and it means the dropdown
/// decides. With no dropdown there is nothing to defer to, so each declares a
/// minimum of at least 1 and a maximum. The engine takes neither a unit count
/// of 0 nor a research time of 0 (measured: "ResearchIngredient's amount must
/// not be 0" is the pack's, and a unit with `time = 0` refuses with "time must
/// be positive."), and it RESETS a stored value outside a setting's own bounds
/// to that setting's default rather than clamping it (measured), so those
/// bounds are what make every value this library can read back a legal one.
///
/// THE SECONDS ARE AN INT SETTING and not a double, because the field carries
/// a research time in whole seconds and a slider the player drags is easier to
/// land on a whole number than on a fraction.
#[derive(Clone, Default)]
pub struct CustomCost {
    pub packs: PacksSettingRef,
    pub count: IntSettingRef,
    pub seconds: IntSettingRef,
}

/// What a text setting is bound to: how many declarations read it, and the
/// category of the recipe when one does.
#[derive(Default)]
pub(crate) struct TextBinding {
    pub(crate) count: usize,
    pub(crate) category: String,
}

/// One dropdown value and the technologies whose cost it selects, in ladder
/// order.
#[derive(Clone, Default)]
pub struct CostChoice {
    pub value: String,
    pub sources: Vec<String>,

    /// What the composed line says this option costs, in the author's own
    /// words. Empty means absent, and the library then names the ladder's
    /// FIRST source through that technology's own locale key.
    ///
    /// WHY IT EXISTS. The settings stage sees `mods` and never `data.raw`, so
    /// the composed tail names the first rung, which is the truth about the
    /// declaration and can be false about the game: a tier whose first rung is
    /// an expansion's technology tells a base-only player their research is
    /// priced like something that does not exist in their game. A plan already
    /// branches on a mod-set bit for its declared lists, so the author can
    /// write the true sentence per mod set, and this is where it goes.
    ///
    /// IT IS LITERAL TEXT AND NOT A LOCALE KEY, on [`ItemSpec::description`]'s
    /// rule: this library cannot wrap a key it did not compose, and a bare key
    /// in a setting's composition costs the row its whole tooltip (measured on
    /// 2.0.77). Where a choice carries one, the `technology-name` key is not
    /// composed for it, so `check_locale_advisories` has nothing to say about
    /// that choice; the override is also the one way an author can take that
    /// advisory away.
    ///
    /// [`IngredientChoice`] GETS NO TWIN, deliberately. An ingredient preset's
    /// second line is the list in internal names, and it is the one line in the
    /// tooltip a player is invited to paste; an author's prose there would be a
    /// line that cannot be pasted, standing where the pasteable one was.
    pub display: String,
}

/// Binds a technology's research cost to a dropdown setting.
///
/// THE PREREQUISITE MOVES WITH THE UNIT. The source whose cost is copied also
/// becomes the technology's sole prerequisite, so price and tree position come
/// from one named point. That is the rule this surface exists to make easy,
/// and it is why `cost_by` does not combine with `after`, `before` or
/// `after_tech`.
///
/// `fallback` is what applies when no source in the chosen ladder carries a
/// unit this library can copy. It is a hand-rolled cost, validated exactly
/// like one, and a technology that falls back has no prerequisite at all.
///
/// THE OPTION LIST IS EXACTLY WHAT THE AUTHOR DECLARED, as
/// [`IngredientChoices`]' is. A technology that also names
/// [`TechSpec::cost_from`] lets the player overwrite the tier's three numbers
/// one field at a time, and the tier supplies whatever is left at its default.
#[derive(Clone, Default)]
pub struct CostChoices {
    pub setting: DropdownSettingRef,
    pub choices: Vec<CostChoice>,
    pub fallback: UnitSpec,

    /// Says this declaration is the one whose presets the dropdown shows.
    ///
    /// A DROPDOWN SHOWS ONE DECLARATION'S PRESETS, because a setting carries
    /// one `localised_description`. Several declarations may name one
    /// dropdown: two recipes with `ingredients_by`, or a recipe and a
    /// technology. Where none of them is marked the rule is positional,
    /// recipes then technologies in declaration order with the last writer
    /// winning, which is what a plan that never sets this field keeps. Marking
    /// one says so instead, and the settings composition, the ladder
    /// predicate, the locale obligation and the game-key advisory all read the
    /// same answer.
    ///
    /// TWO MARKED DECLARATIONS OVER ONE DROPDOWN ARE REFUSED, and so is a
    /// marked `CostBy` with no `CostFrom` beside it, which composes nothing at
    /// all and would hand the dropdown an empty description while another
    /// declaration could have described it. Marking the only declaration that
    /// names a dropdown is accepted and changes nothing: a plan that grows a
    /// second declaration later still says which one describes.
    pub describes: bool,
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

    /// `ingredients` is one fixed list and combines with neither of the other
    /// two. `ingredients_by` lets a dropdown setting choose between several;
    /// `ingredients_from` hands the whole list to the player as text; and the
    /// two of them TOGETHER is the ordinary customizable recipe.
    pub ingredients_by: Option<IngredientChoices>,
    pub ingredients: Vec<Ingredient>,
    /// The text setting this recipe is made of. See
    /// [`Lib::ingredients_setting`] and docs/ingredient-list.md.
    ///
    /// THE TEXT IS THE SWITCH. Whenever it does not say `default` the list the
    /// player wrote is what the recipe is made of, and the dropdown beside it,
    /// if there is one, is set aside with a line saying so. Whenever it does
    /// say `default` the dropdown decides, or the setting's own declared list
    /// applies where there is no dropdown. A text the language refuses behaves
    /// exactly as `default` does, with one ERROR line naming the setting.
    pub ingredients_from: Option<IngredientsSettingRef>,
    /// Zero means 1.
    pub result_count: i64,
    pub category: String,
    pub display_name: String,
    pub description: String,

    /// An EXISTING item this recipe produces, for a recipe whose result this
    /// plan does not declare. Give a default `ItemRef` and this name; the item
    /// is presence probed at emit and refused if absent, and `name` is then
    /// required, because there is no declared item to inherit it from.
    pub result_named: String,

    /// The sort key inside the recipe group. Empty omits the field.
    pub order: String,

    /// Raw prototype fields, passed through VERBATIM after the ones this
    /// library emits, in declaration order.
    ///
    /// THE VALUES ARE YOURS AND ARE NOT TOUCHED. Nothing inside an extra value
    /// is prefixed, and no name inside one is presence probed: this library
    /// cannot know which strings in an arbitrary field are prototype names, so
    /// guessing would be worse than the passthrough. If a field holds a name
    /// the game may not have, you own that check.
    ///
    /// A key this library emits itself is REFUSED rather than merged or
    /// overridden, because two writers of one field is a silent last-writer
    /// and the loser would be whichever order this library happens to use.
    ///
    /// `enabled` IS THE ONE EXCEPTION, for a mod migrating a recipe before its
    /// technology: it is accepted while no technology in this plan names the
    /// recipe in `unlocks`, and the value is emitted in the slot the library's
    /// own `enabled` would have taken, so the prototype's field order is the
    /// same either way. The moment a plan technology unlocks the recipe the
    /// key is refused again, because the research is then what turns the
    /// recipe on.
    pub extra: Vec<(String, Value)>,
}

pub(crate) struct RecipeDecl {
    pub(crate) name: String,
    pub(crate) legacy: bool,
    pub(crate) result: ItemRef,
    pub(crate) spec: RecipeSpec,
}

impl RecipeDecl {
    pub(crate) fn emitted_name(&self, prefix: &str) -> String {
        proto_name(self.legacy, prefix, &self.name)
    }
}

/// One science pack of a hand-rolled research cost, with the same presence
/// ladder an ingredient carries.
///
/// A PACK IS DROPPED, NOT REFUSED, when the game has none of its rungs: an
/// untouched pack list in a modpack that renamed the packs used to be a hard
/// load failure while an untouched ingredient list degraded quietly, and the
/// two are the same promise to the same author. A unit whose packs ALL drop is
/// EMITTED WITH AN EMPTY INGREDIENT LIST, with one ERROR line naming every rung
/// and one line in the technology's own tooltip: measured in play on 2.0.77,
/// such a research COMPLETES for free, which is a balance change the player did
/// not choose and so is disclosed where they look. It used to refuse, and that
/// was a lock-out: the engine's error dialog cannot reach the Mod Settings
/// screen.
#[derive(Clone, Default)]
pub struct Pack {
    pub name: String,
    pub amount: i64,
    /// The rungs after `name`, tried in the order given.
    pub fallbacks: Vec<String>,
}

impl Pack {
    /// A one-rung ladder: the pack the author means, and no substitute.
    pub fn new(name: &str, amount: i64) -> Pack {
        Pack {
            name: String::from(name),
            amount,
            fallbacks: Vec::new(),
        }
    }

    /// The ladder, in the shape [`Ingredient::named`] uses: the amount first,
    /// then the first choice and the fallbacks behind it. The first rung the
    /// game actually has is the one priced.
    pub fn named(amount: i64, first: &str, fallbacks: &[&str]) -> Pack {
        let mut rungs = Vec::with_capacity(fallbacks.len());
        for f in fallbacks {
            rungs.push(String::from(*f));
        }
        Pack {
            name: String::from(first),
            amount,
            fallbacks: rungs,
        }
    }
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
    /// The sort key in the technology screen. Empty omits the field.
    pub order: String,

    /// Raw prototype fields, passed through VERBATIM after the ones this
    /// library emits, in declaration order. See [`RecipeSpec::extra`].
    pub extra: Vec<(String, Value)>,

    pub icon: String,
    pub icon_size: i64,

    /// `cost_of` copies a named technology's whole unit verbatim,
    /// count_formula and all.
    ///
    /// `cost_of`, `unit` and the `cost_by`/`cost_from` pair are EXCLUSIVE, and
    /// `cost_by` with `cost_from` is the one combination: the dropdown is the
    /// tier and the three settings overwrite it field by field.
    pub cost_of: String,
    pub unit: Option<UnitSpec>,
    /// Lets a dropdown setting choose between several sources, and places the
    /// technology as well: see [`CostChoices`].
    pub cost_by: Option<CostChoices>,
    /// `unit` with its three numbers in the player's hands. On its own it does
    /// NOT place the technology and the ordinary placement fields apply;
    /// beside a `cost_by` the tier's own source technology is the
    /// prerequisite, as it is for every other `cost_by`.
    pub cost_from: Option<CustomCost>,

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
    pub(crate) legacy: bool,
    pub(crate) spec: TechSpec,
}

impl TechDecl {
    pub(crate) fn emitted_name(&self, prefix: &str) -> String {
        proto_name(self.legacy, prefix, &self.name)
    }
}

impl SettingDecl {
    /// The name a setting prototype actually carries: prefixed for a generated
    /// setting, verbatim for a legacy one.
    pub(crate) fn emitted_name(&self, prefix: &str) -> String {
        if self.legacy {
            return self.name.clone();
        }
        alloc::format!("{}{}", prefix, self.name)
    }
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
            language: None,
            custom_cost: None,
            order_prefix: String::new(),
            empty_order_after: false,
        }
    }

    /// Places every generated setting declared after this call behind the
    /// setting whose order string is given: from here on a generated
    /// setting's order is that string followed by the two letters it already
    /// gets from its declaration index, so it sorts AFTER the legacy setting
    /// carrying that order and before every legacy order that sorts after
    /// that one. It is not placed before every order that fails to extend the
    /// named one, which is the wider claim and a false one: beside a legacy
    /// "a", `order_after("b")` places a setting at "bab", which is past "a"
    /// and meant to be. Legacy settings keep the orders they were declared
    /// with. Call it again to move on; a plan that never calls it keeps the
    /// bare two letters.
    ///
    /// THIS IS FOR A MOD THAT ALREADY SHIPPED ORDERS. The two letters count
    /// DECLARATION SLOTS, the legacy declarations among them, and run "aa"
    /// to "az" and then "ba": beside legacy orders "a" and "b", a generated
    /// setting in any of the first twenty-six slots lands BETWEEN the two,
    /// and the twenty-seventh declaration carries "ba" and lands past the
    /// second, by arithmetic rather than by choice. A consumer who wants a
    /// generated setting under a particular legacy one names that one's
    /// order here and keeps the generated name.
    ///
    /// A LEGACY ORDER THAT EXTENDS THE NAMED ONE IS THE ONE THING THIS
    /// CANNOT PLACE AROUND, and the settings stage refuses the plan rather
    /// than sorting a setting past it: beside legacy orders "a" and "ab",
    /// `order_after("a")` reaches "aba" as soon as the two letters roll over
    /// from "az" to "ba", and "aba" sorts after "ab" instead of under "a"
    /// with the settings declared before it.
    ///
    /// AN EMPTY ORDER IS REFUSED, at the settings stage rather than here: a
    /// declaration method returns a handle and has nowhere to put a refusal.
    /// It is not the way back to the bare two letters either, because a plan
    /// that wants those never calls this at all.
    pub fn order_after(&mut self, order: &str) {
        self.order_prefix = String::from(order);
        if order.is_empty() {
            self.empty_order_after = true;
        }
    }

    /// Declares a startup bool setting. The name is prefixed on the way out;
    /// what is passed here is the bare name.
    pub fn bool_setting(&mut self, name: &str, def: bool) -> BoolSettingRef {
        self.settings.push(SettingDecl {
            kind: SettingKind::Bool,
            name: String::from(name),
            legacy: false,
            order: String::new(),
            order_prefix: self.order_prefix.clone(),
            def_bool: def,
            def_num: 0.0,
            def_int: 0,
            def_str: String::new(),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
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
            legacy: false,
            order: String::new(),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: def as f64,
            def_int: def,
            def_str: String::new(),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
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
            legacy: false,
            order: String::new(),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: def,
            def_int: 0,
            def_str: String::new(),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
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
            legacy: false,
            order: String::new(),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: 0.0,
            def_int: 0,
            def_str: String::from(def),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
            spec: NumericSpec::default(),
            values: allowed,
        });
        DropdownSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares a startup text setting holding an INGREDIENT LIST the player
    /// writes, in the language documented in docs/ingredient-list.md.
    ///
    /// `def` is the list the reserved word `default` stands for, ladders and
    /// all. It is NOT the setting's default value: that is the word itself,
    /// which is what keeps a player who never opened the settings screen on
    /// the mod's list when the mod changes it (the engine writes every
    /// setting's current value into mod-settings.dat, untouched defaults
    /// included, so "untouched means equal to the rendered list" would have
    /// frozen the old list into every such player's game). The list is
    /// rendered into the setting's DESCRIPTION instead, so the player can see
    /// it and copy it.
    ///
    /// A text setting is bound to exactly one recipe, through
    /// [`RecipeSpec::ingredients_from`]; one that is declared and read by
    /// nothing is refused.
    pub fn ingredients_setting(
        &mut self,
        name: &str,
        def: Vec<Ingredient>,
    ) -> IngredientsSettingRef {
        self.ingredients_decl(name, false, "", def)
    }

    /// Declares an ingredient-list setting under a name this mod ALREADY
    /// SHIPS. See [`Lib::legacy_bool_setting`] for why a migrating mod cannot
    /// rename its settings.
    pub fn legacy_ingredients_setting(
        &mut self,
        full_name: &str,
        def: Vec<Ingredient>,
        order: &str,
    ) -> IngredientsSettingRef {
        self.ingredients_decl(full_name, true, order, def)
    }

    /// Declares a startup text setting holding a research unit's SCIENCE
    /// PACKS. The language is the same one, with tool-type items only and no
    /// empty list. See [`Lib::ingredients_setting`] for what `def` means.
    pub fn packs_setting(&mut self, name: &str, def: Vec<Pack>) -> PacksSettingRef {
        self.packs_decl(name, false, "", def)
    }

    /// Declares a pack-list setting under a name this mod ALREADY SHIPS.
    pub fn legacy_packs_setting(
        &mut self,
        full_name: &str,
        def: Vec<Pack>,
        order: &str,
    ) -> PacksSettingRef {
        self.packs_decl(full_name, true, order, def)
    }

    /// ONE OF THE TWO PLACES THE LANGUAGE IS NAMED, and the whole reason the
    /// installation sits in a constructor rather than in a planner: a plan
    /// reaches the parser and the renderer only through the table this line
    /// puts in it, so a consumer who never declares a text setting links none
    /// of the language at all. See [`Language`].
    fn ingredients_decl(
        &mut self,
        name: &str,
        legacy: bool,
        order: &str,
        def: Vec<Ingredient>,
    ) -> IngredientsSettingRef {
        self.language = Some(&LANGUAGE);
        self.settings.push(SettingDecl {
            kind: SettingKind::Ingredients,
            name: String::from(name),
            legacy,
            order: String::from(order),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: 0.0,
            def_int: 0,
            def_str: String::new(),
            def_ings: def,
            def_packs: Vec::new(),
            spec: NumericSpec::default(),
            values: Vec::new(),
        });
        IngredientsSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// THE OTHER PLACE THE LANGUAGE IS NAMED, and the only place the
    /// custom-cost resolver is: a research cost the player writes needs the
    /// pack text this constructor declares, so a plan without one can never
    /// reach it. See [`Lib::ingredients_decl`] and [`Language`].
    fn packs_decl(
        &mut self,
        name: &str,
        legacy: bool,
        order: &str,
        def: Vec<Pack>,
    ) -> PacksSettingRef {
        self.language = Some(&LANGUAGE);
        self.custom_cost = Some(crate::data::CUSTOM_COST);
        self.settings.push(SettingDecl {
            kind: SettingKind::Packs,
            name: String::from(name),
            legacy,
            order: String::from(order),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: 0.0,
            def_int: 0,
            def_str: String::new(),
            def_ings: Vec::new(),
            def_packs: def,
            spec: NumericSpec::default(),
            values: Vec::new(),
        });
        PacksSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares an item this mod introduces.
    pub fn item(&mut self, name: &str, spec: ItemSpec) -> ItemRef {
        self.item_decl(name, false, spec)
    }

    /// Declares an item under a name this mod ALREADY SHIPS, emitted verbatim
    /// with no prefix.
    ///
    /// THE SAME ARGUMENT AS THE LEGACY SETTINGS, AND STRONGER. A save
    /// references prototype names directly: an item sits in inventories and on
    /// belts by name, a technology is recorded as researched by name, a recipe
    /// is remembered by name in every assembler. A mod's own hand-rolled
    /// neighbours name them too, and the engine's failure for a dangling
    /// reference is not a warning but an abort:
    ///
    /// ```text
    /// Error in assignID: item with name 'bbb-balancer-part' does not exist.
    /// ```
    ///
    /// So a migrating mod cannot rename its prototypes any more than its
    /// settings, and this is the escape hatch. The legacy mark in the name is
    /// the whole documentation of the deviation: a reader sees at the call
    /// site that this name is not derived, and nothing else in the library can
    /// produce one.
    ///
    /// The handle is an ORDINARY handle. `Ingredient::of`, `unlocks`,
    /// `after_tech` and the splices take it exactly as they take a generated
    /// one, so a plan may be part legacy and part generated without either
    /// half knowing.
    pub fn legacy_item(&mut self, full_name: &str, spec: ItemSpec) -> ItemRef {
        self.item_decl(full_name, true, spec)
    }

    fn item_decl(&mut self, name: &str, legacy: bool, spec: ItemSpec) -> ItemRef {
        self.items.push(ItemDecl {
            name: String::from(name),
            legacy,
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
            // The RESULT's declared name, not its emitted one: this is the
            // recipe's own unprefixed name, and the prefix is applied to it at
            // emit like any other.
            name = self.items[result.index - 1].name.clone();
        }
        self.recipe_decl(name, false, result, spec)
    }

    /// Declares a recipe under a name this mod ALREADY SHIPS, emitted verbatim
    /// with no prefix. See [`Lib::legacy_item`] for why prototype names cannot
    /// be regenerated for a mod that has players.
    ///
    /// The name is required rather than inherited from the result: a legacy
    /// recipe and its result item are two independent names the mod already
    /// chose, and deriving one from the other would be a guess.
    pub fn legacy_recipe(
        &mut self,
        result: ItemRef,
        full_name: &str,
        spec: RecipeSpec,
    ) -> RecipeRef {
        self.recipe_decl(String::from(full_name), true, result, spec)
    }

    fn recipe_decl(
        &mut self,
        name: String,
        legacy: bool,
        result: ItemRef,
        spec: RecipeSpec,
    ) -> RecipeRef {
        self.recipes.push(RecipeDecl {
            name,
            legacy,
            result,
            spec,
        });
        RecipeRef {
            lib: self.id,
            index: self.recipes.len(),
        }
    }

    /// Declares a technology this mod introduces.
    pub fn technology(&mut self, name: &str, spec: TechSpec) -> TechRef {
        self.tech_decl(name, false, spec)
    }

    /// Declares a technology under a name this mod ALREADY SHIPS, emitted
    /// verbatim with no prefix. See [`Lib::legacy_item`] for why prototype
    /// names cannot be regenerated for a mod that has players.
    pub fn legacy_technology(&mut self, full_name: &str, spec: TechSpec) -> TechRef {
        self.tech_decl(full_name, true, spec)
    }

    fn tech_decl(&mut self, name: &str, legacy: bool, spec: TechSpec) -> TechRef {
        self.techs.push(TechDecl {
            name: String::from(name),
            legacy,
            spec,
        });
        TechRef {
            lib: self.id,
            index: self.techs.len(),
        }
    }

    /// The installed language, for a caller the validators have already let
    /// through.
    ///
    /// EVERY CALLER SITS BEHIND `validate_text_settings`, which both planners
    /// run before they touch a text setting and which refuses a plan whose
    /// text setting arrived without the table. A `None` here is that guard
    /// having been removed, not a plan a consumer can build, and it says so
    /// rather than rendering something nobody declared.
    pub(crate) fn installed_language(&self) -> &'static Language {
        self.language
            .expect("a text setting reaches the language only through validate_text_settings")
    }

    /// The installed custom-cost resolver. See [`Lib::installed_language`]:
    /// the same guard covers it, and a `CustomCost` reaches this only behind a
    /// packs setting.
    pub(crate) fn installed_custom_cost(&self) -> CustomCostFn {
        self.custom_cost
            .expect("a custom cost reaches its resolver only through validate_text_settings")
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

    // -----------------------------------------------------------------------
    // The Legacy constructors.
    //
    // THESE ARE FOR MIGRATING A MOD THAT ALREADY SHIPPED. Factorio persists a
    // player's startup choices in mod-settings.dat keyed by the setting's
    // NAME, and it has no rename mechanism: a setting that comes back under a
    // different name is a new setting, and every player who had chosen a
    // value gets the default instead. A mod whose settings predate this
    // library therefore cannot adopt the generated names without discarding
    // what its players chose.
    //
    // So the name crosses VERBATIM, with no prefix, and the order string is
    // the consumer's own because a historic mod picked its own (generated
    // settings get two letters from declaration order; a mod that shipped "a"
    // and "b" keeps them, and a settings dump hash pins that).
    //
    // THE INVARIANT THIS BENDS, SAID PLAINLY. Everywhere else in this library
    // an unprefixed name is unrepresentable. Here it is representable through
    // a constructor whose name says Legacy, which is the same signposting
    // `dropdown_setting_needing_locale` uses: the deviation is at the call
    // site, where a reviewer sees it.
    //
    // A NEW SETTING USES THE PREFIXED CONSTRUCTORS. Nothing about these is a
    // shortcut around the prefix; they exist so a migration can preserve
    // values, and a mod with no shipped settings has nothing to preserve.
    // -----------------------------------------------------------------------

    /// Declares a bool setting under a name this mod already ships.
    pub fn legacy_bool_setting(
        &mut self,
        full_name: &str,
        def: bool,
        order: &str,
    ) -> BoolSettingRef {
        self.settings.push(SettingDecl {
            kind: SettingKind::Bool,
            name: String::from(full_name),
            legacy: true,
            order: String::from(order),
            order_prefix: self.order_prefix.clone(),
            def_bool: def,
            def_num: 0.0,
            def_int: 0,
            def_str: String::new(),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
            spec: NumericSpec::default(),
            values: Vec::new(),
        });
        BoolSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares an int setting under a name this mod already ships.
    pub fn legacy_int_setting(
        &mut self,
        full_name: &str,
        def: i64,
        spec: NumericSpec,
        order: &str,
    ) -> IntSettingRef {
        self.settings.push(SettingDecl {
            kind: SettingKind::Int,
            name: String::from(full_name),
            legacy: true,
            order: String::from(order),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: def as f64,
            def_int: def,
            def_str: String::new(),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
            spec,
            values: Vec::new(),
        });
        IntSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares a double setting under a name this mod already ships.
    pub fn legacy_double_setting(
        &mut self,
        full_name: &str,
        def: f64,
        spec: NumericSpec,
        order: &str,
    ) -> DoubleSettingRef {
        self.settings.push(SettingDecl {
            kind: SettingKind::Double,
            name: String::from(full_name),
            legacy: true,
            order: String::from(order),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: def,
            def_int: 0,
            def_str: String::new(),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
            spec,
            values: Vec::new(),
        });
        DoubleSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    /// Declares a string setting under a name this mod already ships. The
    /// values still need locale entries, and a migrated mod already has them
    /// under exactly these keys.
    pub fn legacy_dropdown_setting_needing_locale(
        &mut self,
        full_name: &str,
        def: &str,
        values: &[&str],
        order: &str,
    ) -> DropdownSettingRef {
        let mut allowed = Vec::with_capacity(values.len());
        for v in values {
            allowed.push(String::from(*v));
        }
        self.settings.push(SettingDecl {
            kind: SettingKind::Dropdown,
            name: String::from(full_name),
            legacy: true,
            order: String::from(order),
            order_prefix: self.order_prefix.clone(),
            def_bool: false,
            def_num: 0.0,
            def_int: 0,
            def_str: String::from(def),
            def_ings: Vec::new(),
            def_packs: Vec::new(),
            spec: NumericSpec::default(),
            values: allowed,
        });
        DropdownSettingRef {
            lib: self.id,
            index: self.settings.len(),
        }
    }

    pub(crate) fn valid_dropdown_setting(&self, r: DropdownSettingRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.settings.len()
    }

    pub(crate) fn valid_bool_setting(&self, r: BoolSettingRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.settings.len()
    }

    pub(crate) fn valid_double_setting(&self, r: DoubleSettingRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.settings.len()
    }

    pub(crate) fn valid_int_setting(&self, r: IntSettingRef) -> bool {
        r.lib == self.id && r.index >= 1 && r.index <= self.settings.len()
    }

    // THE TWO TEXT VALIDATORS ASK THE KIND AS WELL, and they are the only ones
    // that do. Following one of these handles is what reaches the ingredient
    // language, and the guard in `validate_text_settings` decides on the
    // setting's KIND; a handle that pointed at a setting of another kind would
    // be followed by a reach the guard never looked at, which is the
    // unvalidated dereference the customizer's own design review already
    // caught once. So the composition and the validator share one condition
    // here too: a followed handle names a text setting, and a text setting has
    // been past the guard.

    pub(crate) fn valid_ingredients_setting(&self, r: IngredientsSettingRef) -> bool {
        r.lib == self.id
            && r.index >= 1
            && r.index <= self.settings.len()
            && self.settings[r.index - 1].kind == SettingKind::Ingredients
    }

    pub(crate) fn valid_packs_setting(&self, r: PacksSettingRef) -> bool {
        r.lib == self.id
            && r.index >= 1
            && r.index <= self.settings.len()
            && self.settings[r.index - 1].kind == SettingKind::Packs
    }

    /// Who reads each text setting, and under which recipe's category.
    ///
    /// THE CATEGORY TRAVELS WITH THE BINDING because the fluid rule is about
    /// the RECIPE and not about the ingredient: the same declared fluid is
    /// legal in a chemistry recipe and a load failure in a crafting one, so a
    /// text setting's declared default can only be checked against the recipe
    /// that reads it.
    ///
    /// A handle this plan never issued is SKIPPED rather than followed,
    /// exactly as `craft_time_bound_settings` skips one: `validate_bindings`
    /// is what refuses it by name, and following it here would mark the wrong
    /// setting.
    pub(crate) fn text_setting_bindings(&self) -> Vec<TextBinding> {
        let mut out: Vec<TextBinding> = Vec::with_capacity(self.settings.len());
        for _ in &self.settings {
            out.push(TextBinding::default());
        }
        let mut mark = |index: usize, category: &str| {
            out[index - 1].count += 1;
            out[index - 1].category = String::from(category);
        };
        for r in &self.recipes {
            if let Some(h) = r.spec.ingredients_from {
                if self.valid_ingredients_setting(h) {
                    mark(h.index, &r.spec.category);
                }
            }
        }
        for t in &self.techs {
            if let Some(cc) = &t.spec.cost_from {
                if self.valid_packs_setting(cc.packs) {
                    mark(cc.packs.index, "");
                }
            }
        }
        out
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
