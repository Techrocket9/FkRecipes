package fkrecipes

import "sync/atomic"

// nextLibID stamps each plan with an identity so a handle carries which plan
// it came from. ATOMIC, not a plain increment: the guest is single-threaded
// but the consumer's own `go test` is not, and two plans built concurrently
// that came out with the same id would resolve each other's handles in
// silence. TinyGo supports sync/atomic, and on single-threaded wasm the
// atomic lowers to the plain load and store it would have been anyway.
// Nothing here reaches an op, so it cannot make a plan non-deterministic.
var nextLibID uint64

// noCopy makes `go vet` refuse a copied plan. A Lib copy would carry the
// original's id, so handles from one would validate against the other, and
// nothing else would go wrong loudly enough to notice. The methods are the
// shape vet's copylocks check looks for and are never called.
type noCopy struct{}

func (*noCopy) Lock()   {}
func (*noCopy) Unlock() {}

// language is the ingredient list reached as VALUES rather than by name, and
// it is a size seam rather than an abstraction: whole-program elimination
// keeps whatever a reachable path NAMES, so a planner that called the parser
// directly made every consumer ship it.
//
// MEASURED ON A FIXTURE, AND NOT ON THE PILOT. go/examples/notext is a plan
// SHAPED like BetterBeltBalancer's before its customizer round (two legacy
// dropdowns driving IngredientsBy and CostBy, no text setting anywhere), and
// it exists so this number can be re-taken. Packaged with fklua it produced a
// 95,227 line, 3,658,810 byte fk_data_module.lua before this seam and a 67,023
// line, 2,716,663 byte one after (measured at c7a806e against an fklua at
// a1fcd04, and a figure for that head alone), because both planners used to
// call the parser, the renderer and the custom-cost resolver by name and TinyGo
// therefore had to keep all three. The pilot's own module is neither figure:
// 39,056 lines and 1,763,788 bytes at its pre-customizer release, 122,031
// lines and 4,904,124 bytes at its round-three head, where it declares text
// settings and links the language on purpose.
//
// THE LINE AND BYTE COUNTS ARE THE ORACLE, and a grep is not. TinyGo inlines a
// single-caller function and its header leaves the module while its code
// stays, so a leak of tens of kilobytes can sit behind a grep for
// parseIngredientList, classifyPiece and resolveCustomCost that answers zero
// on all three. Compare the counts against the CURRENT figures, the
// four-fixture table under "The claim gets its adjective" in Fix round 1b of
// agents/implementation-notes.md, which carries the runnable recipe beside
// them; the pair above is the seam's own before and after and belongs to the
// head it names.
//
// A PLAN THAT DECLARES NO TEXT SETTING CARRIES A NIL HERE, and that state
// cannot be reached through the public surface: the two text-setting
// constructors are the only way to declare one and they install this.
// validateTextSettings is what says so out loud rather than dereferencing
// nothing, because a package-internal caller can append a declaration by hand.
type language struct {
	parse  func(text string, kind listKind, category, setting string, w World) (parsedList, string)
	render func(list ingredientList) string
	amount func(v float64) string
}

// Lib is one mod's plan. Everything the declaration methods record is an
// ordinary value in declaration order; nothing is validated, resolved or
// emitted until PlanSettings or PlanData runs.
type Lib struct {
	noCopy   noCopy
	id       uint64
	settings []settingDecl
	items    []itemDecl
	recipes  []recipeDecl
	techs    []techDecl

	// The ingredient language, installed by the text-setting constructors and
	// nil in a plan that declares no text setting. See language. It is a FIELD
	// AND NOT A PACKAGE-LEVEL VALUE for two reasons: a package-level one
	// initialised at load would name the three functions unconditionally and
	// defeat the whole seam, and a package-level one written by a constructor
	// would be a data race the moment a consumer builds two plans in parallel
	// tests, which `go test -race` is a gate against.
	lang *language
	// customCost is resolveCustomCost held the same way, and packsSetting
	// alone installs it: a CustomCost names a PacksSettingRef, so a plan that
	// never called that constructor can never reach a custom cost.
	customCost func(l *Lib, w, text World, res *resolution, prefix string, t techDecl, c *CustomCost) Value

	// orderPrefix is the order string OrderAfter last named, and it is the
	// one every GENERATED setting declared from here on extends. It is
	// COPIED ONTO EACH DECLARATION as that declaration arrives, never read
	// at plan time: see OrderAfter.
	orderPrefix string
	// orderAfterEmpty records that OrderAfter was called with an empty
	// string, so the settings stage can refuse it. A declaration method has
	// no error to return, and the call is a mistake whether or not a setting
	// follows it.
	orderAfterEmpty bool
}

// New starts an empty plan. It is the ONLY way to get a usable one: a zero
// Lib (var l fkrecipes.Lib) carries id 0, and every such plan would share it,
// which is exactly the cross-plan handle mix-up the id exists to catch. Both
// planning entry points refuse a plan with no id rather than validate handles
// against an identity nothing owns.
func New() *Lib {
	return &Lib{id: atomic.AddUint64(&nextLibID, 1)}
}

// The handles. Each carries the id of the plan that issued it and a 1-BASED
// index into that plan's matching declaration slice, so the Go zero value
// means absent and the Rust mirror gets the same meaning from a derived
// Default. A consumer cannot build one from a number, and a handle borrowed
// from ANOTHER plan is refused rather than silently resolved: two plans both
// have an index 1, and without the id the second plan would emit the first
// plan's item under its own name.
type (
	BoolSettingRef struct {
		_     boolSettingRefTag
		lib   uint64
		index int
	}
	IntSettingRef struct {
		_     intSettingRefTag
		lib   uint64
		index int
	}
	DoubleSettingRef struct {
		_     doubleSettingRefTag
		lib   uint64
		index int
	}
	DropdownSettingRef struct {
		_     dropdownSettingRefTag
		lib   uint64
		index int
	}
	ItemRef struct {
		_     itemRefTag
		lib   uint64
		index int
	}
	RecipeRef struct {
		_     recipeRefTag
		lib   uint64
		index int
	}
	TechRef struct {
		_     techRefTag
		lib   uint64
		index int
	}
	IngredientsSettingRef struct {
		_     ingredientsSettingRefTag
		lib   uint64
		index int
	}
	PacksSettingRef struct {
		_     packsSettingRefTag
		lib   uint64
		index int
	}
)

