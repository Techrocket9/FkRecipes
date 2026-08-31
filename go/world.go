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

	// StageName names the stage the plan is running in ("data",
	// "data-updates", ...). It appears in refusals so the reader knows which
	// of their own calls raised.
	StageName() string

	// StartupSetting reads a startup setting by its FULL, prefixed name. The
	// second result is false when no such setting is readable, which the
	// planner degrades to the declared default plus a log line.
	StartupSetting(name string) (Value, bool)

	// TechNames lists every technology in the game, SORTED. The caller
	// guarantees the sort; the cycle walk's determinism rests on it.
	TechNames() []string

	// TechPrereqs is one technology's prerequisite list, in its own order.
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
