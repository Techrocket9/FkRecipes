package fkrecipes

import (
	"strings"
	"testing"
)

// The three surfaces a mod with hand-rolled settings needs to migrate onto
// this library: names it can keep, ingredients a dropdown chooses, and a
// research cost a dropdown chooses.

func TestLegacySettingsKeepTheirNamesAndOrders(t *testing.T) {
	lib := New()
	lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
		[]string{"vanilla", "cheap", "belt-fast"}, "a")
	lib.LegacyBoolSetting("bbb-multi-edge-parts", false, "b")
	lib.LegacyIntSetting("bbb-batch", 4, Between(1, 20), "c")
	lib.LegacyDoubleSetting("bbb-speed", 2.5, NumericSpec{}, "d")
	// A generated setting beside them keeps the prefix and the derived order.
	lib.BoolSetting("hardened-tools", true)

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="bbb-recipe-cost", setting_type="startup", default_value="vanilla", order="a", allowed_values=["vanilla", "cheap", "belt-fast"]}`,
		`extend {type="bool-setting", name="bbb-multi-edge-parts", setting_type="startup", default_value=false, order="b"}`,
		`extend {type="int-setting", name="bbb-batch", setting_type="startup", default_value=4, order="c", minimum_value=1, maximum_value=20}`,
		`extend {type="double-setting", name="bbb-speed", setting_type="startup", default_value=2.5000000000000000e0, order="d"}`,
		`extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="ae"}`,
	})
}

// A legacy handle is an ordinary handle: the bindings take it unchanged.
func TestLegacySettingsBindLikeGeneratedOnes(t *testing.T) {
	lib := New()
	enabled := lib.LegacyBoolSetting("bbb-enabled", true, "a")
	forging := lib.LegacyDoubleSetting("bbb-forging-time", 3, NumericSpec{}, "b")
	axe := lib.Item("steel-axe", ItemSpec{})
	lib.Recipe(axe, RecipeSpec{CraftTimeFrom: forging})
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: enabled})

	ops, err := lib.PlanData(baseWorld().
		withSetting("bbb-enabled", Bool(false)).
		withSetting("bbb-forging-time", Num(9)))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", energy_required=9, enabled=true, ingredients=[], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="technology", name="steelworks-steel-axes", unit=` + steelProcessingUnit + `, enabled=false, hidden=true}`,
	})
}

// The generated minimum still applies to a legacy double that backs a
// crafting time, because the engine's floor does not care where the name came
// from.
func TestALegacyDoubleBoundAsACraftTimeGetsTheMinimum(t *testing.T) {
	lib := New()
	forging := lib.LegacyDoubleSetting("bbb-forging-time", 3, NumericSpec{}, "a")
	axe := lib.Item("steel-axe", ItemSpec{})
	lib.Recipe(axe, RecipeSpec{CraftTimeFrom: forging})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="double-setting", name="bbb-forging-time", setting_type="startup", default_value=3, order="a", minimum_value=2.0000000000000000e-3}`,
	})
}

func TestLegacySettingRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			name:  "an empty legacy name",
			build: func(l *Lib) { l.LegacyBoolSetting("", true, "a") },
			want:  "fkrecipes: a setting was declared with an empty name",
		},
		{
			name:  "an empty order",
			build: func(l *Lib) { l.LegacyBoolSetting("bbb-enabled", true, "") },
			want:  "fkrecipes: the legacy setting bbb-enabled was declared with an empty order",
		},
		{
			// The names differ as declared and collide as emitted, which is
			// the namespace the engine keeps.
			name: "a legacy name colliding with a generated one",
			build: func(l *Lib) {
				l.BoolSetting("hardened-tools", true)
				l.LegacyBoolSetting("steelworks-hardened-tools", false, "a")
			},
			want: "fkrecipes: two settings share the name steelworks-hardened-tools; the engine keeps the last one silently",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanSettings(settingsWorld())
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

// CheckLocale reads a legacy setting under the name it actually carries, and
// polices its dropdown values under that name too.
func TestCheckLocaleCoversLegacyNames(t *testing.T) {
	lib := New()
	lib.LegacyDropdownSettingNeedingLocale("bbb-recipe-cost", "vanilla",
		[]string{"vanilla", "cheap"}, "a")

	cfg := `[mod-setting-name]