// One zero-size marker per handle, so no two handles share an underlying
// type. Without them a consumer could write BoolSettingRef(someItemRef) and
// the conversion would compile: the plan would then read a bool out of an
// int setting and hide a technology nobody asked to hide. The markers cost
// nothing at runtime and sit first so they add no padding. Rust needs none of
// this; its handle types are nominal already.
type (
	boolSettingRefTag     struct{}
	intSettingRefTag      struct{}
	doubleSettingRefTag   struct{}
	dropdownSettingRefTag struct{}
	itemRefTag            struct{}
	recipeRefTag          struct{}
	techRefTag            struct{}
	// The two text settings are separate types for the same reason the rest
	// are: an ingredient list and a pack list are read with different rules
	// and bound to different fields, and a conversion between them would be a
	// recipe priced in science packs.
	ingredientsSettingRefTag struct{}
	packsSettingRefTag       struct{}
)

// NumericSpec bounds an int or double setting. Both bounds are optional and
// the has-flag carries that: a zero NumericSpec is an unbounded setting, not
// one pinned to zero.
type NumericSpec struct {
	HasMin bool
	Min    float64
	HasMax bool
	Max    float64
}

// Between is the common case, both bounds given.
func Between(low, high float64) NumericSpec {
	return NumericSpec{HasMin: true, Min: low, HasMax: true, Max: high}
}

type settingKind uint8

const (
	settingBool settingKind = iota
	settingInt
	settingDouble
	settingDropdown
	// The two TEXT settings, which are string-settings the player writes into
	// rather than picks from. Both hold an ingredient list in the language
	// docs/ingredient-list.md describes; they differ in what a name may
	// resolve to and in what they are bound to.
	settingIngredients
	settingPacks
)

// isText reports the two setting kinds whose value is a list the player types.
// Everything that treats them alike (the emitted prototype, the locale
// obligation, the bound-exactly-once rule) asks this rather than naming both.
func (k settingKind) isText() bool {
	return k == settingIngredients || k == settingPacks
}

type settingDecl struct {
	kind settingKind
	// name is what the consumer declared. It is emitted with the mod prefix
	// in front of it, unless legacy is set, in which case it IS the emitted
	// name.
	name string
	// legacy marks a setting whose name predates this library. See the
	// Legacy constructors: the name crosses verbatim and the order string is
	// the consumer's rather than one derived from declaration order.
	legacy bool
	order  string
	// orderPrefix is the order string OrderAfter had in force when this
	// setting was declared, and a GENERATED setting's own two letters extend
	// it. EVERY DECLARATION CAPTURES IT, legacy or not: emittedOrder answers
	// with a legacy setting's own order before it reads this field at all, so
	// a legacy declaration carries a value nothing reads, which is cheaper
	// than a branch in every constructor to keep it empty. See emittedOrder.
	orderPrefix string
	defBool     bool
	defNum      float64
	// The int setting's default as it was DECLARED. defNum has already been
	// through float64 by the time validation runs, so the one number that
	// could have rounded on the way in is no longer there to check.
	defInt int64
	defStr string
	spec   NumericSpec
	values []string

	// The DECLARED default of a text setting, which is what the reserved word
	// default means and what the setting's description writes out. It is kept
	// as declarations rather than as text because it carries presence ladders
	// and handles into this plan's own items: the rendering needs the mod
	// prefix, which only a planner has.
	defIngredients []Ingredient
	defPacks       []Pack
}

// emittedName is the name a setting prototype actually carries: prefixed for a
// generated setting, verbatim for a legacy one.
func (s settingDecl) emittedName(prefix string) string {
	if s.legacy {
		return s.name
	}
	return prefix + s.name
}

// emittedOrder is the order string a setting prototype actually carries: the
// consumer's own for a legacy setting, and for a generated one the prefix in
// force at its declaration followed by the two letters its declaration index
// gives it. The planner and the validator both ask this rather than deciding
// it twice and disagreeing about what a refusal is talking about.
func (s settingDecl) emittedOrder(i int) string {
	if s.legacy {
		return s.order
	}
	return s.orderPrefix + orderString(i)
}

// OrderAfter places every generated setting declared after this call behind
// the setting whose order string is given: from here on a generated setting's
// order is that string followed by the two letters it already gets from its
// declaration index.
//
// WHAT THAT PLACES, EXACTLY. The setting sorts after the legacy setting
// carrying the named order, and before every legacy order that sorts after
// that one. It says nothing about the orders that sort BEFORE it: with a
// legacy "a" and a legacy "b", OrderAfter("b") puts what follows at "b"
// followed by its own two letters, which is after "a" as well as after "b",
// because that is where "b" itself already was. The one placement the two
// letters cannot honour is a legacy order that EXTENDS the named one, such
// as "ab" under OrderAfter("a"): a generated setting far enough along the
// alphabet sorts past it, and the settings stage refuses that plan by name
// rather than shipping the silent misplacement.
//
// Legacy settings keep the orders they were declared with. Call it again to
// move on; a plan that never calls it keeps the bare two letters.
//
// WHAT IT IS FOR. A migrated mod keeps the order strings it already shipped,
// and a generated setting's two letters are arithmetic rather than a choice:
// they count DECLARATION SLOTS, the legacy declarations among them, running
// "aa" through "az" and then "ba". Against legacy orders "a" and "b" that
// puts a generated setting in any of the first twenty-six slots between the
// two dropdowns, and the twenty-seventh declaration carries "ba" and lands
// past "b". Naming "a" here puts what follows under the legacy "a" instead,
// with the generated name and therefore the stored value untouched, which is
// the placement a plan used to have to reach for a Legacy constructor and a
// hand-written prefixed name to get.
//
// THE TWO LETTERS STAY on the end rather than being replaced by the given
// string, because they are what keeps declaration order visible: the settings
// under one prefix still sort the way the consumer wrote them, and two
// settings sharing a prefix do not collapse onto one order string, which
// would be the engine placing them rather than the consumer.
//
// THE PREFIX IS CAPTURED AT DECLARATION rather than read when the plan is
// emitted, because it is a property of WHERE THE CALL SITE SITS in the
// declaration sequence. A planner reading the last value set would hand every
// generated setting in the plan the same prefix, and a second call would
// silently move the settings that the first call already placed.
//
// An empty order is refused at the settings stage rather than here: a
// declaration method has no error to return, so the plan carries the mistake
// to the validator that can name it.
func (l *Lib) OrderAfter(order string) {
	if order == "" {
		l.orderAfterEmpty = true
	}
	l.orderPrefix = order
}

