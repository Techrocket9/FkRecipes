package fkrecipes

// World is everything the planner is allowed to know about the game outside
// its own plan. The emit layer implements it over fkdata; host tests
// implement it over fixtures, which is the whole point of the interface.
//
// Every method is a QUESTION, never a write: a planner that could mutate
// could not be replayed, and the plan is what the two languages compare.
type World interface {
	Named

	// StartupSetting reads a startup setting by its FULL, prefixed name. The
	// second result is false when no such setting is readable, which the
	// planner degrades to the declared default plus a log line.
	StartupSetting(name string) (Value, bool)

	// TechNames lists every technology in the game, SORTED. The caller
	// guarantees the sort; the cycle walk's determinism rests on it.
	TechNames() []string

	// TechPrereqs is one technology's prerequisite list, in its own order.
	//
	// STRING ENTRIES ONLY. An entry that is not a string is invisible to this
	// library, so a splice that rewrites the list drops it. The engine refuses
	// a non-string prerequisite anyway, so such an entry is somebody else's
	// load failure already, not one this rewrite introduces.
	TechPrereqs(name string) []string

	// TechUnit is a technology's whole unit, copied verbatim by CostOf. Read
	// the whole map and check the result: 32 of 275 base technologies are
	// research_trigger technologies with no unit at all.
	TechUnit(name string) (Value, bool)

	// TechMaxLevel is a technology's max_level, which lives on the TECHNOLOGY
	// and not in its unit: a verbatim unit copy carries count_formula but
	// cannot carry the level cap, so CostOf reads it from here. A Value
	// because the engine's field is either a number or the string "infinite".
	TechMaxLevel(name string) (Value, bool)

	// TechHasResearchTrigger reports the research_trigger case directly, so
	// CostOf can refuse with a message instead of copying an absent unit.
	TechHasResearchTrigger(name string) bool

	// TechExists answers the prerequisite-presence question. Every dangling
	// prerequisite is a hard load failure naming the CONSUMER's mod, so the
	// planner asks before it names.
	TechExists(name string) bool

	// ItemExists answers the same question for ingredients and science packs.
	ItemExists(name string) bool

	// FluidExists answers it for a fluid ingredient, which is a SEPARATE
	// namespace: data.raw.fluid is not reachable from the item family at all,
	// so a name that is not an item may still be a fluid, and a name that is
	// both is two different prototypes. An untagged name in an ingredient
	// list is asked of ItemExists first and of this only then, which is the
	// tie rule the reference states.
	FluidExists(name string) bool

	// ToolExists answers whether a name is a SCIENCE PACK, and THE NAME IS
	// HISTORICAL. The question it asks is "is this a science pack the running
	// engine's research units accept", which is not the same probe on the two
	// engines this library has been measured on, and the method kept its name
	// because a consumer's fixture World implements it.
	//
	// MEASURED (Factorio 2.0.77 build 84539): a research unit takes tool-type
	// items and nothing else; an item ingredient refuses with "Invalid
	// research unit (iron-plate). Research unit(s) can only be tool type items
	// at the moment."
	//
	// MEASURED (Factorio 2.1.17 build 87315): data.raw.tool DOES NOT EXIST,
	// base's science packs are data.raw.item entries with subgroup
	// "science-pack", and technology.logistics.unit.ingredients names one of
	// those items. So the 2.0 sentence is a 2.0 rule. The emit layer answers
	// this method on both engines; see researchUnitTakesItems below for the
	// key and for what the 2.1 engine really gates on.
	//
	// A pack list asks this rather than ItemExists either way, and a name it
	// answers no for gets a sentence of its own. It is also the question a
	// declared Pack's ladder walks, so a fixture that names a science pack
	// only in its items will see every hand-rolled research cost lose its
	// packs.
	ToolExists(name string) bool

	// EntityExists answers it for ItemSpec.PlaceResult. The engine's failure
	// for an item naming an entity that is not there is an assignID abort
	// naming the item, so this probe is what turns that into a sentence
	// naming the declaration instead.
	EntityExists(name string) bool

	// RecipeExists is asked about the plan's OWN recipe names: a plan that
	// would overwrite an existing prototype is refused, which is also what
	// keeps every planned name new for the cycle overlay.
	RecipeExists(name string) bool
}