bbb-recipe-cost=Recipe cost

[string-mod-setting]
bbb-recipe-cost-vanilla=Vanilla
bbb-recipe-cost-belt-express=Express belts
`
	assertLines(t, lib.CheckLocale("better-belt-balancer", cfg), []string{
		"the dropdown setting bbb-recipe-cost has no [string-mod-setting] entry for its value cheap",
		"the [string-mod-setting] entry bbb-recipe-cost-belt-express matches no dropdown value this plan declares",
	})
}

// ---------------------------------------------------------------------------
// Ingredients a dropdown chooses.
// ---------------------------------------------------------------------------

func quenchPlan(choices []IngredientChoice) *Lib {
	lib := New()
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	lib.Recipe(plate, RecipeSpec{
		IngredientsBy: &IngredientChoices{Setting: medium, Choices: choices},
	})
	return lib
}

func waterAndOil() []IngredientChoice {
	return []IngredientChoice{
		{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
		{Value: "oil", Ingredients: []Ingredient{IngredientNamed(3, "iron-plate")}},
	}
}

func TestIngredientsByFollowsTheSetting(t *testing.T) {
	ops, err := quenchPlan(waterAndOil()).PlanData(
		baseWorld().withSetting("steelworks-quench-medium", Str("oil")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="iron-plate", amount=3}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// An unreadable setting takes the declared default, with the ordinary line.
func TestIngredientsByFallsBackToItsDefaultValue(t *testing.T) {
	ops, err := quenchPlan(waterAndOil()).PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-quench-medium was not readable, so its default applies`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// A chosen plan that names nothing this game has is a recipe made of nothing,
// so the default option's plan applies instead and says so.
func TestIngredientsByFallsBackToTheDefaultPlan(t *testing.T) {
	choices := []IngredientChoice{
		{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
		{Value: "oil", Ingredients: []Ingredient{IngredientNamed(3, "tungsten-carbide")}},
	}
	ops, err := quenchPlan(choices).PlanData(
		baseWorld().withSetting("steelworks-quench-medium", Str("oil")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: hardened-steel-plate: none of tungsten-carbide is present, so the ingredient is dropped`,
		`log fkrecipes: hardened-steel-plate: the oil ingredients name nothing this game has, so the water ingredients apply`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// When the default resolves to nothing either, the recipe is emitted with no
// ingredients and every drop is on the record. The load completes and says
// what happened rather than breaking.
func TestIngredientsByEmitsNothingWhenNoPlanResolves(t *testing.T) {
	choices := []IngredientChoice{
		{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "titanium-plate")}},
		{Value: "oil", Ingredients: []Ingredient{IngredientNamed(3, "tungsten-carbide")}},
	}
	ops, err := quenchPlan(choices).PlanData(
		baseWorld().withSetting("steelworks-quench-medium", Str("oil")))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: hardened-steel-plate: none of tungsten-carbide is present, so the ingredient is dropped`,
		`log fkrecipes: hardened-steel-plate: the oil ingredients name nothing this game has, so the water ingredients apply`,
		`log fkrecipes: hardened-steel-plate: none of titanium-plate is present, so the ingredient is dropped`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// ---------------------------------------------------------------------------
// A research cost a dropdown chooses.
// ---------------------------------------------------------------------------

func tierPlan(choices []CostChoice) *Lib {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics", "military"})
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: choices,
			Fallback: UnitSpec{
				Count:   60,
				Seconds: 30,
				Packs:   []Pack{{Name: "automation-science-pack", Amount: 1}},
			},
		},
	})
	return lib
}

// The ladder takes the first source that exists and carries a unit, copies it
// with its level cap, and makes that source the sole prerequisite.
func TestCostByCopiesTheUnitAndTakesThePrerequisite(t *testing.T) {
	choices := []CostChoice{
		{Value: "logistics", Sources: []string{"logistics-4", "mining-productivity-4"}},
		{Value: "military", Sources: []string{"steel-processing"}},
	}
	ops, err := tierPlan(choices).PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["mining-productivity-4"], unit={count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]], mod_cost_tier="mid-game", time=60}, max_level="infinite"}`,
	})
}

// AN ABSENT SOURCE IS STEPPED PAST, which is the ladder's whole reason for
// being a list: a mod prices its research from whichever of several
// technologies the player's install actually has.
//
// There is no presence probe in the walk. A technology the game does not have
// carries no unit either, so the "carries no usable unit" arm steps past an
// absent rung by the same test it steps past a unit-less one, and this test is
// what says that arm really does cover absence. It goes red if that arm is
// broken, which is what makes the deleted TechExists check unnecessary rather
// than merely redundant.
func TestCostByStepsPastASourceThatIsNotThere(t *testing.T) {
	choices := []CostChoice{
		// Two rungs no vanilla install has, then one it does.
		{Value: "logistics", Sources: []string{"quarry-drills", "logistics-4", "steel-processing"}},
		{Value: "military", Sources: []string{"steel-processing"}},
	}
	ops, err := tierPlan(choices).PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["steel-processing"], unit=` + steelProcessingUnit + `}`,
	})
}