// BoolSetting declares a startup bool setting. The name is prefixed on the
// way out; what is passed here is the bare name.
func (l *Lib) BoolSetting(name string, def bool) BoolSettingRef {
	l.settings = append(l.settings, settingDecl{kind: settingBool, name: name, orderPrefix: l.orderPrefix, defBool: def})
	return BoolSettingRef{lib: l.id, index: len(l.settings)}
}

// IntSetting declares a startup int setting.
func (l *Lib) IntSetting(name string, def int64, spec NumericSpec) IntSettingRef {
	l.settings = append(l.settings, settingDecl{kind: settingInt, name: name, orderPrefix: l.orderPrefix, defNum: float64(def), defInt: def, spec: spec})
	return IntSettingRef{lib: l.id, index: len(l.settings)}
}

// DoubleSetting declares a startup double setting.
func (l *Lib) DoubleSetting(name string, def float64, spec NumericSpec) DoubleSettingRef {
	l.settings = append(l.settings, settingDecl{kind: settingDouble, name: name, orderPrefix: l.orderPrefix, defNum: def, spec: spec})
	return DoubleSettingRef{lib: l.id, index: len(l.settings)}
}

// DropdownSettingNeedingLocale declares a startup string setting with a fixed
// list of allowed values.
//
// THE NAME IS THE WARNING. A bool, int or double setting localises from its
// own name; a dropdown's VALUES have no inline mechanism at all, so every
// entry in values needs a locale line in the CONSUMER's own .cfg or the
// player sees a raw key in the settings screen. Nothing this library emits
// can supply them. Prefer a bool, int or double setting when the choice fits
// one; reach for this when it does not, and ship the locale entries.
func (l *Lib) DropdownSettingNeedingLocale(name string, def string, values []string) DropdownSettingRef {
	l.settings = append(l.settings, settingDecl{kind: settingDropdown, name: name, orderPrefix: l.orderPrefix, defStr: def, values: copyStrings(values)})
	return DropdownSettingRef{lib: l.id, index: len(l.settings)}
}

// IngredientsSetting declares a startup text setting holding a recipe's
// ingredient list, in the language docs/ingredient-list.md describes.
//
// THE SETTING'S DEFAULT TEXT IS THE WORD default, NOT THE RENDERED LIST, and
// that is the decision the whole shape rests on. The engine stores every
// setting's current value in mod-settings.dat, untouched defaults included
// (measured), so a mod that changed its declared list would turn every player
// who never opened the settings screen into a player with a hand-typed list:
// frozen at the old balance, and refusing to load in a modpack that lacks a
// rung of a ladder they never saw. The word keeps meaning "the list this mod
// declares, with its ladders" across releases, and the declared list is
// written out in the setting's description so the player can copy it.
//
// The list is bound to exactly one recipe, through RecipeSpec.IngredientsFrom
// or as the Custom arm of an IngredientChoices dropdown. A setting nothing
// reads is refused: a field the player can edit that changes nothing is a
// declaration mistake, not a feature.
func (l *Lib) IngredientsSetting(name string, def []Ingredient) IngredientsSettingRef {
	return l.ingredientsSetting(name, false, def, "")
}

// LegacyIngredientsSetting declares an ingredient-list setting under a name
// this mod ALREADY SHIPS, emitted verbatim with the given order. See the note
// above the Legacy constructors.
func (l *Lib) LegacyIngredientsSetting(fullName string, def []Ingredient, order string) IngredientsSettingRef {
	return l.ingredientsSetting(fullName, true, def, order)
}

func (l *Lib) ingredientsSetting(name string, legacy bool, def []Ingredient, order string) IngredientsSettingRef {
	// ONE OF THE TWO PLACES THE LANGUAGE IS NAMED, packsSetting being the
	// other. Everything else in the library reaches the parser, the renderer
	// and the amount formatter through these values, so a plan that calls
	// neither constructor names none of them and TinyGo drops all three. See
	// language for the measurement. Idempotent: a plan may declare many text
	// settings, and the language is one for the whole plan.
	if l.lang == nil {
		l.lang = &language{parse: parseIngredientList, render: renderIngredientList, amount: formatListAmount}
	}
	l.settings = append(l.settings, settingDecl{
		kind: settingIngredients, name: name, legacy: legacy, order: order,
		orderPrefix:    l.orderPrefix,
		defIngredients: copyIngredients(def),
	})
	return IngredientsSettingRef{lib: l.id, index: len(l.settings)}
}

// PacksSetting declares a startup text setting holding a research cost's
// science packs, read with the same language and the same reserved word. Only
// items the engine treats as science packs (prototype type tool) resolve, and
// the word none is refused: a research with no packs is not something this
// library emits on an author's behalf.
//
// It is bound through TechSpec.CostFrom or as the Custom arm of a CostChoices
// dropdown, together with an int setting for the count and a double setting
// for the seconds. See CustomCost.
func (l *Lib) PacksSetting(name string, def []Pack) PacksSettingRef {
	return l.packsSetting(name, false, def, "")
}

// LegacyPacksSetting declares a pack-list setting under a name this mod
// ALREADY SHIPS, emitted verbatim with the given order. See the note above the
// Legacy constructors.
func (l *Lib) LegacyPacksSetting(fullName string, def []Pack, order string) PacksSettingRef {
	return l.packsSetting(fullName, true, def, order)
}

func (l *Lib) packsSetting(name string, legacy bool, def []Pack, order string) PacksSettingRef {
	// THE OTHER PLACE THE LANGUAGE IS NAMED, and the ONLY place the
	// custom-cost resolver is: a CustomCost holds a PacksSettingRef, which
	// only this constructor issues, so a plan that never reaches here can
	// never reach resolveCustomCost either. See ingredientsSetting and
	// language. Both installs are idempotent for the same reason.
	if l.lang == nil {
		l.lang = &language{parse: parseIngredientList, render: renderIngredientList, amount: formatListAmount}
	}
	if l.customCost == nil {
		l.customCost = (*Lib).resolveCustomCost
	}
	l.settings = append(l.settings, settingDecl{
		kind: settingPacks, name: name, legacy: legacy, order: order,
		orderPrefix: l.orderPrefix,
		defPacks:    copyPacks(def),
	})
	return PacksSettingRef{lib: l.id, index: len(l.settings)}
}

