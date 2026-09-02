package fkrecipes

import (
	"math"
	"strings"
	"testing"
)

// A believable little mod: an axe head, a steel axe made from it, and the
// research that unlocks the recipe.

// steel-processing's unit as the WORLD hands it back: sorted by key, because
// fkdata sorts every dictionary at every level on the way out. A unit this
// library builds itself is pre-sort and keeps its authored order.
const steelProcessingUnit = `{count=50, ingredients=[["automation-science-pack", 1]], time=15}`

// The same, for the two technologies the split-emission test prices from.
const logistics2Unit = `{count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}`
const logistics3Unit = `{count=400, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], time=60}`

func logistics2UnitValue() Value {
	return unitOf(200, 30, "automation-science-pack", "logistic-science-pack")
}

func TestPlanDataItemAndRecipeShapes(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{
		Icon:        "__steelworks__/graphics/icons/steel-axe.png",
		IconSize:    64,
		StackSize:   20,
		Subgroup:    "tool",
		DisplayName: "Steel axe",
		Description: "Chops trees at twice the speed.",
	})
	head := lib.Item("axe-head", ItemSpec{Icon: "__steelworks__/graphics/icons/axe-head.png"})
	lib.Recipe(head, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(2, "steel-plate", "iron-plate")},
	})
	lib.Recipe(axe, RecipeSpec{
		CraftTime:   2.5,
		Category:    "crafting",
		ResultCount: 2,
		Ingredients: []Ingredient{
			IngredientOf(head, 1),
			IngredientNamed(4, "steel-plate", "iron-plate"),
		},
		DisplayName: "Steel axe",
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", localised_name=["", "Steel axe"], localised_description=["", "Chops trees at twice the speed."], icon="__steelworks__/graphics/icons/steel-axe.png", icon_size=64, stack_size=20, subgroup="tool"}`,
		`extend {type="item", name="steelworks-axe-head", icon="__steelworks__/graphics/icons/axe-head.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-axe-head", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-axe-head", amount=1}]}`,
		`extend {type="recipe", name="steelworks-steel-axe", localised_name=["", "Steel axe"], category="crafting", energy_required=2.5000000000000000e0, enabled=true, ingredients=[{type="item", name="steelworks-axe-head", amount=1}, {type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=2}]}`,
	})
}