// A PRESENT-BUT-UNUSABLE UNIT IS STEPPED PAST, which witnesses the SHAPE half
// of the unit arm independently of the flag: this rung answers ok=true, so
// only the Kind check can reject it.
//
// A present nil is what fromV produces for a unit whose table carried a key
// this library drops, so it is a real answer rather than an invented one.
func TestCostByStepsPastAPresentButUnusableUnit(t *testing.T) {
	choices := []CostChoice{
		{Value: "logistics", Sources: []string{"steel-processing", "logistics-2"}},
		{Value: "military", Sources: []string{"logistics-2"}},
	}
	w := baseWorld().withNilUnit("steel-processing")
	ops, err := tierPlan(choices).PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit=` + logistics2Unit + `}`,
	})
}

// THE ok FLAG IS HONOURED OVER THE VALUE THAT RIDES WITH IT, which is the
// second of the unit arm's two guards and needs its own witness.
//
// GO ONLY, AND THAT IS THE TYPE'S DOING RATHER THAN AN OMISSION. Go's TechUnit
// returns (Value, bool), a pair that can disagree with itself, so the flag and
// the value are two guards. Rust's tech_unit returns Option<Value>, where a
// value cannot ride along with absence at all: the case below is
// unrepresentable there, and its single fall-through arm is the whole guard.
//
// The World contract makes the flag the answer to "is there a unit here", so a
// World that hands back a real map beside ok=false is violating it. Every
// World this repository ships returns Nil beside false, which is why the shape
// check alone would look sufficient: only a fixture that breaks the contract
// on purpose can tell the two terms apart. A consumer's own World can break it
// by accident, and the cost of getting this wrong is a technology priced from
// a unit the game does not have.
func TestCostByHonoursTheAbsentFlagOverAMapValue(t *testing.T) {
	choices := []CostChoice{
		// steel-processing really is in this world and really does carry a
		// map, so the shape check waves it through; only the flag rejects it.
		{Value: "logistics", Sources: []string{"steel-processing", "logistics-2"}},
		{Value: "military", Sources: []string{"logistics-2"}},
	}
	w := baseWorld().withMapUnitButAbsent("steel-processing")
	ops, err := tierPlan(choices).PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"], unit=` + logistics2Unit + `}`,
	})
}