// CustomCost is a research cost with its three numbers in the PLAYER's hands:
// the science packs as a text setting, the count as an int setting and the
// seconds as a double setting.
//
// THE TWO NUMERIC SETTINGS MUST DECLARE MINIMA, and the library refuses a
// CustomCost whose settings do not: the engine refuses a unit with a count of
// 0 ("ResearchIngredient's amount must not be 0" for a pack, "time must be
// positive" for the time, both measured), and a numeric setting whose stored
// value falls outside its own bounds is RESET to the default rather than
// clamped (measured). A minimum of at least 1 on the count and above 0 on the
// seconds is therefore what makes every value the data stage can read legal,
// without the data stage having to guess what to do with an illegal one.
//
// Position is the prerequisite ladder, and it belongs to a Custom arm: under
// CostChoices.Custom the chosen tier's source technology would have been the
// prerequisite, so the custom arm names its own ladder and the first rung the
// game has becomes the sole prerequisite. Under TechSpec.CostFrom the ordinary
// placement fields say where the technology goes, and a Position there is
// refused rather than quietly ignored.
type CustomCost struct {
	Packs    PacksSettingRef
	Count    IntSettingRef
	Seconds  DoubleSettingRef
	Position []string
}

// ItemSpec describes a generated item prototype. The integer fields are
// int64 because TinyGo's wasm int is 32 bits while the Rust mirror's is 64:
// a width that differs between the halves is a transcript that differs.
type ItemSpec struct {
	Icon        string
	IconSize    int64 // zero omits the field; the engine's own default is 64
	StackSize   int64 // zero means 50
	Subgroup    string
	DisplayName string
	Description string

	// Order is the sort key inside the subgroup. Empty omits the field and
	// lets the engine order by name.
	Order string

	// PlaceResult is the entity this item builds, by name. It is PRESENCE
	// PROBED like every other name this library emits: an item naming an
	// entity the game does not have aborts the load with the engine's own
	// assignID error rather than anything this library could soften, so a
	// missing one is refused at plan time with the name in the message.
	//
	// The entity is somebody's: your own mod's hand-rolled one, or another
	// mod's. This library does not emit entities, so there is nothing here to
	// prefix and nothing to derive.
	PlaceResult string

	// Extra is raw prototype fields, passed through VERBATIM after the ones
	// this library emits, in declaration order. See RecipeSpec.Extra.
	Extra []KV
}

type itemDecl struct {
	name   string
	legacy bool
	spec   ItemSpec
}

func (d itemDecl) emittedName(prefix string) string { return protoName(d.legacy, prefix, d.name) }

// Item declares an item this mod introduces.
func (l *Lib) Item(name string, spec ItemSpec) ItemRef {
	return l.item(name, false, spec)
}

// LegacyItem declares an item under a name this mod ALREADY SHIPS, emitted
// verbatim with no prefix.
//
// THE SAME ARGUMENT AS THE LEGACY SETTINGS, AND STRONGER. A save references
// prototype names directly: an item sits in inventories and on belts by name,
// a technology is recorded as researched by name, a recipe is remembered by
// name in every assembler. A mod's own hand-rolled neighbours name them too,
// and the engine's failure for a dangling reference is not a warning but an
// abort:
//
//	Error in assignID: item with name 'bbb-balancer-part' does not exist.
//
// So a migrating mod cannot rename its prototypes any more than its settings,
// and this is the escape hatch. The Legacy mark in the name is the whole
// documentation of the deviation: a reader sees at the call site that this
// name is not derived, and nothing else in the library can produce one.
//
// The handle is an ORDINARY handle. IngredientOf, Unlocks, AfterTech and the
// splices take it exactly as they take a generated one, so a plan may be part
// legacy and part generated without either half knowing.
func (l *Lib) LegacyItem(fullName string, spec ItemSpec) ItemRef {
	return l.item(fullName, true, spec)
}

func (l *Lib) item(name string, legacy bool, spec ItemSpec) ItemRef {
	spec.Extra = copyKVs(spec.Extra)
	l.items = append(l.items, itemDecl{name: name, legacy: legacy, spec: spec})
	return ItemRef{lib: l.id, index: len(l.items)}
}

// ingredientKind is which namespace an ingredient's name lives in. The engine
// keeps items and fluids apart and spells the difference in the recipe form
// itself ({type="item", ...} against {type="fluid", ...}), so the plan carries
// the answer rather than guessing at emit.
//
// THE ZERO VALUE IS AN ITEM, deliberately: every Ingredient built before
// fluids existed stays exactly what it was, and no existing declaration had to
// be rewritten to gain the field.
type ingredientKind uint8

const (
	kindItem ingredientKind = iota
	kindFluid
)

// Ingredient is one line of a recipe. It is built by IngredientOf,
// IngredientNamed or FluidIngredient and cannot be built from a bare string
// any other way: an ingredient the game does not have is a hard load failure
// naming the consumer's mod, so a name reaches a prototype only after a
// presence probe.
//
// AN ITEM AMOUNT IS AN INTEGER AND A FLUID AMOUNT IS NOT. The engine refuses
// an item count outside 0..65535 and accepts a fractional one only to mean
// something no mod should ship a player, while a fluid takes any positive
// amount, 0.5 included (both measured). Two fields rather than one double
// keeps that difference in the type instead of in a comment.
type Ingredient struct {
	item        ItemRef
	amount      int64
	fluidAmount float64
	kind        ingredientKind
	candidates  []string
}

// IngredientOf names an item this plan declares. It always resolves: the
// prototype is emitted by the same plan.
func IngredientOf(it ItemRef, amount int64) Ingredient {
	return Ingredient{item: it, amount: amount}
}