// The ladder resolves to the first candidate the game actually has, and drops
// the ingredient when it has none of them. Never a guess: a name the game
// does not have is a hard load failure naming the consumer's mod.
func TestIngredientLadderFallsBackAndDrops(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{Icon: "__steelworks__/graphics/icons/steel-axe.png"})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{
			IngredientNamed(4, "steel-plate", "iron-plate"),
			IngredientNamed(1, "tungsten-plate", "titanium-plate"),
		},
	})

	// The world has iron but no steel, so the ladder takes its second rung.
	ops, err := lib.PlanData(baseWorld().withoutItem("steel-plate"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axe: none of tungsten-plate, titanium-plate is present, so the ingredient is dropped`,
		`extend {type="item", name="steelworks-steel-axe", icon="__steelworks__/graphics/icons/steel-axe.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="iron-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// CostOf copies the named technology's whole unit VERBATIM: a count_formula
// is a string, so an infinite technology's cost comes along with no evaluator
// and no key of it rewritten or reordered.
func TestTechnologyCostOfCopiesUnitVerbatim(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{Icon: "__steelworks__/graphics/icons/steel-axe.png"})
	rec := lib.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")}})
	lib.Technology("steel-axes", TechSpec{
		Icon:        "__steelworks__/graphics/technology/steel-axes.png",
		IconSize:    128,
		CostOf:      "mining-productivity-4",
		After:       "steel-processing",
		Unlocks:     []RecipeRef{rec},
		DisplayName: "Steel axes",
		Description: "Sharper edges, fewer swings.",
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", icon="__steelworks__/graphics/icons/steel-axe.png", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=false, ingredients=[{type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="technology", name="steelworks-steel-axes", localised_name=["", "Steel axes"], localised_description=["", "Sharper edges, fewer swings."], icon="__steelworks__/graphics/technology/steel-axes.png", icon_size=128, prerequisites=["steel-processing"], unit={count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}, max_level="infinite", effects=[{type="unlock-recipe", recipe="steelworks-steel-axe"}]}`,
	})
}

// The level cap is a TECHNOLOGY field, not a unit field, so the verbatim unit
// copy cannot carry it: CostOf reads it separately, or an infinite source
// would produce a one-level copy.
func TestTechnologyCostOfCarriesMaxLevel(t *testing.T) {
	const miningUnit = `{count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}`

	cases := []struct {
		name   string
		costOf string
		world  func(*fixtureWorld) *fixtureWorld
		want   string
	}{
		{
			name:   "an infinite source keeps its infinity",
			costOf: "mining-productivity-4",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want:   `extend {type="technology", name="steelworks-steel-axes", unit=` + miningUnit + `, max_level="infinite"}`,
		},
		{
			// An overhaul that caps the endless research at a finite level.
			name:   "a numeric cap comes across as a number",
			costOf: "mining-productivity-4",
			world:  func(w *fixtureWorld) *fixtureWorld { return w.withMaxLevel("mining-productivity-4", Num(20)) },
			want:   `extend {type="technology", name="steelworks-steel-axes", unit=` + miningUnit + `, max_level=20}`,
		},
		{
			name:   "a single-level source gets no cap at all",
			costOf: "steel-processing",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want:   `extend {type="technology", name="steelworks-steel-axes", unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			lib.Technology("steel-axes", TechSpec{CostOf: c.costOf})

			ops, err := lib.PlanData(c.world(baseWorld()))
			assertNoError(t, err)

			assertLines(t, transcript(ops), []string{c.want})
		})
	}
}

// A hand-rolled unit has no source technology to read a cap from, so the
// escape hatch never emits max_level.
func TestTechnologyUnitSpecCarriesNoMaxLevel(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{
		Unit: &UnitSpec{Count: 75, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
	})

	// This world answers a cap for any name at all, so the only thing keeping
	// max_level off the prototype is the planner not asking.
	ops, err := lib.PlanData(baseWorld().answeringMaxLevelForAnyName(Num(20)))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// The two ingredient shapes are not interchangeable: a recipe takes the long
// dict form, a technology unit takes the short tuple form, and the engine
// refuses each in the other's place.
func TestTechnologyUnitSpecUsesShortTupleForm(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{
		Unit: &UnitSpec{
			Count:   75,
			Seconds: 30,
			Packs: []Pack{
				{Name: "automation-science-pack", Amount: 1},
				{Name: "logistic-science-pack", Amount: 2},
			},
		},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit={count=75, time=30, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 2]]}}`,
	})
}

func TestTechnologyEnabledBy(t *testing.T) {
	cases := []struct {
		name    string
		def     bool
		world   func(*fixtureWorld) *fixtureWorld
		tail    string
		logLine string
	}{
		{
			name:  "the setting is on",
			def:   false,
			world: func(w *fixtureWorld) *fixtureWorld { return w.withSetting("steelworks-hardened-tools", Bool(true)) },
			tail:  `, enabled=true}`,
		},
		{
			name:  "the setting is off",
			def:   true,
			world: func(w *fixtureWorld) *fixtureWorld { return w.withSetting("steelworks-hardened-tools", Bool(false)) },
			tail:  `, enabled=false, hidden=true}`,
		},
		{
			name:    "the setting is unreadable and defaults on",
			def:     true,
			world:   func(w *fixtureWorld) *fixtureWorld { return w },
			tail:    `, enabled=true}`,
			logLine: "log fkrecipes: the setting steelworks-hardened-tools was not readable, so its default applies",
		},
		{
			name:    "the setting is unreadable and defaults off",
			def:     false,
			world:   func(w *fixtureWorld) *fixtureWorld { return w },
			tail:    `, enabled=false, hidden=true}`,
			logLine: "log fkrecipes: the setting steelworks-hardened-tools was not readable, so its default applies",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			on := lib.BoolSetting("hardened-tools", c.def)
			lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: on})

			ops, err := lib.PlanData(c.world(baseWorld()))
			assertNoError(t, err)

			want := []string{}
			if c.logLine != "" {
				want = append(want, c.logLine)
			}
			want = append(want, `extend {type="technology", name="steelworks-steel-axes", unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}`+c.tail)
			assertLines(t, transcript(ops), want)
		})
	}
}