// A research_trigger technology is not a cost source, so the ladder steps past
// it exactly as it steps past one that is not there.
//
// The trigger flag is what has to do the work here, so the fixture's
// steam-power is given a dictionary unit it would not have in the game: with
// no unit the "carries no unit" arm would step past it anyway and the trigger
// check would never be the reason.
func TestCostByStepsPastAResearchTriggerSource(t *testing.T) {
	choices := []CostChoice{
		{Value: "logistics", Sources: []string{"steam-power", "steel-processing"}},
		{Value: "military", Sources: []string{"steel-processing"}},
	}
	w := baseWorld().withUnit("steam-power", Obj(
		kv("count", Num(1)),
		kv("time", Num(1)),
		kv("ingredients", Arr(Arr(Str("automation-science-pack"), Num(1)))),
	))
	ops, err := tierPlan(choices).PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["steel-processing"], unit=` + steelProcessingUnit + `}`,
	})
}

// When no source in the chosen ladder carries a unit, the fallback cost
// applies and the technology hangs off nothing.
func TestCostByFallsBackWithNoPrerequisite(t *testing.T) {
	choices := []CostChoice{
		{Value: "logistics", Sources: []string{"logistics-4", "steam-power"}},
		{Value: "military", Sources: []string{"steel-processing"}},
	}
	ops, err := tierPlan(choices).PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-tips-research-tier was not readable, so its default applies`,
		`log fkrecipes: hardened-tips: no source for the logistics cost carries a unit, so the fallback cost applies and the technology has no prerequisite`,
		`extend {type="technology", name="steelworks-hardened-tips", unit={count=60, time=30, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// The edge the ladder chose joins the cycle overlay like any other.
func TestCostByEdgeReachesTheCycleWalk(t *testing.T) {
	choices := []CostChoice{
		{Value: "logistics", Sources: []string{"logistics-2"}},
		{Value: "military", Sources: []string{"steel-processing"}},
	}
	lib := tierPlan(choices)
	// logistics-2 is made to require the technology the plan is about to add,
	// so the copied edge closes a ring.
	w := baseWorld().withPrereqs("logistics-2", "steelworks-hardened-tips")

	_, err := lib.PlanData(w)
	if err == nil {
		t.Fatal("the plan was accepted, want a cycle refusal")
	}
	want := "fkrecipes: a prerequisite cycle: logistics-2 -> steelworks-hardened-tips -> logistics-2"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}

func TestChoiceRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			name: "a recipe naming both ingredient forms",
			build: func(l *Lib) {
				medium := l.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water"})
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					Ingredients:   []Ingredient{IngredientNamed(1, "steel-plate")},
					IngredientsBy: &IngredientChoices{Setting: medium, Choices: []IngredientChoice{{Value: "water"}}},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate names both Ingredients and IngredientsBy; pick one",
		},
		{
			// This plan declares a dropdown of its own, so the stray handle
			// is IN RANGE here and only the per-plan tag can tell it apart.
			name: "an ingredients setting from another plan",
			build: func(l *Lib) {
				other := New()
				stray := other.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water"})
				l.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water"})
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					IngredientsBy: &IngredientChoices{Setting: stray, Choices: []IngredientChoice{{Value: "water"}}},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate names an ingredients setting that this plan never declared",
		},
		{
			name: "a choice for a value the setting does not allow",
			build: func(l *Lib) {
				medium := l.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					IngredientsBy: &IngredientChoices{Setting: medium, Choices: []IngredientChoice{
						{Value: "water"}, {Value: "brine"},
					}},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate offers something for brine where the setting steelworks-quench-medium allows oil",
		},
		{
			name: "a value with no choice behind it",
			build: func(l *Lib) {
				medium := l.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					IngredientsBy: &IngredientChoices{Setting: medium, Choices: []IngredientChoice{{Value: "water"}}},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate offers nothing for the value oil that the setting steelworks-quench-medium allows",
		},
		{
			name: "a choice beyond what the setting allows",
			build: func(l *Lib) {
				medium := l.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water"})
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					IngredientsBy: &IngredientChoices{Setting: medium, Choices: []IngredientChoice{
						{Value: "water"}, {Value: "oil"},
					}},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate offers something for oil, which the setting steelworks-quench-medium does not allow",
		},
		{
			name: "an ingredient inside a choice that names nothing this plan declared",
			build: func(l *Lib) {
				medium := l.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water"})
				plate := l.Item("hardened-steel-plate", ItemSpec{})
				l.Recipe(plate, RecipeSpec{
					IngredientsBy: &IngredientChoices{Setting: medium, Choices: []IngredientChoice{
						{Value: "water", Ingredients: []Ingredient{IngredientOf(ItemRef{}, 1)}},
					}},
				})
			},
			want: "fkrecipes: the recipe hardened-steel-plate names an ingredient item that this plan never declared",
		},
		{
			name: "a technology naming CostBy and a placement",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
				l.Technology("hardened-tips", TechSpec{
					After: "steel-processing",
					CostBy: &CostChoices{Setting: tier, Choices: []CostChoice{{Value: "logistics"}},
						Fallback: UnitSpec{Count: 1, Seconds: 1}},
				})
			},
			want: "fkrecipes: the technology hardened-tips names CostBy with a placement; the prerequisite moves with the unit, so CostBy places the technology itself",
		},
		{
			// In range here too, for the same reason.
			name: "a cost setting from another plan",
			build: func(l *Lib) {
				other := New()
				stray := other.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
				l.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
				l.Technology("hardened-tips", TechSpec{
					CostBy: &CostChoices{Setting: stray, Choices: []CostChoice{{Value: "logistics"}},
						Fallback: UnitSpec{Count: 1, Seconds: 1}},
				})
			},
			want: "fkrecipes: the technology hardened-tips names a cost setting that this plan never declared",
		},
		{
			name: "a cost choice for a value the setting does not allow",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
				l.Technology("hardened-tips", TechSpec{
					CostBy: &CostChoices{Setting: tier, Choices: []CostChoice{{Value: "military"}},
						Fallback: UnitSpec{Count: 1, Seconds: 1}},
				})
			},
			want: "fkrecipes: the technology hardened-tips offers something for military where the setting steelworks-tips-research-tier allows logistics",
		},
		{
			// The fallback is the cost that applies when nothing else does, so
			// it is held to the same rules as a hand-rolled one.
			name: "a fallback the engine would refuse",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
				l.Technology("hardened-tips", TechSpec{
					CostBy: &CostChoices{Setting: tier, Choices: []CostChoice{{Value: "logistics"}},
						Fallback: UnitSpec{Count: 0, Seconds: 30}},
				})
			},
			want: "fkrecipes: the technology hardened-tips has a unit count below 1, which the engine refuses",
		},
		{
			// The chosen value names no source at all, so the fallback IS what
			// applies: its packs are probed, the only one drops, and a cost with
			// nothing left is refused. A fallback nobody reaches is a different
			// test, below.
			name: "a fallback that applies and whose every pack the game lacks",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tips-research-tier", "logistics", []string{"logistics"})
				l.Technology("hardened-tips", TechSpec{
					CostBy: &CostChoices{Setting: tier, Choices: []CostChoice{{Value: "logistics"}},
						Fallback: UnitSpec{Count: 60, Seconds: 30,
							Packs: []Pack{{Name: "military-science-pack", Amount: 1}}}},
				})
			},
			want: "fkrecipes: the technology hardened-tips has no science pack the game has; research takes at least one",
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

// A choice list is a snapshot like every other spec slice.
func TestChoicesDoNotAliasTheCallerSlices(t *testing.T) {
	lib := New()
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water"})
	plate := lib.Item("hardened-steel-plate", ItemSpec{})

	ingredients := []Ingredient{IngredientNamed(2, "steel-plate")}
	choices := []IngredientChoice{{Value: "water", Ingredients: ingredients}}
	lib.Recipe(plate, RecipeSpec{IngredientsBy: &IngredientChoices{Setting: medium, Choices: choices}})
	ingredients[0] = IngredientNamed(99, "iron-plate")
	choices[0] = IngredientChoice{Value: "oil"}

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	joined := strings.Join(transcript(ops), "\n")
	if !strings.Contains(joined, `{type="item", name="steel-plate", amount=2}`) {
		t.Errorf("the plan followed the caller's edits:\n%s", joined)
	}
}

// ---------------------------------------------------------------------------
// A recipe that migrates before the technology that unlocks it.
// ---------------------------------------------------------------------------

// The pilot's shape: the recipe moves onto the library while the technology
// stays hand-rolled, so the recipe has to say enabled = false itself and
// nothing in the plan can say it for them. The field lands where the library's
// own enabled would have, not at the tail with the rest of Extra, so the
// golden captured before the migration does not move.
func TestExtraCarriesEnabledWhenNothingInThePlanUnlocksTheRecipe(t *testing.T) {
	lib := New()
	part := lib.LegacyItem("bbb-balancer-part", ItemSpec{})
	lib.LegacyRecipe(part, "bbb-balancer-part", RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		// allow_productivity IS DECLARED FIRST, so a pass through that let
		// enabled ride the tail would emit it after results and after this
		// key rather than in front of ingredients.
		Extra: []KV{kv("allow_productivity", Bool(true)), kv("enabled", Bool(false))},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="bbb-balancer-part", stack_size=50}`,
		`extend {type="recipe", name="bbb-balancer-part", enabled=false, ingredients=[{type="item", name="steel-plate", amount=1}], results=[{type="item", name="bbb-balancer-part", amount=1}], allow_productivity=true}`,
	})
}