// IngredientNamed is the presence ladder: the candidates are tried in order
// and the first one the game actually has is used. If none is present the
// ingredient is DROPPED with a log line, never guessed at, because a wrong
// guess is somebody else's overhaul pack failing to load.
func IngredientNamed(amount int64, first string, fallbacks ...string) Ingredient {
	candidates := make([]string, 0, 1+len(fallbacks))
	candidates = append(candidates, first)
	candidates = append(candidates, fallbacks...)
	return Ingredient{amount: amount, candidates: candidates}
}

// FluidIngredient names a FLUID by the same presence ladder as
// IngredientNamed: the candidates are tried in order, the first one the game
// actually has is used, and an ingredient no candidate resolves is dropped
// with a log line rather than guessed at.
//
// THERE IS NO HANDLE ARM. This library declares items and never fluids, so
// every fluid it can name is somebody else's and the ladder is the only way
// to name one. That is also why the amount is a plain double: the engine takes
// any positive amount for a fluid, and a fluid ingredient in a recipe whose
// category is crafting is refused by the planner with the engine's own rule
// before it can become a load failure.
func FluidIngredient(amount float64, first string, fallbacks ...string) Ingredient {
	candidates := make([]string, 0, 1+len(fallbacks))
	candidates = append(candidates, first)
	candidates = append(candidates, fallbacks...)
	return Ingredient{kind: kindFluid, fluidAmount: amount, candidates: candidates}
}

// IngredientChoice is one dropdown value and the ingredients it selects.
type IngredientChoice struct {
	Value       string
	Ingredients []Ingredient
}

// IngredientChoices binds a recipe's ingredients to a dropdown setting: the
// player picks a value and the matching plan is what the recipe is made of.
//
// Every plan is resolved by the ordinary ladder rules, so a plan may name
// things another mod provides. A chosen plan that resolves to nothing falls
// back to the DEFAULT option's plan, with a line saying so, rather than
// emitting a recipe made of nothing.
type IngredientChoices struct {
	Setting DropdownSettingRef
	Choices []IngredientChoice

	// CustomValue is the dropdown value that hands the ingredients to a text
	// setting the player writes. Empty means the word custom.
	//
	// IT IS A FIELD RATHER THAN A CONSTANT because a mod that already ships a
	// dropdown may already have a value literally named custom, and Factorio
	// keys a stored choice by its value: renaming one discards what every
	// player had chosen, which is exactly the loss the migration path exists to
	// avoid. This commit declares the field and copies it; the commit that
	// binds a text setting is where it starts selecting anything.
	CustomValue string

	// Custom hands the ingredients to a text setting the player writes, under
	// the value CustomValue names. The dropdown lists that value and Choices
	// do NOT cover it: it is the one value with no author-written plan behind
	// it.
	//
	// The zero handle is no custom arm at all, and then the dropdown may not
	// list a value named custom: a player choosing it would get a recipe made
	// of nothing.
	Custom IngredientsSettingRef
}

// CostChoice is one dropdown value and the technologies whose cost it selects,
// in ladder order.
type CostChoice struct {
	Value   string
	Sources []string
}

// CostChoices binds a technology's research cost to a dropdown setting.
//
// THE PREREQUISITE MOVES WITH THE UNIT. The source whose cost is copied also
// becomes the technology's sole prerequisite, so price and tree position come
// from one named point. That is the rule this surface exists to make easy, and
// it is why CostBy does not combine with After, Before or AfterTech.
//
// Fallback is what applies when no source in the chosen ladder carries a unit
// this library can copy. It is a hand-rolled cost, validated exactly like one,
// and a technology that falls back has no prerequisite at all.
type CostChoices struct {
	Setting  DropdownSettingRef
	Choices  []CostChoice
	Fallback UnitSpec

	// CustomValue is the dropdown value that hands the research cost to
	// settings the player writes. Empty means the word custom, and it is a
	// field for the same reason IngredientChoices.CustomValue is one.
	CustomValue string

	// Custom hands the research cost to settings the player writes, under the
	// value CustomValue names, exactly as IngredientChoices.Custom does for a
	// recipe. Its Position is REQUIRED, because a chosen tier's source
	// technology would have been the prerequisite and the custom arm has no
	// source to take one from.
	Custom *CustomCost
}

// RecipeSpec describes a generated recipe prototype.
type RecipeSpec struct {
	Name string // empty means the result item's name

	// Exactly one of CraftTime and CraftTimeFrom, or neither: a fixed
	// crafting time, or one the PLAYER sets through a generated double
	// setting. Zero and a zero handle both mean "say nothing", and the engine
	// applies its own default.
	//
	// ONLY A DOUBLE SETTING IS BINDABLE, and the handle's type is what says
	// so: there is no int-setting arm to get wrong, because an IntSettingRef
	// does not fit here. That is the same shape EnabledBy uses for a bool.
	CraftTime     float64
	CraftTimeFrom DoubleSettingRef

	// Exactly one of Ingredients, IngredientsBy and IngredientsFrom, or none
	// of them. Ingredients is one fixed list; IngredientsBy lets a dropdown
	// setting choose between several; IngredientsFrom hands the whole list to
	// a text setting the PLAYER writes, in the language
	// docs/ingredient-list.md describes.
	IngredientsBy *IngredientChoices
	Ingredients   []Ingredient

	// IngredientsFrom is the text setting this recipe is made of. The zero
	// handle means absent. See IngredientsSetting: the setting's text starts
	// out as the word default, which means the list declared there with its
	// ladders.
	IngredientsFrom IngredientsSettingRef
	ResultCount     int64 // zero means 1
	Category        string
	DisplayName     string
	Description     string

	// ResultNamed is an EXISTING item this recipe produces, for a recipe
	// whose result this plan does not declare. Give a zero ItemRef and this
	// name; the item is presence probed at emit and refused if absent, and
	// Name is then required, because there is no declared item to inherit it
	// from.
	ResultNamed string

	// Order is the sort key inside the recipe group. Empty omits the field.
	Order string

	// Extra is raw prototype fields, passed through VERBATIM after the ones
	// this library emits, in declaration order.
	//
	// THE VALUES ARE YOURS AND ARE NOT TOUCHED. Nothing inside an Extra value
	// is prefixed, and no name inside one is presence probed: this library
	// cannot know which strings in an arbitrary field are prototype names, so
	// guessing would be worse than the passthrough. If a field holds a name
	// the game may not have, you own that check.
	//
	// A key this library emits itself is REFUSED rather than merged or
	// overridden, because two writers of one field is a silent last-writer
	// and the loser would be whichever order this library happens to use.
	//
	// enabled IS THE ONE EXCEPTION, for a mod migrating one prototype at a
	// time: it is accepted while no technology in this plan unlocks this
	// recipe, and the value you give is emitted where the library's own
	// enabled would have gone rather than at the end. Declare a technology
	// that unlocks the recipe and the field goes back to the library, with a
	// refusal naming that technology.
	Extra []KV
}