// Named is the one question a SETTINGS plan asks.
//
// PlanSettings takes this rather than the whole World, because it needs the
// mod name and nothing else: a consumer holding their own settings plan up to
// the light in a host test implements ONE method instead of ten. World embeds
// it, so anything that satisfies World still satisfies this and the emit
// layer passes the same value to both planners.
type Named interface {
	// ModName is the packaged mod's name, the sole source of the prefix.
	ModName() string
}

// UnimplementedWorld is the World a fixture EMBEDS. Every method panics
// naming itself, so a fixture implements the questions its own test asks and
// inherits a loud refusal for the rest:
//
//	type myWorld struct {
//		fkrecipes.UnimplementedWorld
//		items []string
//	}
//
//	func (w *myWorld) ModName() string            { return "mymod" }
//	func (w *myWorld) ItemExists(n string) bool   { ... }
//
// WHY IT EXISTS, measured by the pilot. This interface grew EntityExists in
// round two and every consumer's host stub stopped compiling on the same day,
// with a Go error naming a method they had never heard of rather than the
// declaration that needed it. The interface will keep growing; embedding this
// means a method added later costs a consumer nothing until a plan of theirs
// actually asks the question, and then it costs them a panic that names the
// method to write.
//
// IT PANICS RATHER THAN ANSWERING. A zero answer would be a fixture quietly
// claiming the game has no items, which is a green test over a plan that
// dropped every ingredient. The Rust mirror gets the same behaviour from
// default trait methods.
//
// It is not for the emit layer, which answers every question for real.
type UnimplementedWorld struct{}

// The embed is only worth anything if it covers the whole interface, so the
// compiler is asked. A method added to World above without an arm here is a
// build failure in this file rather than in a consumer's fixture.
var _ World = UnimplementedWorld{}

// unimplemented is the one message, so all of them read alike.
func unimplemented(method string) string {
	return "fkrecipes: World." + method + " is not implemented by this fixture"
}

// ModName panics. A fixture that plans anything needs it, because the prefix
// derives from it.
func (UnimplementedWorld) ModName() string { panic(unimplemented("ModName")) }

// StartupSetting panics.
func (UnimplementedWorld) StartupSetting(name string) (Value, bool) {
	panic(unimplemented("StartupSetting"))
}

// TechNames panics.
func (UnimplementedWorld) TechNames() []string { panic(unimplemented("TechNames")) }

// TechPrereqs panics.
func (UnimplementedWorld) TechPrereqs(name string) []string { panic(unimplemented("TechPrereqs")) }

// TechUnit panics.
func (UnimplementedWorld) TechUnit(name string) (Value, bool) { panic(unimplemented("TechUnit")) }

// TechMaxLevel panics.
func (UnimplementedWorld) TechMaxLevel(name string) (Value, bool) {
	panic(unimplemented("TechMaxLevel"))
}

// TechHasResearchTrigger panics.
func (UnimplementedWorld) TechHasResearchTrigger(name string) bool {
	panic(unimplemented("TechHasResearchTrigger"))
}

// TechExists panics.
func (UnimplementedWorld) TechExists(name string) bool { panic(unimplemented("TechExists")) }

// ItemExists panics.
func (UnimplementedWorld) ItemExists(name string) bool { panic(unimplemented("ItemExists")) }

// FluidExists panics.
func (UnimplementedWorld) FluidExists(name string) bool { panic(unimplemented("FluidExists")) }

// ToolExists panics.
func (UnimplementedWorld) ToolExists(name string) bool { panic(unimplemented("ToolExists")) }

// EntityExists panics.
func (UnimplementedWorld) EntityExists(name string) bool { panic(unimplemented("EntityExists")) }

// RecipeExists panics.
func (UnimplementedWorld) RecipeExists(name string) bool { panic(unimplemented("RecipeExists")) }

// StageKind is which of this library's two plans a stage calls for.
//
// The zero value is deliberately neither: a stage this library does not plan
// for has to be distinguishable from one it does, and "not one of ours" is the
// answer for a name that is new, misspelled, or simply unknown to the fkdata
// build in the consumer's module.
type StageKind int

const (
	// StageNone is a stage this library does not plan for.
	StageNone StageKind = iota
	// StageKindSettings is the settings stage, which plans setting prototypes.
	StageKindSettings
	// StageKindData is any data-family stage, which plans everything else.
	StageKindData
)

