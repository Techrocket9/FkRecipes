package fkrecipes

import "testing"

// fixtureWorld is the host stand-in for the game: a slice of base Factorio
// big enough to exercise every branch, and small enough to read. Slices, not
// maps, for the same reason the library uses slices.
type fixtureWorld struct {
	modName  string
	stage    string
	settings []KV
	items    []string
	recipes  []string
	techs    []fixtureTech

	nilMaxLevelFor []string

	// A max_level answered for ANY name, even one no technology carries.
	// Some Worlds are loose about lookups; the planner must not ask a
	// question it has no source technology for.
	looseMaxLevel Value
}

type fixtureTech struct {
	name     string
	prereqs  []string
	unit     Value // KindNil means the technology carries no unit
	maxLevel Value // KindNil means the technology has no level cap
	trigger  bool
}

func (w *fixtureWorld) ModName() string   { return w.modName }
func (w *fixtureWorld) StageName() string { return w.stage }

func (w *fixtureWorld) StartupSetting(name string) (Value, bool) {
	for _, s := range w.settings {
		if s.Key == name {
			return s.Val, true
		}
	}
	return Nil(), false
}

func (w *fixtureWorld) TechNames() []string {
	names := make([]string, 0, len(w.techs))
	for _, t := range w.techs {
		names = append(names, t.name)
	}
	return names
}

func (w *fixtureWorld) TechPrereqs(name string) []string {
	for _, t := range w.techs {
		if t.name == name {
			return t.prereqs
		}
	}
	return nil
}

func (w *fixtureWorld) TechUnit(name string) (Value, bool) {
	for _, t := range w.techs {
		if t.name == name && t.unit.Kind != KindNil {
			return t.unit, true
		}
	}
	return Nil(), false
}

func (w *fixtureWorld) TechMaxLevel(name string) (Value, bool) {
	// A read that is PRESENT and nil: what a LuaObject or a table this
	// library cannot carry collapses to on the way in.
	for _, n := range w.nilMaxLevelFor {
		if n == name {
			return Nil(), true
		}
	}
	for _, t := range w.techs {
		if t.name == name && t.maxLevel.Kind != KindNil {
			return t.maxLevel, true
		}
	}
	if w.looseMaxLevel.Kind != KindNil {
		return w.looseMaxLevel, true
	}
	return Nil(), false
}

func (w *fixtureWorld) TechHasResearchTrigger(name string) bool {
	for _, t := range w.techs {
		if t.name == name {
			return t.trigger
		}
	}
	return false
}

func (w *fixtureWorld) TechExists(name string) bool {
	for _, t := range w.techs {
		if t.name == name {
			return true
		}
	}
	return false
}

func (w *fixtureWorld) ItemExists(name string) bool {
	for _, it := range w.items {
		if it == name {
			return true
		}
	}
	return false
}

func (w *fixtureWorld) RecipeExists(name string) bool {
	for _, r := range w.recipes {
		if r == name {
			return true
		}
	}
	return false
}

// Mutators, so a test says what it changed about the world instead of
// restating the whole world.

func (w *fixtureWorld) withoutTech(name string) *fixtureWorld {
	kept := make([]fixtureTech, 0, len(w.techs))
	for _, t := range w.techs {
		if t.name != name {
			kept = append(kept, t)
		}
	}
	w.techs = kept
	return w
}

func (w *fixtureWorld) withoutItem(name string) *fixtureWorld {
	kept := make([]string, 0, len(w.items))
	for _, it := range w.items {
		if it != name {
			kept = append(kept, it)
		}
	}
	w.items = kept
	return w
}

func (w *fixtureWorld) withPrereqs(name string, prereqs ...string) *fixtureWorld {
	for i := range w.techs {
		if w.techs[i].name == name {
			w.techs[i].prereqs = prereqs
		}
	}
	return w
}

func (w *fixtureWorld) withSetting(name string, v Value) *fixtureWorld {
	w.settings = append(w.settings, KV{Key: name, Val: v})
	return w
}

func (w *fixtureWorld) withNilMaxLevel(name string) *fixtureWorld {
	w.nilMaxLevelFor = append(w.nilMaxLevelFor, name)
	return w
}

func (w *fixtureWorld) withUnit(name string, v Value) *fixtureWorld {
	for i := range w.techs {
		if w.techs[i].name == name {
			w.techs[i].unit = v
		}
	}
	return w
}

func (w *fixtureWorld) withStage(name string) *fixtureWorld {
	w.stage = name
	return w
}