type recipeDecl struct {
	name   string
	legacy bool
	result ItemRef
	spec   RecipeSpec
}

func (d recipeDecl) emittedName(prefix string) string { return protoName(d.legacy, prefix, d.name) }

// Recipe declares a recipe producing an item this plan declares.
func (l *Lib) Recipe(result ItemRef, spec RecipeSpec) RecipeRef {
	name := spec.Name
	if name == "" && l.validItem(result) {
		// The RESULT's declared name, not its emitted one: this is the
		// recipe's own unprefixed name, and the prefix is applied to it at
		// emit like any other. A legacy result therefore hands a legacy-shaped
		// name to a GENERATED recipe, which would be wrong; Recipe refuses a
		// legacy result without an explicit name in validate for that reason.
		name = l.items[result.index-1].name
	}
	return l.recipe(name, false, result, spec)
}

// LegacyRecipe declares a recipe under a name this mod ALREADY SHIPS, emitted
// verbatim with no prefix. See LegacyItem for why prototype names cannot be
// regenerated for a mod that has players.
//
// The name is required rather than inherited from the result: a legacy recipe
// and its result item are two independent names the mod already chose, and
// deriving one from the other would be a guess.
func (l *Lib) LegacyRecipe(result ItemRef, fullName string, spec RecipeSpec) RecipeRef {
	return l.recipe(fullName, true, result, spec)
}

func (l *Lib) recipe(name string, legacy bool, result ItemRef, spec RecipeSpec) RecipeRef {
	spec.Ingredients = copyIngredients(spec.Ingredients)
	spec.IngredientsBy = copyIngredientChoices(spec.IngredientsBy)
	spec.Extra = copyKVs(spec.Extra)
	l.recipes = append(l.recipes, recipeDecl{name: name, legacy: legacy, result: result, spec: spec})
	return RecipeRef{lib: l.id, index: len(l.recipes)}
}

// Pack is one science pack of a hand-rolled research cost, with the same
// presence ladder every other name in this library gets.
//
// FALLBACKS EXIST BECAUSE A SCIENCE PACK IS SOMEBODY ELSE'S PROTOTYPE. Name and
// then Fallbacks are tried in order through ToolExists and the first one the
// game has is used; a pack no rung resolves is DROPPED with a log line, exactly
// as an ingredient is, rather than refusing the load of a modpack that renamed
// or removed a pack. A unit whose every pack drops is refused, because research
// with no pack at all is not something this library will emit on an author's
// behalf.
type Pack struct {
	Name      string
	Amount    int64
	Fallbacks []string
}

// ladder is the whole candidate list, first rung first. Built rather than
// stored so that a Pack written as a plain literal, which is how nearly every
// one of them is written, needs no constructor.
func (p Pack) ladder() []string {
	out := make([]string, 0, 1+len(p.Fallbacks))
	out = append(out, p.Name)
	return append(out, p.Fallbacks...)
}

// UnitSpec is the hand-rolled research cost, the ESCAPE HATCH. Prefer CostOf:
// it takes cost and tree position from one named technology, which is the
// rule this surface is built to make easy.
type UnitSpec struct {
	Count   int64
	Seconds float64
	Packs   []Pack
}

// TechSpec describes a generated technology prototype.
type TechSpec struct {
	// Order is the sort key in the technology screen. Empty omits the field.
	Order string

	// Extra is raw prototype fields, passed through VERBATIM after the ones
	// this library emits, in declaration order. See RecipeSpec.Extra.
	Extra []KV

	Icon     string
	IconSize int64

	// Exactly one of CostOf, Unit, CostBy and CostFrom. CostOf copies a named
	// technology's whole unit verbatim, count_formula and all. CostBy lets a
	// dropdown setting choose between several sources, and places the
	// technology as well: see CostChoices. CostFrom is Unit with its three
	// numbers in the player's hands, placed by the ordinary placement fields:
	// see CustomCost.
	CostOf   string
	Unit     *UnitSpec
	CostBy   *CostChoices
	CostFrom *CustomCost

	// Tree placement, and exactly one anchor. After names a technology the
	// GAME has; AfterTech names one THIS PLAN declares, which is how a plan
	// chains its own research. After with Before splices the new technology
	// between the two. Before needs After: it splices around technologies
	// that already exist, so it has nothing to say about a plan's own.
	After     string
	AfterTech TechRef // the zero value is no anchor
	Before    string

	Unlocks   []RecipeRef
	EnabledBy BoolSettingRef // the zero value is no setting at all

	DisplayName string
	Description string
}

type techDecl struct {
	name   string
	legacy bool
	spec   TechSpec
}

func (d techDecl) emittedName(prefix string) string { return protoName(d.legacy, prefix, d.name) }

// protoName is the one place a prototype name is decided. A legacy name is
// whatever the mod ships; everything else derives from the packaged mod.
func protoName(legacy bool, prefix, name string) string {
	if legacy {
		return name
	}
	return prefix + name
}

// Technology declares a technology this mod introduces.
func (l *Lib) Technology(name string, spec TechSpec) TechRef {
	return l.technology(name, false, spec)
}

// LegacyTechnology declares a technology under a name this mod ALREADY SHIPS,
// emitted verbatim with no prefix. See LegacyItem for why prototype names
// cannot be regenerated for a mod that has players.
func (l *Lib) LegacyTechnology(fullName string, spec TechSpec) TechRef {
	return l.technology(fullName, true, spec)
}

func (l *Lib) technology(name string, legacy bool, spec TechSpec) TechRef {
	spec.Unlocks = copyRecipeRefs(spec.Unlocks)
	spec.Unit = copyUnit(spec.Unit)
	spec.CostBy = copyCostChoices(spec.CostBy)
	spec.CostFrom = copyCustomCost(spec.CostFrom)
	spec.Extra = copyKVs(spec.Extra)
	l.techs = append(l.techs, techDecl{name: name, legacy: legacy, spec: spec})
	return TechRef{lib: l.id, index: len(l.techs)}
}