// StageKindOf maps a stage NAME to the plan it calls for, reporting false for
// a stage this library does not plan for.
//
// WHY THIS IS A PURE FUNCTION AND NOT AN if IN Emit. The emit layer sits
// behind the wasm build gate, so a dispatch written there is a decision no
// host test can reach; this one is testable with plain `go test`, and the
// mapping is where the interesting judgement lives.
//
// THE DEFAULT IS REFUSAL, NOT "PLAN DATA". Emit's first shape read "settings
// plans settings, EVERYTHING ELSE plans data", which is correct for the four
// stages fkdata names today and quietly wrong for any fifth. FkLua leaves
// settings-updates and settings-final-fixes unwired on purpose; if they are
// ever wired, the old shape would run PlanData at a settings stage, where
// data.raw does not exist, and the failure would be a confusing probe error
// rather than a sentence naming the cause. Mapping by name with an explicit
// none means that day is a one-line change here plus a decision about what
// the settings family should do, rather than a silent misroute.
//
// "unknown" is fkdata's own name for a stage id it does not recognise, so it
// is spelled out here alongside the two unwired ones: all three are the same
// answer.
func StageKindOf(name string) (StageKind, bool) {
	switch name {
	case "settings":
		return StageKindSettings, true
	case "data", "data-updates", "data-final-fixes":
		return StageKindData, true
	}
	return StageNone, false
}

// packSubgroup is the item subgroup a science pack carries on an engine whose
// research units take items rather than tools.
//
// MEASURED (Factorio 2.1.17 build 87315, base plus its bundled DLC data,
// `factorio -c <a private config> --mod-directory <a probe mod> --dump-data`):
// every one of base's seven packs is a data.raw.item entry with subgroup
// "science-pack". The same walk over data.raw at the DATA stage finds two
// other items in that subgroup, coin and science, which are not science packs,
// so this probe is a NAMED APPROXIMATION and not the engine's own gate.
//
// THE ENGINE'S OWN GATE IS LAB COVERAGE, and it is not askable here. Measured
// on the same engine, a technology whose unit names iron-plate refuses with
//
//	Technology probe-item-unit: there is no lab that will accept all of the science packs this technology requires.
//	Science packs: iron-plate
//
// and the same refusal comes for a freshly declared item whose subgroup IS
// "science-pack" and which no lab lists, while adding iron-plate to
// data.raw.lab.lab.inputs makes a unit naming iron-plate load with exit 0. So
// the gate is the lab's inputs and the subgroup has nothing to do with it. A
// lab-keyed probe is still the wrong probe for this library, because the data
// stage cannot see the answer: measured in the same run, at the data stage
// base's `lab` lists seven inputs, space-age's five packs are not items yet
// and its biolab does not exist, and all of them arrive by data-final-fixes.
// A lab-keyed probe would therefore drop every pack of any modpack that
// assembles its labs after the data stage, which is a free research per
// technology; the subgroup is visible at the data stage and selects exactly
// base's seven. The 2.0 probe was an approximation of the same kind: it asked
// whether a name was the right TYPE, not whether a lab would take it.
const packSubgroup = "science-pack"

// researchUnitTakesItems answers WHICH OF THE TWO MEASURED ENGINES this is,
// from base's own version, and it is a pure function for the reason
// StageKindOf is: the emit layer sits behind the wasm build gate, so a
// decision written there is one no host test can reach.
//
// WHY THE VERSION AND NOT THE PRESENCE OF data.raw.tool. "Ask data.raw.tool
// where that table exists and data.raw.item where it does not" is the obvious
// key and it is MEASURABLY WRONG. The tool prototype type still exists on 2.1
// (defines.prototypes.item still lists "tool" among its 21 keys, measured),
// and a mod that declares a tool-type prototype loads on 2.1 with exit 0 while
// a technology beside it prices itself in an ITEM, also measured. One ported
// mod still shipping a legacy tool-typed pack would put a table there and take
// every base science pack away from this library, which is the free-research
// outcome this whole fix exists to close. base's version is the engine's, is
// one env read, and says which engine is running whatever the mod set did.
//
// UNREADABLE MEANS THE CURRENT ENGINE. base is always installed, so this arm
// is for a host that answered something this parser cannot read.
//
// WHAT A WRONG ANSWER COSTS, per fact rather than as one claim. On the SCIENCE
// PACK neither way can stop a load: the tool branch on a 2.1 engine and the
// item branch on a 2.0 engine both DROP packs, a free research and disclosed,
// because a 2.0 science pack is not in data.raw.item at all. On the RECIPE
// CATEGORY the wrong answer is worse and is UNMEASURED: emitting `categories`
// to a 2.0 engine either has it ignore an unknown key, which silently puts the
// recipe in `crafting` and refuses a fluid ingredient there, or has it refuse
// the key outright. The 2.0 binary was gone from the machine that found this,
// so neither outcome was probed. That is an argument for keying on something
// always readable, which base's version is, and not a claim that the arm is
// harmless.
func researchUnitTakesItems(baseVersion string) bool {
	major, minor, ok := majorMinor(baseVersion)
	if !ok {
		return true
	}
	return major > 2 || (major == 2 && minor >= 1)
}