// The consumer's value is EMITTED, not merely tolerated: enabled = true on a
// recipe nothing unlocks is what the library would have written anyway, and
// false is the migration case, so the true arm alone could not tell a
// passthrough from a library default. This one gives the value the library
// would not have chosen if any technology had been there.
func TestExtraEnabledIsTheValueThatReachesThePrototype(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(4, "steel-plate")},
		Extra:       []KV{kv("enabled", Bool(false))},
	})

	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=false, ingredients=[{type="item", name="steel-plate", amount=4}], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// The other direction. A technology in this plan unlocks the recipe, so the
// library owns enabled again and the refusal names the technology that
// decided it, which is the line the consumer has to delete.
func TestExtraCannotCarryEnabledWhenAPlanTechnologyUnlocksTheRecipe(t *testing.T) {
	lib := New()
	part := lib.LegacyItem("bbb-balancer-part", ItemSpec{})
	rec := lib.LegacyRecipe(part, "bbb-balancer-part", RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		Extra:       []KV{kv("enabled", Bool(false))},
	})
	// The technology is declared AFTER the recipe, and the refusal comes from
	// the recipe loop, which runs first: the check reads the whole plan rather
	// than what has been declared by the time the recipe was.
	lib.Technology("balancer", TechSpec{CostOf: "logistics-2", Unlocks: []RecipeRef{rec}})

	_, err := lib.PlanData(baseWorld())
	assertRefusal(t, err, "fkrecipes: the recipe bbb-balancer-part puts enabled in Extra, "+
		"but the technology balancer unlocks it, so the library owns that field")
}