// A handle is valid only for the plan that issued it: the id keeps an
// in-range index from another plan out.

func (l *Lib) validItem(r ItemRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.items)
}

func (l *Lib) validRecipe(r RecipeRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.recipes)
}

func (l *Lib) validTech(r TechRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.techs)
}

func (l *Lib) validBoolSetting(r BoolSettingRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.settings)
}

func (l *Lib) validDoubleSetting(r DoubleSettingRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.settings)
}

// craftTimeBoundSettings marks the double settings some recipe reads its
// crafting time from. The settings stage needs it too, which is why the
// binding lives in the plan rather than in the data pass: the generated
// setting's minimum depends on what it backs.
//
// A handle from another plan is SKIPPED rather than followed, so a bad
// reference cannot mark the wrong setting here; PlanData is what refuses it
// by name.
func (l *Lib) craftTimeBoundSettings() []bool {
	bound := make([]bool, len(l.settings))
	for _, r := range l.recipes {
		if l.validDoubleSetting(r.spec.CraftTimeFrom) {
			bound[r.spec.CraftTimeFrom.index-1] = true
		}
	}
	return bound
}

// effectiveNumericSpec is the setting's bounds as EMITTED: a craft-time-bound
// double with no minimum of its own gets the floor-safe one. The auto-minimum
// is a real bound and is validated exactly like a declared one.
func (l *Lib) effectiveNumericSpec(i int, bound []bool) NumericSpec {
	spec := l.settings[i].spec
	if bound[i] && !spec.HasMin {
		spec.HasMin = true
		spec.Min = craftTimeAutoMinimum
	}
	return spec
}

// A plan is a snapshot taken at the moment of declaration: everything a spec
// hands over that the caller still holds a reference to is COPIED here. A
// consumer reusing one ingredient buffer across several recipes is ordinary
// Go, and without these copies their second recipe would rewrite their first.
// The Rust mirror needs none of it: its specs are moved, Vec and all.

// copyKVs snapshots an Extra list, VALUES AND ALL.
//
// A shallow copy is not enough here, and Extra is the one place in the library
// where that matters. Every other spec slice holds flat structs, so copying
// the backbone copies everything; a Value holds Arr and Map SLICE HEADERS, so
// a Pair carrying a container would leave the plan pointing at the caller's
// backing array. Building the value inline hides it, but the shape a consumer
// actually writes is a slice built up first and passed second:
//
//	flags := []Value{Str("hidden")}
//	lib.Item("part", ItemSpec{Extra: []KV{Pair("flags", Arr(flags...))}})
//	flags[0] = Str("something-else")   // would rewrite the declared plan
//
// THE RUST MIRROR NEEDS NONE OF THIS, and that asymmetry is by construction
// rather than by omission: its extra is a Vec<(String, Value)> that is MOVED
// into the declaration, so the caller has nothing left to mutate and an
// aliasing bug of this shape cannot be written.
func copyKVs(in []KV) []KV {
	if in == nil {
		return nil
	}
	out := make([]KV, len(in))
	for i, e := range in {
		out[i] = KV{Key: e.Key, Val: copyValue(e.Val)}
	}
	return out
}

// copyValue deep-copies a value's containers. Kind, Bool, Num and Str are
// copied by the assignment itself: a Go string is immutable, so it needs no
// copy of its own.
func copyValue(v Value) Value {
	out := v
	if v.Arr != nil {
		out.Arr = make([]Value, len(v.Arr))
		for i, item := range v.Arr {
			out.Arr[i] = copyValue(item)
		}
	}
	if v.Map != nil {
		out.Map = make([]KV, len(v.Map))
		for i, e := range v.Map {
			out.Map[i] = KV{Key: e.Key, Val: copyValue(e.Val)}
		}
	}
	return out
}

func copyStrings(in []string) []string {
	if in == nil {
		return nil
	}
	out := make([]string, len(in))
	copy(out, in)
	return out
}

func copyIngredients(in []Ingredient) []Ingredient {
	if in == nil {
		return nil
	}
	out := make([]Ingredient, len(in))
	for i, ing := range in {
		ing.candidates = copyStrings(ing.candidates)
		out[i] = ing
	}
	return out
}

func copyRecipeRefs(in []RecipeRef) []RecipeRef {
	if in == nil {
		return nil
	}
	out := make([]RecipeRef, len(in))
	copy(out, in)
	return out
}

// The unit arrives as a pointer, so the caller keeps a handle on the whole
// cost after Technology returns.
func copyUnit(in *UnitSpec) *UnitSpec {
	if in == nil {
		return nil
	}
	out := *in
	out.Packs = copyPacks(in.Packs)
	return &out
}

// copyPacks is a DEEP copy: a Pack carries a ladder of its own now, and a
// shallow row copy would leave the caller holding the same backing array for
// every fallback list in the plan.
func copyPacks(in []Pack) []Pack {
	if in == nil {
		return nil
	}
	out := make([]Pack, len(in))
	for i, p := range in {
		p.Fallbacks = copyStrings(p.Fallbacks)
		out[i] = p
	}
	return out
}

// ---------------------------------------------------------------------------
// The Legacy constructors.
//
// THESE ARE FOR MIGRATING A MOD THAT ALREADY SHIPPED. Factorio persists a
// player's startup choices in mod-settings.dat keyed by the setting's NAME,
// and it has no rename mechanism: a setting that comes back under a different
// name is a new setting, and every player who had chosen a value gets the
// default instead. A mod whose settings predate this library therefore cannot
// adopt the generated names without discarding what its players chose.
//
// So the name crosses VERBATIM, with no prefix, and the order string is the
// consumer's own because a historic mod picked its own (generated settings get
// two letters from declaration order; a mod that shipped "a" and "b" keeps
// them, and a settings dump hash pins that).
//
// THE INVARIANT THIS BENDS, SAID PLAINLY. Everywhere else in this library an
// unprefixed name is unrepresentable. Here it is representable through a
// constructor whose name says Legacy, which is the same signposting
// DropdownSettingNeedingLocale uses: the deviation is in the call site, where
// a reviewer sees it.
//
// A NEW SETTING USES THE PREFIXED CONSTRUCTORS. Nothing about these is a
// shortcut around the prefix; they exist so a migration can preserve values,
// and a mod with no shipped settings has nothing to preserve.
// ---------------------------------------------------------------------------

