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
)

type settingDecl struct {
	kind    settingKind
	name    string
	defBool bool
	defNum  float64
	// The int setting's default as it was DECLARED. defNum has already been
	// through float64 by the time validation runs, so the one number that
	// could have rounded on the way in is no longer there to check.
	defInt int64
	defStr string
	spec   NumericSpec
	values []string
}

// BoolSetting declares a startup bool setting. The name is prefixed on the
// way out; what is passed here is the bare name.
func (l *Lib) BoolSetting(name string, def bool) BoolSettingRef {
	l.settings = append(l.settings, settingDecl{kind: settingBool, name: name, defBool: def})
	return BoolSettingRef{lib: l.id, index: len(l.settings)}
}

// IntSetting declares a startup int setting.
func (l *Lib) IntSetting(name string, def int64, spec NumericSpec) IntSettingRef {
	l.settings = append(l.settings, settingDecl{kind: settingInt, name: name, defNum: float64(def), defInt: def, spec: spec})
	return IntSettingRef{lib: l.id, index: len(l.settings)}
}

// DoubleSetting declares a startup double setting.
func (l *Lib) DoubleSetting(name string, def float64, spec NumericSpec) DoubleSettingRef {
	l.settings = append(l.settings, settingDecl{kind: settingDouble, name: name, defNum: def, spec: spec})
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
	l.settings = append(l.settings, settingDecl{kind: settingDropdown, name: name, defStr: def, values: copyStrings(values)})
	return DropdownSettingRef{lib: l.id, index: len(l.settings)}
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
}

type itemDecl struct {
	name string
	spec ItemSpec
}

// Item declares an item this mod introduces.
func (l *Lib) Item(name string, spec ItemSpec) ItemRef {
	l.items = append(l.items, itemDecl{name: name, spec: spec})
	return ItemRef{lib: l.id, index: len(l.items)}
}

// Ingredient is one line of a recipe. It is built by IngredientOf or
// IngredientNamed and cannot be built from a bare string any other way: an
// ingredient the game does not have is a hard load failure naming the
// consumer's mod, so a name reaches a prototype only after a presence probe.
type Ingredient struct {
	item       ItemRef
	amount     int64
	candidates []string
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
	Ingredients   []Ingredient
	ResultCount   int64 // zero means 1
	Category      string
	DisplayName   string
	Description   string
}

type recipeDecl struct {
	name   string
	result ItemRef
	spec   RecipeSpec
}

// Recipe declares a recipe producing an item this plan declares.
func (l *Lib) Recipe(result ItemRef, spec RecipeSpec) RecipeRef {
	name := spec.Name
	if name == "" && l.validItem(result) {
		name = l.items[result.index-1].name
	}
	spec.Ingredients = copyIngredients(spec.Ingredients)
	l.recipes = append(l.recipes, recipeDecl{name: name, result: result, spec: spec})
	return RecipeRef{lib: l.id, index: len(l.recipes)}
}

// Pack is one science pack of a hand-rolled research cost.
type Pack struct {
	Name   string
	Amount int64
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
	Icon     string
	IconSize int64

	// Exactly one of CostOf and Unit. CostOf copies a named technology's
	// whole unit verbatim, count_formula and all.
	CostOf string
	Unit   *UnitSpec

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
	name string
	spec TechSpec
}

// Technology declares a technology this mod introduces.
func (l *Lib) Technology(name string, spec TechSpec) TechRef {
	spec.Unlocks = copyRecipeRefs(spec.Unlocks)
	spec.Unit = copyUnit(spec.Unit)
	l.techs = append(l.techs, techDecl{name: name, spec: spec})
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
	if in.Packs != nil {
		out.Packs = make([]Pack, len(in.Packs))
		copy(out.Packs, in.Packs)
	}
	return &out
}