func (w *fixtureWorld) withModName(name string) *fixtureWorld {
	w.modName = name
	return w
}

func (w *fixtureWorld) withTech(t fixtureTech) *fixtureWorld {
	w.techs = append(w.techs, t)
	return w
}

func (w *fixtureWorld) withItem(name string) *fixtureWorld {
	w.items = append(w.items, name)
	return w
}

func (w *fixtureWorld) withRecipe(name string) *fixtureWorld {
	w.recipes = append(w.recipes, name)
	return w
}

func (w *fixtureWorld) answeringMaxLevelForAnyName(v Value) *fixtureWorld {
	w.looseMaxLevel = v
	return w
}

func (w *fixtureWorld) withMaxLevel(name string, v Value) *fixtureWorld {
	for i := range w.techs {
		if w.techs[i].name == name {
			w.techs[i].maxLevel = v
		}
	}
	return w
}

func unitOf(count int, seconds float64, packs ...string) Value {
	ings := make([]Value, 0, len(packs))
	for _, p := range packs {
		ings = append(ings, Arr(Str(p), Num(1)))
	}
	// Sorted by key, because that is the order a unit comes back in: fkdata
	// sorts every dictionary at every level on the way out, so a plan reading
	// data.raw never sees authoring order. What this library EMITS is
	// pre-sort, which is why a hand-rolled unit's golden is count, time,
	// ingredients while a copied one's is alphabetical.
	return Obj(
		kv("count", Num(float64(count))),
		kv("ingredients", Arr(ings...)),
		kv("time", Num(seconds)),
	)
}

// baseWorld is a fresh copy per test: the mutators above rewrite it in place.
// Technology names are SORTED, which World.TechNames promises its caller.
func baseWorld() *fixtureWorld {
	return &fixtureWorld{
		modName: "steelworks",
		stage:   "data",
		recipes: []string{
			"electronic-circuit",
			"iron-gear-wheel",
			"steel-plate",
		},
		items: []string{
			"automation-science-pack",
			"chemical-science-pack",
			"copper-plate",
			"electronic-circuit",
			"iron-gear-wheel",
			"iron-plate",
			"logistic-science-pack",
			"steel-plate",
		},
		techs: []fixtureTech{
			{name: "automation", prereqs: []string{"electronics"}, unit: unitOf(250, 30, "automation-science-pack")},
			{name: "electronics", unit: unitOf(30, 15, "automation-science-pack")},
			{name: "logistics", unit: unitOf(20, 15, "automation-science-pack")},
			{name: "logistics-2", prereqs: []string{"logistics", "automation"}, unit: unitOf(200, 30, "automation-science-pack", "logistic-science-pack")},
			{name: "logistics-3", prereqs: []string{"logistics-2"}, unit: unitOf(400, 60, "automation-science-pack", "logistic-science-pack", "chemical-science-pack")},
			// An infinite technology, the CostOf-verbatim case. count_formula
			// is a string, so copying it needs no evaluator, and the level cap
			// sits on the technology beside the unit, not inside it.
			{name: "mining-productivity-4", prereqs: []string{"logistics-3"}, maxLevel: Str("infinite"), unit: Obj(
				kv("count_formula", Str("2^(L-4)*1000")),
				kv("ingredients", Arr(
					Arr(Str("automation-science-pack"), Num(1)),
					Arr(Str("logistic-science-pack"), Num(1)),
					Arr(Str("chemical-science-pack"), Num(1)),
				)),
				// A key this library has never heard of, left on the unit by
				// whichever mod owns the source technology. The copy carries
				// it through untouched, because the copy is a copy and not a
				// rebuild from the fields the planner knows.
				kv("mod_cost_tier", Str("mid-game")),
				kv("time", Num(60)),
			)},
			// A research_trigger technology: no unit at all, which is the
			// measured crash class CostOf has to refuse.
			{name: "steam-power", trigger: true},
			{name: "steel-processing", unit: unitOf(50, 15, "automation-science-pack")},
		},
	}
}

func TestFixtureTechNamesAreSorted(t *testing.T) {
	names := baseWorld().TechNames()
	for i := 1; i < len(names); i++ {
		if names[i-1] >= names[i] {
			t.Fatalf("fixture technology names are not sorted: %q then %q", names[i-1], names[i])
		}
	}
}

// settingsWorld is baseWorld at the OTHER stage. The settings stage runs
// before data.raw exists, and the planner asks it for nothing but the mod
// name and the stage name.
func settingsWorld() *fixtureWorld {
	return baseWorld().withStage("settings")
}