// LegacyBoolSetting declares a bool setting under a name this mod already
// ships. See the note above the Legacy constructors.
func (l *Lib) LegacyBoolSetting(fullName string, def bool, order string) BoolSettingRef {
	l.settings = append(l.settings, settingDecl{
		kind: settingBool, name: fullName, legacy: true, order: order, orderPrefix: l.orderPrefix, defBool: def,
	})
	return BoolSettingRef{lib: l.id, index: len(l.settings)}
}

// LegacyIntSetting declares an int setting under a name this mod already
// ships. See the note above the Legacy constructors.
func (l *Lib) LegacyIntSetting(fullName string, def int64, spec NumericSpec, order string) IntSettingRef {
	l.settings = append(l.settings, settingDecl{
		kind: settingInt, name: fullName, legacy: true, order: order, orderPrefix: l.orderPrefix,
		defNum: float64(def), defInt: def, spec: spec,
	})
	return IntSettingRef{lib: l.id, index: len(l.settings)}
}

// LegacyDoubleSetting declares a double setting under a name this mod already
// ships. See the note above the Legacy constructors.
func (l *Lib) LegacyDoubleSetting(fullName string, def float64, spec NumericSpec, order string) DoubleSettingRef {
	l.settings = append(l.settings, settingDecl{
		kind: settingDouble, name: fullName, legacy: true, order: order, orderPrefix: l.orderPrefix,
		defNum: def, spec: spec,
	})
	return DoubleSettingRef{lib: l.id, index: len(l.settings)}
}

// LegacyDropdownSettingNeedingLocale declares a string setting under a name
// this mod already ships. See the note above the Legacy constructors, and the
// note on DropdownSettingNeedingLocale: the values still need locale entries,
// and a migrated mod already has them under exactly these keys.
func (l *Lib) LegacyDropdownSettingNeedingLocale(fullName string, def string, values []string, order string) DropdownSettingRef {
	l.settings = append(l.settings, settingDecl{
		kind: settingDropdown, name: fullName, legacy: true, order: order, orderPrefix: l.orderPrefix,
		defStr: def, values: copyStrings(values),
	})
	return DropdownSettingRef{lib: l.id, index: len(l.settings)}
}

func copyIngredientChoices(in *IngredientChoices) *IngredientChoices {
	if in == nil {
		return nil
	}
	out := *in
	out.Choices = make([]IngredientChoice, len(in.Choices))
	for i, c := range in.Choices {
		c.Ingredients = copyIngredients(c.Ingredients)
		out.Choices[i] = c
	}
	return &out
}

func copyCostChoices(in *CostChoices) *CostChoices {
	if in == nil {
		return nil
	}
	out := *in
	out.Choices = make([]CostChoice, len(in.Choices))
	for i, c := range in.Choices {
		c.Sources = copyStrings(c.Sources)
		out.Choices[i] = c
	}
	out.Fallback.Packs = copyPacks(in.Fallback.Packs)
	out.Custom = copyCustomCost(in.Custom)
	return &out
}

// copyCustomCost snapshots a custom research cost. The three handles are flat
// values; Position is the caller's slice and is copied for the same reason
// every other declared slice is.
func copyCustomCost(in *CustomCost) *CustomCost {
	if in == nil {
		return nil
	}
	out := *in
	out.Position = copyStrings(in.Position)
	return &out
}

func (l *Lib) validDropdownSetting(r DropdownSettingRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.settings)
}

func (l *Lib) validIntSetting(r IntSettingRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.settings)
}

// THE TWO TEXT VALIDATORS ASK THE KIND AS WELL, and they are the only ones
// that do. Following one of these handles is what reaches the ingredient
// language, and the guard in validateTextSettings decides on the setting's
// KIND; a handle that pointed at a setting of another kind would be followed
// by a reach the guard never looked at, which with the language held as
// function values is a call into nothing. So the composition and the validator
// share one condition here too: a followed handle names a text setting, and a
// text setting has been past the guard.

func (l *Lib) validIngredientsSetting(r IngredientsSettingRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.settings) &&
		l.settings[r.index-1].kind == settingIngredients
}

func (l *Lib) validPacksSetting(r PacksSettingRef) bool {
	return r.lib == l.id && r.index >= 1 && r.index <= len(l.settings) &&
		l.settings[r.index-1].kind == settingPacks
}

// customValue is the dropdown value a Custom arm answers to. Empty means the
// word custom, which is what almost every mod will use; the field exists for
// the mod that already ships a preset under that name and would lose every
// stored preference by renaming it.
func (c *IngredientChoices) customValue() string {
	if c.CustomValue == "" {
		return defaultCustomValue
	}
	return c.CustomValue
}

func (c *CostChoices) customValue() string {
	if c.CustomValue == "" {
		return defaultCustomValue
	}
	return c.CustomValue
}

// defaultCustomValue is the dropdown value a Custom arm takes when the author
// names none, and the value a dropdown WITHOUT an arm may not offer.
const defaultCustomValue = "custom"

// coversValue reports whether an author-written choice already claims a value.
func coversIngredientValue(choices []IngredientChoice, value string) bool {
	for _, c := range choices {
		if c.Value == value {
			return true
		}
	}
	return false
}

func coversCostValue(choices []CostChoice, value string) bool {
	for _, c := range choices {
		if c.Value == value {
			return true
		}
	}
	return false
}

// countValue is how many times a dropdown offers a value. The Custom arm needs
// exactly one: none is an arm nothing can select, and two is an allowed_values
// list the engine would keep the last of.
func countValue(values []string, want string) int {
	n := 0
	for _, v := range values {
		if v == want {
			n++
		}
	}
	return n
}

// withoutValue is the allowed-value list a Custom arm's Choices are compared
// against: every value but the one the arm answers to.
func withoutValue(values []string, drop string) []string {
	out := make([]string, 0, len(values))
	for _, v := range values {
		if v == drop {
			continue
		}
		out = append(out, v)
	}
	return out
}
