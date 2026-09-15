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

	entities          []string
	nilUnitFor        []string
	nilMaxLevelFor    []string
	mapUnitButAbsent  []string
	settingsButAbsent []KV

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
	// A VALUE ALONGSIDE ok=false, the settings twin of the arm TechUnit has:
	// the flag is the answer to "is this setting there", so a caller must
	// honour it over whatever value rides along. Checked FIRST so it wins over
	// a value the same fixture also answers properly.
	for _, s := range w.settingsButAbsent {
		if s.Key == name {
			return s.Val, false
		}
	}
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

// withSettingButAbsent makes one setting answer a real value beside ok=false,
// which is the contract violation the !ok terms of resolveTextList and
// readNumber are the guard against: an edit-shaped value that the World says is
// not there. Go's World answers (Value, bool) and Rust's answers Option<Value>,
// so this pair exists on one side only and is tested on one side only. Read by
// TestASettingAnsweringAValueBesideNotOkIsAbsent.
func (w *fixtureWorld) withSettingButAbsent(name string, v Value) *fixtureWorld {
	w.settingsButAbsent = append(w.settingsButAbsent, KV{Key: name, Val: v})
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

// THE ENGINE KEY, which decides which probe answers the science-pack question
// in the emit layer. It lives here rather than in guest.go for the reason
// StageKindOf does: guest.go is behind //go:build tinygo.wasm, so a decision
// written there is a decision no host test can reach, and this one changes
// what every priced research in the game is made of.
//
// THE TWO MEASURED ROWS ARE THE FIRST TWO. 2.0.77 is the engine the tool rule
// was probed on, 2.1.17 is the engine that has no data.raw.tool at all. The
// rest are the shapes a host can hand back.
func TestResearchUnitTakesItemsKeysOnBasesOwnVersion(t *testing.T) {
	for _, row := range []struct {
		version string
		want    bool
		why     string
	}{
		{"2.0.77", false, "the engine the tool-type rule was measured on"},
		{"2.1.17", true, "the engine with no data.raw.tool at all"},
		{"2.0", false, "a version with no patch part"},
		{"2.1", true, "the same, one minor up"},
		{"1.1.110", false, "the series before the one this library was written for"},
		{"2.2.0", true, "a minor this library has never seen"},
		{"3.0.0", true, "a major this library has never seen"},
		{"2", false, "a major alone reads as minor 0"},
		{"", true, "unreadable: the current engine"},
		{"experimental", true, "unreadable: the current engine"},
	} {
		if got := researchUnitTakesItems(row.version); got != row.want {
			t.Errorf("researchUnitTakesItems(%q) = %v, want %v (%s)",
				row.version, got, row.want, row.why)
		}
	}
}

// The parser under it, including the two refusals: no leading digit at all,
// and a run of digits long enough that reading it as an int would be a guess.
func TestMajorMinorReadsTheLeadingTwoNumbers(t *testing.T) {
	for _, row := range []struct {
		version      string
		major, minor int
		ok           bool
	}{
		{"2.1.17", 2, 1, true},
		{"2.0.77", 2, 0, true},
		{"2.1", 2, 1, true},
		{"2", 2, 0, true},
		{"0.18.47", 0, 18, true},
		{"2.", 2, 0, true},
		{"2.x", 2, 0, true},
		{"", 0, 0, false},
		{".1", 0, 0, false},
		{"v2.1", 0, 0, false},
		{"99999.1", 0, 0, false},
		{"2.99999", 2, 0, true},
	} {
		major, minor, ok := majorMinor(row.version)
		if major != row.major || minor != row.minor || ok != row.ok {
			t.Errorf("majorMinor(%q) = %d, %d, %v; want %d, %d, %v",
				row.version, major, minor, ok, row.major, row.minor, row.ok)
		}
	}
}

// The subgroup constant is a NAME the emit layer compares a leaf against, and
// a typo in it would answer no for every science pack on a 2.1 engine with
// nothing else in the library changing. Measured on 2.1.17 build 87315.
func TestPackSubgroupIsTheMeasuredName(t *testing.T) {
	if packSubgroup != "science-pack" {
		t.Errorf("packSubgroup = %q, want %q: the subgroup base's seven packs "+
			"carry on 2.1.17", packSubgroup, "science-pack")
	}
}

// THE SECOND ENGINE-KEYED FACT: the spelling of a recipe's category. A 2.1
// engine refuses `category` outright, so this one is the whole mod failing to
// load rather than a degradation, and it is answered on the way out so the Op
// stream a consumer asserts on does not move under them.
func TestRespellRecipeCategoryRewritesOnlyARecipesOwnField(t *testing.T) {
	recipe := Obj(
		kv("type", Str("recipe")),
		kv("name", Str("m-quenching")),
		kv("category", Str("crafting-with-fluid")),
		kv("energy_required", Num(7.5)),
	)
	got := respellRecipeCategory(recipe)
	want := Obj(
		kv("type", Str("recipe")),
		kv("name", Str("m-quenching")),
		kv("categories", Arr(Str("crafting-with-fluid"))),
		kv("energy_required", Num(7.5)),
	)
	if !valuesEqualForCategory(got, want) {
		t.Errorf("respellRecipeCategory rewrote the recipe as %v, want %v", got, want)
	}

	// A RECIPE WITH NO CATEGORY IS UNTOUCHED: the library omits the field for
	// a recipe that declared none, and an invented "crafting" would be a value
	// the author never wrote.
	plain := Obj(kv("type", Str("recipe")), kv("name", Str("m-rivet")))
	if !valuesEqualForCategory(respellRecipeCategory(plain), plain) {
		t.Error("respellRecipeCategory changed a recipe that declared no category")
	}

	// AND IT TOUCHES NOTHING ELSE. "category" on another prototype kind is
	// that kind's own field with its own meaning; the type is read off the
	// prototype rather than assumed.
	other := Obj(kv("type", Str("item")), kv("category", Str("science-pack")))
	if !valuesEqualForCategory(respellRecipeCategory(other), other) {
		t.Error("respellRecipeCategory rewrote a field on a prototype that is not a recipe")
	}
	if !valuesEqualForCategory(respellRecipeCategory(Str("not a prototype")), Str("not a prototype")) {
		t.Error("respellRecipeCategory rewrote something that is not a map")
	}
}

// The two keys agree, and they agree BY CONSTRUCTION rather than by two tables
// drifting apart: both engine facts were measured on the same two binaries.
func TestBothEngineFactsKeyTheSameWay(t *testing.T) {
	for _, version := range []string{"2.0.77", "2.1.17", "2.0", "2.1", "", "1.1.110"} {
		if recipeCategoriesAreAList(version) != researchUnitTakesItems(version) {
			t.Errorf("the two engine keys disagree at %q", version)
		}
	}
}

// valuesEqualForCategory is a structural compare deep enough for the rows
// above: maps in order, arrays in order, scalars by kind and value.
func valuesEqualForCategory(a, b Value) bool {
	if a.Kind != b.Kind {
		return false
	}
	switch a.Kind {
	case KindStr:
		return a.Str == b.Str
	case KindNum:
		return a.Num == b.Num
	case KindArr:
		if len(a.Arr) != len(b.Arr) {
			return false
		}
		for i := range a.Arr {
			if !valuesEqualForCategory(a.Arr[i], b.Arr[i]) {
				return false
			}
		}
		return true
	case KindMap:
		if len(a.Map) != len(b.Map) {
			return false
		}
		for i := range a.Map {
			if a.Map[i].Key != b.Map[i].Key ||
				!valuesEqualForCategory(a.Map[i].Val, b.Map[i].Val) {
				return false
			}
		}
		return true
	}
	return true
}
