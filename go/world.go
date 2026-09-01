package fkrecipes

// World is everything the planner is allowed to know about the game outside
// its own plan. The emit layer implements it over fkdata; host tests
// implement it over fixtures, which is the whole point of the interface.
//
// Every method is a QUESTION, never a write: a planner that could mutate
// could not be replayed, and the plan is what the two languages compare.
type World interface {
	// ModName is the packaged mod's name, the sole source of the prefix.
	ModName() string

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

	// RecipeExists is asked about the plan's OWN recipe names: a plan that
	// would overwrite an existing prototype is refused, which is also what
	// keeps every planned name new for the cycle overlay.
	RecipeExists(name string) bool
}

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
