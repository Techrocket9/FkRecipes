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

	// ToolExists answers whether a name is a SCIENCE PACK. The engine takes
	// tool-type items and nothing else in a research unit (measured: an item
	// ingredient refuses with "Research unit(s) can only be tool type items at
	// the moment"), so a pack list asks this rather than ItemExists, and an
	// item that is not a tool gets a sentence of its own.
	//
	// It is also the question a declared Pack's ladder walks, so a fixture
	// that names a science pack only in its items will see every hand-rolled
	// research cost lose its packs.
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