func TestTreePlacement(t *testing.T) {
	cases := []struct {
		name  string
		after string
		// before is empty for the plain After case.
		before string
		world  func(*fixtureWorld) *fixtureWorld
		want   []string
	}{
		{
			name:  "after alone",
			after: "logistics-2",
			world: func(w *fixtureWorld) *fixtureWorld { return w },
			want:  []string{`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}`},
		},
		{
			name:  "after alone, absent",
			after: "quarry-drills",
			world: func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`log fkrecipes: steel-axes: quarry-drills is absent, so the prerequisite is dropped`,
				`extend {type="technology", name="steelworks-steel-axes", unit=UNIT}`,
			},
		},
		{
			name:   "insert between, the splice replaces the edge",
			after:  "logistics-2",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}`,
				`set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]`,
			},
		},
		{
			name:   "insert between, the far end does not require the near one",
			after:  "electronics",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`log fkrecipes: steel-axes: logistics-3 does not require electronics, so the new technology is appended to its prerequisites`,
				`extend {type="technology", name="steelworks-steel-axes", prerequisites=["electronics"], unit=UNIT}`,
				`set technology.logistics-3.prerequisites = ["logistics-2", "steelworks-steel-axes"]`,
			},
		},
		{
			// Another mod removed the technology this plan meant to splice
			// in front of.
			name:   "insert between, the far end is absent",
			after:  "logistics-2",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w.withoutTech("logistics-3") },
			want: []string{
				`log fkrecipes: steel-axes: logistics-3 is absent, so InsertBetween degrades to After(logistics-2)`,
				`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit=UNIT}`,
			},
		},
		{
			name:   "insert between, the near end is absent",
			after:  "quarry-drills",
			before: "logistics-3",
			world:  func(w *fixtureWorld) *fixtureWorld { return w },
			want: []string{
				`log fkrecipes: steel-axes: quarry-drills is absent, so the prerequisite is dropped`,
				`extend {type="technology", name="steelworks-steel-axes", unit=UNIT}`,
			},
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: c.after, Before: c.before})

			ops, err := lib.PlanData(c.world(baseWorld()))
			assertNoError(t, err)

			want := make([]string, 0, len(c.want))
			for _, line := range c.want {
				want = append(want, strings.Replace(line, "UNIT", steelProcessingUnit, 1))
			}
			assertLines(t, transcript(ops), want)
		})
	}
}

// Two splices into the same technology chain: the second reads what the first
// planned, or the second Set op would silently undo the first.
func TestTwoSplicesIntoOneTechnologyChain(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", Before: "logistics-3"})
	lib.Technology("bronze-axes", TechSpec{CostOf: "steel-processing", After: "electronics", Before: "logistics-3"})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: bronze-axes: logistics-3 does not require electronics, so the new technology is appended to its prerequisites`,
		`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`extend {type="technology", name="steelworks-bronze-axes", prerequisites=["electronics"], unit={count=50, ingredients=[["automation-science-pack", 1]], time=15}}`,
		`set technology.logistics-3.prerequisites = ["steelworks-steel-axes"]`,
		`set technology.logistics-3.prerequisites = ["steelworks-steel-axes", "steelworks-bronze-axes"]`,
	})
}

// Nothing this library emits can carry a name it did not prefix. The check is
// a property over the whole stream, not one assertion per prototype.
func TestEveryEmittedNameIsPrefixed(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{Icon: "__steelworks__/graphics/icons/steel-axe.png"})
	rec := lib.Recipe(axe, RecipeSpec{Name: "steel-axe-forging", Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")}})
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", Before: "logistics-3", Unlocks: []RecipeRef{rec}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	extends := 0
	for _, op := range ops {
		if op.Kind != OpExtend {
			continue
		}
		extends++
		name, ok := field(op.Proto, "name")
		if !ok {
			t.Fatalf("a prototype carries no name: %s", renderValue(op.Proto))
		}
		if !strings.HasPrefix(name.Str, "steelworks-") {
			t.Errorf("prototype name %q is not prefixed", name.Str)
		}
	}
	if extends != 3 {
		t.Fatalf("expected three prototypes, got %d", extends)
	}

	// The splice writes into somebody else's technology, so the PATH keeps
	// their unprefixed name and only the value carries ours.
	last := ops[len(ops)-1]
	if last.Kind != OpSet {
		t.Fatalf("the last op is not the splice: %v", last.Kind)
	}
	if got := renderPath(last.Path); got != "technology.logistics-3.prerequisites" {
		t.Errorf("splice path is %q", got)
	}
	if got := renderValue(last.Val); got != `["steelworks-steel-axes"]` {
		t.Errorf("splice value is %q", got)
	}
}

func TestPlanDataRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		world func(*fixtureWorld) *fixtureWorld
		want  string
	}{
		{
			name: "two items share a name",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{})
				l.Item("steel-axe", ItemSpec{})
			},
			want: "fkrecipes: two items share the name steelworks-steel-axe; the second would overwrite the first",
		},
		{
			name: "two recipes share a name",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				head := l.Item("axe-head", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Name: "steel-axe-forging"})
				l.Recipe(head, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: two recipes share the name steelworks-steel-axe-forging; the second would overwrite the first",
		},
		{
			name: "two technologies share a name",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "logistics-2"})
			},
			want: "fkrecipes: two technologies share the name steelworks-steel-axes; the second would overwrite the first",
		},
		{
			name: "a recipe with no result item",
			build: func(l *Lib) {
				l.Recipe(ItemRef{}, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
		},
		{
			name: "an ingredient item from no plan",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientOf(ItemRef{}, 1)}})
			},
			want: "fkrecipes: the recipe steel-axe names an ingredient item that this plan never declared",
		},
		{
			name: "neither CostOf nor Unit",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{After: "steel-processing"})
			},
			want: "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit or CostBy",
		},
		{
			name: "both CostOf and Unit",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{
					CostOf: "steel-processing",
					Unit:   &UnitSpec{Count: 50, Seconds: 15, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
				})
			},
			want: "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit or CostBy",
		},
		{
			name: "Before without After",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", Before: "logistics-3"})
			},
			want: "fkrecipes: the technology steel-axes names Before without After; InsertBetween needs both ends",
		},
		{
			name: "a unit count below one",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{Count: 0, Seconds: 15, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}}})
			},
			want: "fkrecipes: the technology steel-axes has a unit count below 1, which the engine refuses",
		},
		{
			name: "a science pack the game does not have",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{Count: 50, Seconds: 15, Packs: []Pack{{Name: "military-science-pack", Amount: 1}}}})
			},
			want: "fkrecipes: the technology steel-axes prices itself in military-science-pack, which does not exist",
		},
		{
			name: "CostOf names a technology that is not there",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "logistics-4"})
			},
			want: "fkrecipes: CostOf(logistics-4): no technology of that name exists",
		},
		{
			name: "CostOf names a research_trigger technology",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steam-power"})
			},
			want: "fkrecipes: CostOf(steam-power): steam-power is a research_trigger technology with no unit to copy; name a unit-carrying technology instead",
		},
		{
			name: "unlocking a recipe from no plan",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", Unlocks: []RecipeRef{{}}})
			},
			want: "fkrecipes: the technology steel-axes unlocks a recipe that this plan never declared",
		},
		{
			// Out of range for THIS plan, which is the shape that used to
			// index straight into a shorter slice and panic.
			name: "an EnabledBy setting out of range",
			build: func(l *Lib) {
				other := New()
				other.BoolSetting("hardened-tools", true)
				other.BoolSetting("brittle-heads", false)
				stray := other.BoolSetting("sharpened-edges", true)
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: stray})
			},
			want: "fkrecipes: the technology steel-axes names an EnabledBy setting that this plan never declared",
		},
		{
			name: "an item this plan would overwrite",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{})
			},
			world: func(w *fixtureWorld) *fixtureWorld { return w.withItem("steelworks-steel-axe") },
			want:  "fkrecipes: the item steelworks-steel-axe already exists in data.raw; this plan would overwrite it",
		},
		{
			name: "a recipe this plan would overwrite",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Name: "steel-axe-forging"})
			},
			world: func(w *fixtureWorld) *fixtureWorld { return w.withRecipe("steelworks-steel-axe-forging") },
			want:  "fkrecipes: the recipe steelworks-steel-axe-forging already exists in data.raw; this plan would overwrite it",
		},
		{
			name: "a technology this plan would overwrite",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withTech(fixtureTech{name: "steelworks-steel-axes", unit: unitOf(50, 15, "automation-science-pack")})
			},
			want: "fkrecipes: the technology steelworks-steel-axes already exists in data.raw; this plan would overwrite it",
		},
		{
			name: "both anchors at once",
			build: func(l *Lib) {
				first := l.Technology("bronze-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", AfterTech: first})
			},
			want: "fkrecipes: the technology steel-axes names both After and AfterTech; pick one anchor",
		},
		{
			name: "a splice around a technology this plan declares",
			build: func(l *Lib) {
				first := l.Technology("bronze-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", AfterTech: first, Before: "logistics-3"})
			},
			want: "fkrecipes: the technology steel-axes names Before with AfterTech; InsertBetween splices around a technology that already exists",
		},
		{
			name: "a crafting time that is not a number",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: math.Inf(1)})
			},
			want: "fkrecipes: the recipe steel-axe declares a crafting time that is not a finite number",
		},
		{
			name: "a research time that is not a number",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: math.NaN(),
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes declares a research time that is not a finite number",
		},
		{
			name: "an item with an empty name",
			build: func(l *Lib) {
				l.Item("", ItemSpec{})
			},
			want: "fkrecipes: an item was declared with an empty name",
		},
		{
			name: "a technology with an empty name",
			build: func(l *Lib) {
				l.Technology("", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: a technology was declared with an empty name",
		},
		{
			// A holed or mixed Lua table crosses as a number-keyed map, which
			// converts to nil as a whole subtree rather than being half kept.
			// A unit that lost its ingredient list is a technology researchable
			// for free, so the copy is refused rather than emitted.
			name: "a CostOf source whose unit lost a subtree on the way in",
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withUnit("steel-processing", Obj(
					kv("count", Num(50)),
					kv("ingredients", Nil()),
					kv("time", Num(15)),
				))
			},
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): the unit of steel-processing holds a table this library cannot copy faithfully",
		},
		{
			name: "a science pack with an empty name",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: 15,
					Packs:   []Pack{{Name: "", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes prices itself in a pack with an empty name",
		},
		{
			name: "both a fixed and a bound crafting time",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: 2.5, CraftTimeFrom: from})
			},
			want: "fkrecipes: the recipe steel-axe names both CraftTime and CraftTimeFrom; pick one",
		},
		{
			name: "a crafting-time setting from another plan",
			build: func(l *Lib) {
				other := New()
				stray := other.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: stray})
			},
			want: "fkrecipes: the recipe steel-axe names a crafting-time setting that this plan never declared",
		},
		{
			// Measured: the engine refuses energy_required <= 0.001.
			name: "a declared crafting time below the engine floor",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: 0.001})
			},
			want: "fkrecipes: the recipe steel-axe declares a crafting time the engine refuses (energy_required can't be <= 0.001)",
		},
		{
			// The generated setting's own minimum clears the floor, so this
			// is what a colliding mod's setting looks like: same name, same
			// type, last declaration wins, silently.
			name: "a bound crafting time answered below the engine floor",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withSetting("steelworks-axe-craft-time", Num(0.001))
			},
			want: "fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, which answers at or below the engine floor (energy_required can't be <= 0.001)",
		},
		{
			// An infinity is ABOVE the floor, so the floor arm would wave it
			// through and ship a recipe that never completes.
			name: "a bound crafting time answered as an infinity",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withSetting("steelworks-axe-craft-time", Num(math.Inf(1)))
			},
			want: "fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, which answers a value that is not a finite number",
		},
		{
			// A NaN compares false against the floor, so it reached the floor
			// arm and was reported as a value at or below it, which it is not.
			name: "a bound crafting time answered as a NaN",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withSetting("steelworks-axe-craft-time", Num(math.NaN()))
			},
			want: "fkrecipes: the recipe steel-axe reads its crafting time from steelworks-axe-craft-time, which answers a value that is not a finite number",
		},
		{
			// PRESENT and nil, which is what a unit whose table carried a
			// numeric key collapses to: a different answer from "no unit".
			name:  "a CostOf source whose unit arrives as nil",
			world: func(w *fixtureWorld) *fixtureWorld { return w.withNilUnit("steel-processing") },
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): steel-processing has a unit that is not a dictionary",
		},
		{
			name: "a CostOf source whose max_level lost a subtree on the way in",
			world: func(w *fixtureWorld) *fixtureWorld {
				return w.withMaxLevel("steel-processing", Obj(kv("levels", Nil())))
			},
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): the max_level of steel-processing holds a table this library cannot copy faithfully",
		},
		{
			// The World says the technology is there and is not a research
			// trigger, but hands back no unit: the arm the emit layer's
			// TechUnit read lands on.
			name:  "a CostOf source that carries no unit at all",
			world: func(w *fixtureWorld) *fixtureWorld { return w.withUnit("steel-processing", Nil()) },
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			want: "fkrecipes: CostOf(steel-processing): steel-processing carries no unit to copy",
		},
		{
			name: "a negative stack size",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{StackSize: -20})
			},
			want: "fkrecipes: the item steel-axe has a negative stack size, which the engine refuses",
		},
		{
			name: "a negative icon size on an item",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{IconSize: -64})
			},
			want: "fkrecipes: the item steel-axe has a negative icon size, which the engine refuses",
		},
		{
			name: "a negative icon size on a technology",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", IconSize: -128})
			},
			want: "fkrecipes: the technology steel-axes has a negative icon size, which the engine refuses",
		},
		{
			name: "an ingredient amount below one",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(0, "steel-plate")}})
			},
			want: "fkrecipes: the recipe steel-axe has an ingredient amount below 1, which the engine refuses",
		},
		{
			name: "a negative result count",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{ResultCount: -2})
			},
			want: "fkrecipes: the recipe steel-axe has a negative result count, which the engine refuses",
		},
		{
			name: "a negative crafting time",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{CraftTime: -2.5})
			},
			want: "fkrecipes: the recipe steel-axe has a negative crafting time, which the engine refuses",
		},
		{
			name: "a science pack amount below one",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: 15,
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 0}},
				}})
			},
			want: "fkrecipes: the technology steel-axes has a science pack amount below 1, which the engine refuses",
		},
		{
			name: "a research time of zero",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   50,
					Seconds: 0,
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes has a research time at or below zero, which the engine refuses",
		},
		{
			name: "a stack size past what a double holds",
			build: func(l *Lib) {
				l.Item("steel-axe", ItemSpec{StackSize: 9007199254740993})
			},
			want: "fkrecipes: the item steel-axe declares a stack size a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			name: "an ingredient amount past what a double holds",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(9007199254740993, "steel-plate")}})
			},
			want: "fkrecipes: the recipe steel-axe declares an ingredient amount a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			name: "a unit count past what a double holds",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
					Count:   9007199254740993,
					Seconds: 15,
					Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
				}})
			},
			want: "fkrecipes: the technology steel-axes declares a unit count a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			name: "a CostOf source whose unit is not a dictionary",
			build: func(l *Lib) {
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})
			},
			world: func(w *fixtureWorld) *fixtureWorld {
				// A Lua sequence IS a table, which is why the refusal says
				// dictionary: this shape has to be refused too.
				return w.withUnit("steel-processing", Arr(Num(50), Num(15)))
			},
			want: "fkrecipes: CostOf(steel-processing): steel-processing has a unit that is not a dictionary",
		},
		{
			// The result handle is checked before the duplicate-name scan, so
			// a recipe with no result is told what is actually wrong instead
			// of being reported as a name collision with the recipe it
			// accidentally shares a name with.
			name: "a recipe with no result item that also collides by name",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Name: "steel-axe-forging"})
				l.Recipe(ItemRef{}, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			w := baseWorld()
			if c.world != nil {
				w = c.world(w)
			}
			ops, err := lib.PlanData(w)
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}
}

// A handle is a fact about ONE plan. Another plan's handle is in range here
// and points at something else entirely, so it is refused rather than
// silently resolved into this plan's prototypes.
func TestHandlesFromAnotherPlanAreRefused(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			// The review's first shape: an in-range ItemRef that names a
			// different item in the other plan.
			name: "a result item handle from another plan",
			build: func(l *Lib) {
				other := New()
				bronze := other.Item("bronze-axe", ItemSpec{})
				l.Item("steel-axe", ItemSpec{})
				l.Recipe(bronze, RecipeSpec{Name: "steel-axe-forging"})
			},
			want: "fkrecipes: a recipe was declared with no result item; Recipe needs an item this plan declared",
		},
		{
			name: "an ingredient handle from another plan",
			build: func(l *Lib) {
				other := New()
				bronze := other.Item("bronze-axe", ItemSpec{})
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientOf(bronze, 1)}})
			},
			want: "fkrecipes: the recipe steel-axe names an ingredient item that this plan never declared",
		},
		{
			// The review's second shape: a BoolSettingRef that lands on this
			// plan's int setting, which would have read a bool out of it.
			name: "an EnabledBy handle from another plan lands on an int setting",
			build: func(l *Lib) {
				other := New()
				flag := other.BoolSetting("hardened-tools", true)
				l.IntSetting("axe-durability", 250, Between(50, 1000))
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: flag})
			},
			want: "fkrecipes: the technology steel-axes names an EnabledBy setting that this plan never declared",
		},
		{
			name: "an unlock handle from another plan",
			build: func(l *Lib) {
				other := New()
				bronze := other.Item("bronze-axe", ItemSpec{})
				rec := other.Recipe(bronze, RecipeSpec{})
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", Unlocks: []RecipeRef{rec}})
			},
			want: "fkrecipes: the technology steel-axes unlocks a recipe that this plan never declared",
		},
		{
			name: "an AfterTech handle from another plan",
			build: func(l *Lib) {
				other := New()
				anchor := other.Technology("bronze-axes", TechSpec{CostOf: "steel-processing"})
				l.Technology("steel-axes", TechSpec{CostOf: "steel-processing", AfterTech: anchor})
			},
			want: "fkrecipes: the technology steel-axes names an AfterTech technology that this plan never declared",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanData(baseWorld())
			if err == nil {
				t.Fatalf("plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}
}

// AfterTech is how a plan chains its own research: After probes the GAME, so
// an own-tech name would log a drop and leave the second technology detached.
func TestAfterTechChainsOwnTechnologies(t *testing.T) {
	lib := New()
	first := lib.Technology("bronze-axes", TechSpec{CostOf: "steel-processing", After: "steel-processing"})
	lib.Technology("steel-axes", TechSpec{CostOf: "logistics-2", AfterTech: first})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-bronze-axes", prerequisites=["steel-processing"], unit=` + steelProcessingUnit + `}`,
		`extend {type="technology", name="steelworks-steel-axes", prerequisites=["steelworks-bronze-axes"], unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

func TestPlanDataRefusesAnEmptyModName(t *testing.T) {
	lib := New()
	lib.Item("steel-axe", ItemSpec{})

	ops, err := lib.PlanData(baseWorld().withModName(""))
	if err == nil {
		t.Fatal("plan was accepted with no mod name")
	}
	want := "fkrecipes: the mod name is empty, so nothing can be prefixed; package with an fklua that wires ModName"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// TinyGo's wasm int is 32 bits and the Rust mirror's i64 is not, so the
// widths are pinned rather than assumed. These are not realistic Factorio
// numbers: the point is that both halves carry the same one.
func TestWideAmountsSurviveTheEmit(t *testing.T) {
	lib := New()
	// 2^53 exactly: the largest integer a Lua double still holds without
	// rounding, so the planner accepts it and the transcript prints it as
	// the plain digits it is.
	axe := lib.Item("steel-axe", ItemSpec{StackSize: 9007199254740992})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(3000000000, "steel-plate")},
		ResultCount: 2500000000,
	})
	lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
		Count:   5000000000,
		Seconds: 15,
		Packs:   []Pack{{Name: "automation-science-pack", Amount: 3000000000}},
	}})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=9007199254740992}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steel-plate", amount=3000000000}], results=[{type="item", name="steelworks-steel-axe", amount=2500000000}]}`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=5000000000, time=15, ingredients=[["automation-science-pack", 3000000000]]}}`,
	})
}

// A plan is a snapshot. Reusing one buffer across several declarations is
// ordinary Go, and a consumer who does it must not find their first recipe
// rewritten by their second.
func TestDeclarationsDoNotAliasCallerSlices(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	head := lib.Item("axe-head", ItemSpec{})

	ingredients := []Ingredient{IngredientOf(head, 1), IngredientNamed(4, "steel-plate")}
	lib.Recipe(axe, RecipeSpec{Ingredients: ingredients})
	// The caller reuses the buffer for their next recipe.
	ingredients[0] = IngredientNamed(99, "copper-plate")
	ingredients[1] = IngredientNamed(99, "iron-plate")

	packs := []Pack{{Name: "automation-science-pack", Amount: 1}}
	unit := &UnitSpec{Count: 50, Seconds: 15, Packs: packs}
	lib.Technology("steel-axes", TechSpec{Unit: unit})
	// The caller keeps the unit and edits it afterwards.
	packs[0].Amount = 99
	unit.Count = 99
	unit.Seconds = 99

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="item", name="steelworks-axe-head", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[{type="item", name="steelworks-axe-head", amount=1}, {type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="technology", name="steelworks-steel-axes", unit={count=50, time=15, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// Go-only: the Rust mirror takes &dyn World, which cannot be null.
func TestPlanningRefusesANilWorld(t *testing.T) {
	lib := New()
	lib.Item("steel-axe", ItemSpec{})

	if _, err := lib.PlanData(nil); err == nil || err.Error() != "fkrecipes: PlanData was given a nil World" {
		t.Errorf("PlanData(nil) gave %v", err)
	}
	if _, err := lib.PlanSettings(nil); err == nil || err.Error() != "fkrecipes: PlanSettings was given a nil World" {
		t.Errorf("PlanSettings(nil) gave %v", err)
	}
}

// A max_level the World reports as present but nil is a value this library
// could not carry across; writing it would put a nil into the prototype.
func TestANilMaxLevelIsNotEmitted(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing"})

	ops, err := lib.PlanData(baseWorld().withNilMaxLevel("steel-processing"))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit=` + steelProcessingUnit + `}`,
	})
}

