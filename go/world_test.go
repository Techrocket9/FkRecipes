package fkrecipes

import "testing"

// fixtureWorld is the host stand-in for the game: a slice of base Factorio
// big enough to exercise every branch, and small enough to read. Slices, not
// maps, for the same reason the library uses slices.
type fixtureWorld struct {
	modName  string
	settings []KV
	items    []string
	fluids   []string
	tools    []string
	recipes  []string
	techs    []fixtureTech

	entities         []string
	nilUnitFor       []string
	nilMaxLevelFor   []string
	mapUnitButAbsent []string

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

func (w *fixtureWorld) ModName() string { return w.modName }

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
	// A MAP ALONGSIDE ok=false. The World contract says the flag is the
	// answer to "is there a unit here", so a caller must honour it over
	// whatever value rides along; a World that returns both is violating the
	// contract, and this arm exists so the library's honouring of it has a
	// witness instead of an argument. Checked FIRST so it wins over the
	// technology's real unit: the point is a rung the ladder would otherwise
	// take.
	for _, n := range w.mapUnitButAbsent {
		if n == name {
			return Obj(
				kv("count", Num(1)),
				kv("time", Num(1)),
				kv("ingredients", Arr(Arr(Str("automation-science-pack"), Num(1)))),
			), false
		}
	}
	// PRESENT and nil: the exact shape fromV produces for a unit whose table
	// carried a numeric key, which is a different answer from "no unit".
	for _, n := range w.nilUnitFor {
		if n == name {
			return Nil(), true
		}
	}
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

// The fluids and the science packs are their OWN namespaces here, exactly as
// they are in data.raw: a name in items answers ItemExists and nothing else,
// which is what lets a fixture say "both" is an item and a fluid at once and
// hold the tie rule up to the light.
func (w *fixtureWorld) FluidExists(name string) bool {
	for _, f := range w.fluids {
		if f == name {
			return true
		}
	}
	return false
}

func (w *fixtureWorld) ToolExists(name string) bool {
	for _, t := range w.tools {
		if t == name {
			return true
		}
	}
	return false
}

func (w *fixtureWorld) EntityExists(name string) bool {
	for _, e := range w.entities {
		if e == name {
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

// withMapUnitButAbsent makes one technology answer a real map beside ok=false,
// which is the contract violation the ladder's !ok term is the guard against.
func (w *fixtureWorld) withMapUnitButAbsent(name string) *fixtureWorld {
	w.mapUnitButAbsent = append(w.mapUnitButAbsent, name)
	return w
}

func (w *fixtureWorld) withNilUnit(name string) *fixtureWorld {
	w.nilUnitFor = append(w.nilUnitFor, name)
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

func (w *fixtureWorld) withModName(name string) *fixtureWorld {
	w.modName = name
	return w
}

func (w *fixtureWorld) withTech(t fixtureTech) *fixtureWorld {
	w.techs = append(w.techs, t)
	return w
}

func (w *fixtureWorld) withEntity(name string) *fixtureWorld {
	w.entities = append(w.entities, name)
	return w
}

func (w *fixtureWorld) withItem(name string) *fixtureWorld {
	w.items = append(w.items, name)
	return w
}

func (w *fixtureWorld) withFluid(name string) *fixtureWorld {
	w.fluids = append(w.fluids, name)
	return w
}

func (w *fixtureWorld) withoutFluid(name string) *fixtureWorld {
	kept := make([]string, 0, len(w.fluids))
	for _, f := range w.fluids {
		if f != name {
			kept = append(kept, f)
		}
	}
	w.fluids = kept
	return w
}

func (w *fixtureWorld) withTool(name string) *fixtureWorld {
	w.tools = append(w.tools, name)
	w.items = append(w.items, name)
	return w
}

func (w *fixtureWorld) withoutTool(name string) *fixtureWorld {
	kept := make([]string, 0, len(w.tools))
	for _, t := range w.tools {
		if t != name {
			kept = append(kept, t)
		}
	}
	w.tools = kept
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
		// One entity, so a PlaceResult can resolve as well as fail. It is a
		// DERIVED type rather than a plain "entity": that is what the real
		// probe has to walk, and it is the type BBB's own item names.
		entities: []string{"steel-chest"},
		recipes: []string{
			"electronic-circuit",
			"iron-gear-wheel",
			"steel-plate",
		},
		// The two fluids a steelworks quenches with, and the science packs as
		// TOOLS as well as items: a pack is a tool-type item, so the game
		// answers both questions about it with yes.
		fluids: []string{"steam", "water"},
		tools: []string{
			"automation-science-pack",
			"chemical-science-pack",
			"logistic-science-pack",
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

// settingsWorld is what a settings-stage plan is handed. The settings stage
// runs before data.raw exists and the planner asks it for nothing but the mod
// name, so this is baseWorld under a name that says which plan is being held
// up to the light at the call site.
func settingsWorld() *fixtureWorld {
	return baseWorld()
}

// THE STAGE DISPATCH, held in the pure half so a test can reach it.
//
// The interesting half of this table is the bottom: the two stage names FkLua
// leaves unwired today. Emit's first shape was "settings plans settings,
// EVERYTHING ELSE plans data", which maps both of them to the data plan and
// would run PlanData at a settings stage where data.raw does not exist. This
// table is what makes the day they are wired a deliberate change rather than a
// silent misroute.
func TestStageKindOf(t *testing.T) {
	cases := []struct {
		name string
		want StageKind
		ok   bool
	}{
		{"settings", StageKindSettings, true},
		{"data", StageKindData, true},
		{"data-updates", StageKindData, true},
		{"data-final-fixes", StageKindData, true},
		// fkdata's own name for an id it does not recognise.
		{"unknown", StageNone, false},
		// Deliberately unwired upstream. Neither is a data stage, and neither
		// is one this library plans for until somebody decides what it means.
		{"settings-updates", StageNone, false},
		{"settings-final-fixes", StageNone, false},
		// Not a stage at all.
		{"", StageNone, false},
		{"Data", StageNone, false},
		{"data ", StageNone, false},
	}
	for _, c := range cases {
		got, ok := StageKindOf(c.name)
		if got != c.want || ok != c.ok {
			t.Errorf("StageKindOf(%q) = %v, %v; want %v, %v", c.name, got, ok, c.want, c.ok)
		}
	}
}

// Every stage fkdata can name is either planned for or refused BY NAME, with
// nothing falling through a default. This is the anti-vacuity half: the table
// above could be trimmed to two rows and still pass, and this could not.
func TestEveryFkdataStageNameIsDecided(t *testing.T) {
	// The names fkdata's StageID.Name returns, read from the guest module this
	// library requires. A name added there and not here is the gap this test
	// exists to find.
	for _, name := range []string{"settings", "data", "data-updates", "data-final-fixes", "unknown"} {
		got, ok := StageKindOf(name)
		if name == "unknown" {
			if ok {
				t.Errorf("StageKindOf(%q) plans for a stage fkdata could not identify", name)
			}
			continue
		}
		if !ok {
			t.Errorf("StageKindOf(%q) refuses a stage fkdata names and this library hooks", name)
		}
		if name == "settings" && got != StageKindSettings {
			t.Errorf("StageKindOf(%q) = %v, want the settings plan", name, got)
		}
		if name != "settings" && got != StageKindData {
			t.Errorf("StageKindOf(%q) = %v, want the data plan", name, got)
		}
	}
}