// majorMinor reads the leading "<major>.<minor>" of a version string. A
// version with no minor part reads as minor 0, and anything with no leading
// digit at all is not a version this library will key on.
//
// HAND-ROLLED RATHER THAN strconv, because the whole input is two small
// unsigned numbers and an overflow on a hostile string would be a silent wrong
// answer rather than an error: a run of digits longer than four is refused
// here instead.
func majorMinor(v string) (int, int, bool) {
	major, rest, ok := leadingNumber(v)
	if !ok {
		return 0, 0, false
	}
	if len(rest) == 0 || rest[0] != '.' {
		return major, 0, true
	}
	minor, _, ok := leadingNumber(rest[1:])
	if !ok {
		return major, 0, true
	}
	return major, minor, true
}

// leadingNumber reads the run of digits at the front of s, and what follows.
func leadingNumber(s string) (int, string, bool) {
	n := 0
	for n < len(s) && s[n] >= '0' && s[n] <= '9' {
		n++
	}
	if n == 0 || n > 4 {
		return 0, s, false
	}
	value := 0
	for i := 0; i < n; i++ {
		value = value*10 + int(s[i]-'0')
	}
	return value, s[n:], true
}

// recipeCategoriesAreAList is the SECOND engine-keyed fact, and it is keyed by
// the same function for the same reason: the emit layer cannot make this call
// where a host test could see it.
//
// MEASURED (Factorio 2.1.17 build 87315): a recipe prototype carrying
// `category` refuses the load outright, with
//
//	Error while loading recipe prototype "<name>" (recipe): In RecipePrototype, `category` and `additional_categories` got merged into `categories` table. Please use that instead.
//
// while the same recipe carrying `categories = {"crafting-with-fluid"}` loads
// with exit 0, and base's own sulfuric-acid reads `categories = {"chemistry"}`
// with no `category` at all. That is not a degradation, it is the WHOLE MOD
// failing to load, so it is the more severe of this round's two findings and
// the one that made a 2.1 golden row impossible until it was answered.
//
// THE SPELLING IS THE EMIT LAYER'S AND NOT THE PLAN'S. respellRecipeCategory
// rewrites the pair on the way out, so the Op stream a consumer's host test
// asserts on says `category` on every engine and does not move under them.
// The alternative was a question on World, which is an interface a consumer's
// own fixture implements: adding a method to it would break every one of them
// for a spelling this library can answer by itself.
func recipeCategoriesAreAList(baseVersion string) bool {
	return researchUnitTakesItems(baseVersion)
}

// respellRecipeCategory rewrites a recipe prototype's "category" pair into the
// "categories" pair a 2.1 engine wants, holding that one name.
//
// IT TOUCHES A RECIPE AND NOTHING ELSE. The type is read off the prototype
// itself rather than assumed from the caller, because the same Op stream
// carries items, technologies and four kinds of setting, and "category" is a
// field name a future prototype of another kind could carry with a meaning of
// its own.
//
// A PROTOTYPE WITH NO CATEGORY COMES BACK UNTOUCHED, by identity: the library
// omits the field entirely for a recipe that declared no category, because an
// absent field is the engine's own default and "crafting" spelled out would be
// this library inventing a value the author never wrote.
func respellRecipeCategory(proto Value) Value {
	if proto.Kind != KindMap || !protoTypeIs(proto, "recipe") {
		return proto
	}
	out := make([]KV, 0, len(proto.Map))
	changed := false
	for _, p := range proto.Map {
		if p.Key == "category" && p.Val.Kind == KindStr {
			out = append(out, kv("categories", Arr(p.Val)))
			changed = true
			continue
		}
		out = append(out, p)
	}
	if !changed {
		return proto
	}
	return Obj(out...)
}

// protoTypeIs answers whether a prototype's own "type" field is this name.
func protoTypeIs(proto Value, typ string) bool {
	for _, p := range proto.Map {
		if p.Key == "type" {
			return p.Val.Kind == KindStr && p.Val.Str == typ
		}
	}
	return false
}