// A Set op's value reaches fkdata.Set, where a nil DELETES the key instead of
// writing one. Nothing plans a deletion, and this is what says so.
func TestNoPlannedSetOpCarriesANilValue(t *testing.T) {
	lib := New()
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "logistics-2", Before: "logistics-3"})
	lib.Technology("bronze-axes", TechSpec{CostOf: "steel-processing", After: "electronics", Before: "logistics-3"})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	sets := 0
	for _, op := range ops {
		if op.Kind != OpSet {
			continue
		}
		sets++
		if op.Val.Kind == KindNil {
			t.Errorf("a Set op at %s carries nil, which would delete the key", renderPath(op.Path))
		}
		for _, entry := range op.Val.Arr {
			if entry.Kind == KindNil {
				t.Errorf("a Set op at %s carries a nil entry", renderPath(op.Path))
			}
		}
	}
	if sets != 2 {
		t.Fatalf("expected two splices, got %d", sets)
	}
}

// The whole binding, end to end: the settings stage generates the setting
// with a minimum that clears the engine floor, and the data stage reads the
// player's answer back into energy_required.
func TestCraftTimeBindingRoundTrip(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	from := lib.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
	lib.Recipe(axe, RecipeSpec{
		CraftTimeFrom: from,
		Ingredients:   []Ingredient{IngredientNamed(4, "steel-plate")},
	})

	settingsOps, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	assertLines(t, transcript(settingsOps), []string{
		`extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=2.5000000000000000e0, order="aa", minimum_value=2.0000000000000000e-3}`,
	})

	// The player set it to four seconds.
	dataOps, err := lib.PlanData(baseWorld().withSetting("steelworks-axe-craft-time", Num(4)))
	assertNoError(t, err)
	assertLines(t, transcript(dataOps), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", energy_required=4, enabled=true, ingredients=[{type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// An unreadable setting degrades the same way an unreadable enablement does:
// one log line, and the declared default applies.
func TestCraftTimeBindingFallsBackToItsDefault(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	from := lib.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})
	lib.Recipe(axe, RecipeSpec{CraftTimeFrom: from})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-axe-craft-time was not readable, so its default applies`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", energy_required=2.5000000000000000e0, enabled=true, ingredients=[], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// ---------------------------------------------------------------------------
// Two Libs, two data hooks: the split-emission pattern.
// ---------------------------------------------------------------------------

// THE ONE-DATA-HOOK RULE IS PER Lib, NOT PER MOD, and this is what says so.
//
// A single Lib emitted from two data-family hooks refuses, because the second
// pass finds the first pass's prototypes already in data.raw and reports the
// overwrite. That is a real rule and it is not the rule "a mod gets one data
// hook": a mod may carry TWO plans, one creating its own content at fk_data
// and one patching another mod's tree at fk_data_updates, each emitting its
// own settings at fk_settings. The stages share one data.raw, so what makes it
// work is that the two plans declare different names; nothing else is needed.
//
// The patching plan is planned against a world that carries the creating
// plan's prototypes, which is what data.raw actually looks like by the time
// fk_data_updates runs.
func TestTwoLibsSplitCreationFromPatching(t *testing.T) {
	// PLAN A, at fk_data: this mod's own content.
	creation := New()
	hardened := creation.BoolSetting("hardened-tools", true)
	plate := creation.Item("hardened-steel-plate", ItemSpec{})
	creation.Recipe(plate, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}})
	creation.Technology("hardened-steel", TechSpec{
		CostOf:    "logistics-2",
		After:     "steel-processing",
		EnabledBy: hardened,
	})

	// The setting is answered rather than left unreadable, so the transcript
	// is the split itself and not a degradation log.
	aOps, err := creation.PlanData(baseWorld().withSetting("steelworks-hardened-tools", Bool(true)))
	assertNoError(t, err)
	assertLines(t, transcript(aOps), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
		`extend {type="technology", name="steelworks-hardened-steel", prerequisites=["steel-processing"], unit=` + logistics2Unit + `, enabled=true}`,
	})

	// data.raw AS PLAN A LEFT IT. The patching plan runs a stage later, so
	// everything above is already there and is what it plans against.
	after := baseWorld().
		withSetting("steelworks-deep-tempering", Bool(true)).
		withItem("steelworks-hardened-steel-plate").
		withRecipe("steelworks-hardened-steel-plate").
		withTech(fixtureTech{
			name:    "steelworks-hardened-steel",
			prereqs: []string{"steel-processing"},
			unit:    logistics2UnitValue(),
		})

	// PLAN B, at fk_data_updates: a patch, spliced around plan A's OWN
	// technology by name. It cannot use AfterTech, because that takes a handle
	// and handles do not cross plans; a name is how one plan reaches another's
	// emitted prototype, which is the same way it reaches any other mod's.
	patch := New()
	deep := patch.BoolSetting("deep-tempering", true)
	patch.Technology("tempering", TechSpec{
		CostOf:    "logistics-3",
		After:     "steel-processing",
		Before:    "steelworks-hardened-steel",
		EnabledBy: deep,
	})

	bOps, err := patch.PlanData(after)
	if err != nil {
		t.Fatalf("the patching plan was refused: %v", err)
	}
	assertLines(t, transcript(bOps), []string{
		`extend {type="technology", name="steelworks-tempering", prerequisites=["steel-processing"], unit=` + logistics3Unit + `, enabled=true}`,
		`set technology.steelworks-hardened-steel.prerequisites = ["steelworks-tempering"]`,
	})

	// Each plan emits ITS OWN settings, and the two sets are disjoint: the
	// settings stage runs once, so both hooks route into it and a shared name
	// would be the silent last-writer-wins the settings validator refuses
	// within one plan and cannot see across two.
	aSettings, err := creation.PlanSettings(settingsWorld())
	assertNoError(t, err)
	bSettings, err := patch.PlanSettings(settingsWorld())
	assertNoError(t, err)
	assertLines(t, transcript(aSettings), []string{
		`extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="aa"}`,
	})
	assertLines(t, transcript(bSettings), []string{
		`extend {type="bool-setting", name="steelworks-deep-tempering", setting_type="startup", default_value=true, order="aa"}`,
	})
}

// The other half of the rule, so the pair cannot both pass on a library that
// never refuses an overwrite: two plans that DO share a name are refused, and
// the refusal comes from the world carrying the first plan's prototype rather
// than from anything the second plan knows about the first.
func TestASecondLibSharingANameIsStillRefused(t *testing.T) {
	patch := New()
	patch.Technology("hardened-steel", TechSpec{CostOf: "logistics-3"})

	after := baseWorld().withTech(fixtureTech{
		name:    "steelworks-hardened-steel",
		prereqs: []string{"steel-processing"},
		unit:    logistics2UnitValue(),
	})

	_, err := patch.PlanData(after)
	if err == nil {
		t.Fatal("the plan was accepted, want an overwrite refusal")
	}
	want := "fkrecipes: the technology steelworks-hardened-steel already exists in data.raw; this plan would overwrite it"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}