// A handle from ANOTHER plan is not an unlock in this one. The technology loop
// is what refuses it, by name; the enabled check must not follow it and read
// past this plan's recipes on the way.
func TestExtraEnabledIsNotDecidedByAForeignUnlockHandle(t *testing.T) {
	other := New()
	otherPart := other.Item("other-part", ItemSpec{})
	otherRec := other.Recipe(otherPart, RecipeSpec{})

	lib := New()
	part := lib.LegacyItem("bbb-balancer-part", ItemSpec{})
	lib.LegacyRecipe(part, "bbb-balancer-part", RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		Extra:       []KV{kv("enabled", Bool(false))},
	})
	lib.Technology("balancer", TechSpec{CostOf: "logistics-2", Unlocks: []RecipeRef{otherRec}})

	_, err := lib.PlanData(baseWorld())
	assertRefusal(t, err, "fkrecipes: the technology balancer unlocks a recipe that this plan never declared")
}

// Everything else about Extra is unchanged on a recipe: another field the
// library emits is still refused, and enabled twice is still the consumer's
// own last-writer.
func TestExtraEnabledDoesNotLoosenTheOtherRecipeChecks(t *testing.T) {
	cases := []struct {
		name  string
		extra []KV
		want  string
	}{
		{
			name:  "a field the library emits, beside an accepted enabled",
			extra: []KV{kv("enabled", Bool(false)), kv("results", Arr())},
			want:  "fkrecipes: the recipe steel-axe sets results through Extra, which this library emits",
		},
		{
			name:  "enabled twice",
			extra: []KV{kv("enabled", Bool(false)), kv("enabled", Bool(true))},
			want:  "fkrecipes: the recipe steel-axe sets enabled through Extra twice",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			axe := lib.Item("steel-axe", ItemSpec{})
			lib.Recipe(axe, RecipeSpec{Extra: c.extra})

			_, err := lib.PlanData(baseWorld())
			assertRefusal(t, err, c.want)
		})
	}
}
