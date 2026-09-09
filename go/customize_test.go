package fkrecipes

import (
	"math"
	"strconv"
	"strings"
	"testing"
)

// THE CUSTOMIZER'S TESTS: the settings a player edits, the bindings that give
// them meaning, and what the data stage does with what they typed.
//
// The language itself is pinned by testdata/ingredient-list/cases.txt and
// ingredientlist_test.go. Nothing here re-tests a message the corpus owns; what
// is held up to the light here is the BINDING: which setting a recipe reads,
// what the settings screen shows, and which of the three text paths applies.

// customWorld is baseWorld plus the two names the customizer's fixtures reach
// for: an iron stick to type into a list, and a military science pack so a
// declared pack ladder has a second rung that exists.
func customWorld() *fixtureWorld {
	return baseWorld().withItem("iron-stick").withTool("military-science-pack")
}

// ---------------------------------------------------------------------------
// The settings stage.
// ---------------------------------------------------------------------------

// The emitted prototype, whole. default_value is the WORD, not the list: the
// engine stores every setting's current value including untouched defaults, so
// a rendered default would freeze a silent player's balance at the day they
// installed the mod. The list is in the description instead, and the ladder
// shows its first rung there because the field itself never shows a name.
func TestPlanSettingsEmitsATextSetting(t *testing.T) {
	lib := New()
	rivet := lib.Item("steel-rivet", ItemSpec{})
	parts := lib.IngredientsSetting("rivet-ingredients", []Ingredient{
		IngredientNamed(2, "tungsten-plate", "steel-plate"),
		IngredientOf(rivet, 4),
	})
	lib.Recipe(rivet, RecipeSpec{Name: "steel-rivet-forging", IngredientsFrom: parts})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="steelworks-rivet-ingredients", setting_type="startup",` +
			` default_value="default", order="aa", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-rivet-ingredients"],` +
			` "` + "\n" + `default: 2 tungsten-plate, 4 steelworks-steel-rivet"]}`,
	})
}

// A pack list is the same prototype with the same word in it, and an empty
// ingredient list renders as none: a recipe the author declared free reads as
// free in the description rather than as a blank the engine would refuse.
func TestPlanSettingsEmitsAPacksSettingAndAnEmptyList(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	free := lib.IngredientsSetting("axe-ingredients", nil)
	lib.Recipe(axe, RecipeSpec{IngredientsFrom: free})
	packs := lib.PacksSetting("axe-packs", []Pack{
		{Name: "automation-science-pack", Amount: 1},
		{Name: "military-science-pack", Amount: 2, Fallbacks: []string{"logistic-science-pack"}},
	})
	count := lib.IntSetting("axe-count", 20, Between(1, 100000))
	seconds := lib.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
	lib.Technology("steel-axes", TechSpec{
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="steelworks-axe-ingredients", setting_type="startup",` +
			` default_value="default", order="aa", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-axe-ingredients"], "` + "\n" + `default: none"]}`,
		`extend {type="string-setting", name="steelworks-axe-packs", setting_type="startup",` +
			` default_value="default", order="ab", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-axe-packs"],` +
			` "` + "\n" + `default: 1 automation-science-pack, 2 military-science-pack"]}`,
		`extend {type="int-setting", name="steelworks-axe-count", setting_type="startup", default_value=20, order="ac", minimum_value=1, maximum_value=100000}`,
		`extend {type="double-setting", name="steelworks-axe-seconds", setting_type="startup", default_value=10, order="ad", minimum_value=5.0000000000000000e-1, maximum_value=600}`,
	})
}

// A legacy text setting keeps the name the mod already ships and the order it
// already chose, for exactly the reason every other Legacy constructor exists:
// mod-settings.dat is keyed by name and the engine has no rename.
func TestPlanSettingsEmitsALegacyTextSetting(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	parts := lib.LegacyIngredientsSetting("bbb-part-ingredients",
		[]Ingredient{IngredientNamed(3, "steel-plate")}, "c")
	lib.Recipe(axe, RecipeSpec{IngredientsFrom: parts})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="bbb-part-ingredients", setting_type="startup",` +
			` default_value="default", order="c", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.bbb-part-ingredients"], "` + "\n" + `default: 3 steel-plate"]}`,
	})
}

// The composed dropdown description: the consumer's own key, then one nested
// string per preset labelled by the value's OWN locale entry, because the
// settings screen shows the player that label and not the raw key.
func TestPlanSettingsComposesADropdownDescription(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil", "custom"})
	quench := lib.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	lib.Recipe(plate, RecipeSpec{
		Category: "chemistry",
		IngredientsBy: &IngredientChoices{
			Setting: medium,
			Choices: []IngredientChoice{
				{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate"), FluidIngredient(10, "water")}},
				{Value: "oil", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate"), FluidIngredient(0.5, "lubricant")}},
			},
			Custom: quench,
		},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="steelworks-quench-medium", setting_type="startup",` +
			` default_value="water", order="aa", allowed_values=["water", "oil", "custom"],` +
			` localised_description=["", ["mod-setting-description.steelworks-quench-medium"],` +
			` ["", "` + "\n" + `", ["string-mod-setting.steelworks-quench-medium-water"], ": 2 steel-plate, 10 [fluid=water]"],` +
			` ["", "` + "\n" + `", ["string-mod-setting.steelworks-quench-medium-oil"], ": 2 steel-plate, 0.5 [fluid=lubricant]"]]}`,
		`extend {type="string-setting", name="steelworks-quench-ingredients", setting_type="startup",` +
			` default_value="default", order="ab", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-quench-ingredients"], "` + "\n" + `default: 2 steel-plate"]}`,
	})
}

// A research dropdown's preset is a technology whose cost is copied, so the
// line names it rather than pretending to render a unit the game owns.
//
// AND IT NAMES IT THE WAY THE PLAYER SEES IT. The line ends in the source's
// LOCALISED name, five parameters rather than four, because the internal name
// beside a tech tree showing "Military 4" names nothing the player can find.
// technology-name is the game's own section, so this adds no locale entry for
// the mod to write; a choice with no source at all still ends in a plain
// string, because there is no technology to name.
func TestPlanSettingsComposesACostDropdownDescription(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-tier", "projectile", []string{"projectile", "none", "custom"})
	packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tips-count", 30, Between(1, 100000))
	seconds := lib.DoubleSetting("tips-seconds", 15, Between(0.5, 600))
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{
				{Value: "projectile", Sources: []string{"mining-productivity-4", "logistics-3"}},
				{Value: "none"},
			},
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
			Custom: &CustomCost{
				Packs: packs, Count: count, Seconds: seconds, Position: []string{"logistics-2"},
			},
		},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	got, ok := field(ops[0].Proto, "localised_description")
	if !ok {
		t.Fatalf("the dropdown carries no composed description")
	}
	want := `["", ["mod-setting-description.steelworks-tips-tier"],` +
		` ["", "` + "\n" + `", ["string-mod-setting.steelworks-tips-tier-projectile"], ": cost of ", ["technology-name.mining-productivity-4"]],` +
		` ["", "` + "\n" + `", ["string-mod-setting.steelworks-tips-tier-none"], ": the fallback cost"]]`
	if renderValue(got) != want {
		t.Errorf("\n got: %s\nwant: %s", renderValue(got), want)
	}
}

// PAST NINETEEN PRESETS THE COMPOSITION STILL NESTS, AND THE NEW LINE IS WHAT
// IT NESTS. A preset line is ONE parameter of the group above it however many
// tables sit inside it, so the five-parameter cost line does not eat into the
// ceiling and the flat-then-nested shape is exactly the one a four-parameter
// line produced. The technology-name table rides BESIDE the label, at the same
// level, and a one-preset description is three table levels deep either way,
// nowhere near the twenty levels the engine takes.
func TestCostDropdownDescriptionNestsPastNineteenPresets(t *testing.T) {
	const presets = 21
	values := make([]string, 0, presets+1)
	choices := make([]CostChoice, 0, presets)
	for i := 0; i < presets; i++ {
		v := "tier" + strconv.Itoa(i)
		values = append(values, v)
		choices = append(choices, CostChoice{Value: v, Sources: []string{"source" + strconv.Itoa(i)}})
	}
	values = append(values, "custom")

	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-tier", values[0], values)
	packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tips-count", 30, Between(1, 100000))
	seconds := lib.DoubleSetting("tips-seconds", 15, Between(0.5, 600))
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting:  tier,
			Choices:  choices,
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
			Custom: &CustomCost{
				Packs: packs, Count: count, Seconds: seconds, Position: []string{"logistics-2"},
			},
		},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	got, ok := field(ops[0].Proto, "localised_description")
	if !ok {
		t.Fatalf("the dropdown carries no composed description")
	}
	// Twenty-two parameters, so nineteen stay and the twentieth slot nests.
	if got.Kind != KindArr || len(got.Arr) != maxLocalisedParams+1 {
		t.Fatalf("the top level is %s", renderValue(got))
	}
	if tail := got.Arr[maxLocalisedParams]; tail.Kind != KindArr || len(tail.Arr) != 4 {
		t.Errorf("the last slot is %s, want a nested group of three lines", renderValue(tail))
	}
	lines := 0
	var walk func(Value, int)
	walk = func(v Value, depth int) {
		if v.Kind != KindArr {
			return
		}
		if len(v.Arr) > maxLocalisedParams+1 {
			t.Errorf("a level holds %d elements, want at most %d", len(v.Arr), maxLocalisedParams+1)
		}
		if depth > 20 {
			t.Fatalf("the nesting is deeper than the engine takes")
		}
		// A preset line rather than a group: the newline in the second slot is
		// what tells them apart, and a line is counted and not descended into.
		if len(v.Arr) > 1 && v.Arr[1].Kind == KindStr && v.Arr[1].Str == "\n" {
			lines++
			if len(v.Arr) != 5 {
				t.Errorf("a cost preset line holds %d parameters, want 5: %s", len(v.Arr), renderValue(v))
			}
			return
		}
		for _, item := range v.Arr {
			walk(item, depth+1)
		}
	}
	walk(got, 1)
	if lines != presets {
		t.Errorf("the description holds %d preset lines, want %d", lines, presets)
	}
}

// THE ENGINE'S CEILING IS TWENTY PARAMETERS AND TWENTY LEVELS (measured: 21 of
// either refuses), so a dropdown with more presets than fit nests instead of
// overflowing. Each level keeps at most twenty parameters, and a level that
// needs more hands the rest to a nested group in its last slot.
func TestLocalisedGroupNestsPastTheEngineCeiling(t *testing.T) {
	params := func(n int) []Value {
		out := make([]Value, 0, n)
		for i := 0; i < n; i++ {
			out = append(out, Str(strconv.Itoa(i)))
		}
		return out
	}
	// Twenty fits flat: the key plus twenty parameters is twenty-one elements.
	flat := localisedGroup(params(20))
	if len(flat.Arr) != 21 {
		t.Errorf("twenty parameters produced %d elements, want 21", len(flat.Arr))
	}
	// Twenty-one does not: nineteen stay and the twentieth slot nests.
	nested := localisedGroup(params(21))
	if len(nested.Arr) != 21 {
		t.Fatalf("twenty-one parameters produced %d elements, want 21", len(nested.Arr))
	}
	tail := nested.Arr[20]
	if tail.Kind != KindArr || len(tail.Arr) != 3 {
		t.Fatalf("the last slot is %s, want a nested group of two", renderValue(tail))
	}
	if renderValue(tail) != `["", "19", "20"]` {
		t.Errorf("the nested group is %s", renderValue(tail))
	}
	// Every level obeys the ceiling, however deep it goes.
	var walk func(Value, int)
	walk = func(v Value, depth int) {
		if v.Kind != KindArr {
			return
		}
		if len(v.Arr) > maxLocalisedParams+1 {
			t.Errorf("a level holds %d elements, want at most %d", len(v.Arr), maxLocalisedParams+1)
		}
		if depth > 20 {
			t.Fatalf("the nesting is deeper than the engine takes")
		}
		for _, item := range v.Arr {
			walk(item, depth+1)
		}
	}
	walk(localisedGroup(params(200)), 1)
}

// ---------------------------------------------------------------------------
// Plan-time refusals.
// ---------------------------------------------------------------------------

// Every refusal the bindings add, at BOTH planners: a settings stage that
// accepted a plan the data stage refuses would ship a settings screen for a
// mod that cannot load.
func TestCustomizerPlanRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
		// dataOnly is a refusal the DATA planner's own declaration loops own,
		// which the settings planner never reaches: it renders defaults and
		// composes descriptions, and a plain ingredient list is neither.
		dataOnly bool
	}{
		{
			name: "IngredientsFrom beside Ingredients",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{
					IngredientsFrom: parts,
					Ingredients:     []Ingredient{IngredientNamed(1, "steel-plate")},
				})
			},
			want: "fkrecipes: the recipe steel-axe names both Ingredients and IngredientsFrom; pick one",
		},
		{
			name: "IngredientsFrom beside IngredientsBy",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain"})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{
					IngredientsFrom: parts,
					IngredientsBy: &IngredientChoices{
						Setting: style,
						Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
					},
				})
			},
			want: "fkrecipes: the recipe steel-axe names both IngredientsBy and IngredientsFrom; pick one",
		},
		{
			name: "IngredientsFrom naming another plan's setting",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				other := New()
				parts := other.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsFrom: parts})
			},
			want: "fkrecipes: the recipe steel-axe reads its ingredients from a setting that this plan never declared",
		},
		{
			name: "a Custom arm naming another plan's setting",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "custom"})
				other := New()
				parts := other.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
					Setting: style,
					Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
					Custom:  parts,
				}})
			},
			want: "fkrecipes: the recipe steel-axe names a Custom ingredients setting that this plan never declared",
		},
		{
			// The pilot's own defect: the player picks custom and gets a
			// recipe made of nothing, with no line saying why. The value is
			// offered and NO choice covers it, which is what the refusal is
			// about; the test below is the same dropdown with a preset behind
			// the word.
			name: "a dropdown offering custom with no arm",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "custom"})
				l.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
					Setting: style,
					Choices: []IngredientChoice{
						{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}},
					},
				}})
			},
			want: "fkrecipes: the setting steelworks-style offers custom, and the recipe steel-axe names no Custom arm for it",
		},
		{
			// The cost twin, which had no witness of its own.
			name: "a cost dropdown offering custom with no arm",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap", "custom"})
				l.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
					Setting:  tier,
					Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
					Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
				}})
			},
			want: "fkrecipes: the setting steelworks-tier offers custom, and the technology steel-axes names no Custom arm for it",
		},
		{
			// The migration hazard the CustomValue field exists for, named
			// rather than resolved silently in either direction.
			name: "a CustomValue a preset already covers",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "mine"})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
					Setting: style,
					Choices: []IngredientChoice{
						{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}},
						{Value: "mine", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
					},
					CustomValue: "mine",
					Custom:      parts,
				}})
			},
			want: "fkrecipes: the recipe steel-axe gives mine a preset as well as a Custom arm; name the arm's value with CustomValue",
		},
		{
			name: "a Custom arm the dropdown does not offer",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain"})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
					Setting: style,
					Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
					Custom:  parts,
				}})
			},
			want: "fkrecipes: the recipe steel-axe names a Custom arm for custom, which the setting steelworks-style does not offer",
		},
		{
			name: "a Custom arm the dropdown offers twice",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "custom", "custom"})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
					Setting: style,
					Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
					Custom:  parts,
				}})
			},
			want: "fkrecipes: the setting steelworks-style offers custom more than once, and a Custom arm needs it exactly once",
		},
		{
			name: "CostFrom with a Position",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{
					Packs: packs, Count: count, Seconds: seconds, Position: []string{"logistics-2"},
				}})
			},
			want: "fkrecipes: the technology steel-axes names CostFrom with a Position; Position belongs to a Custom arm, and CostFrom is placed by After, Before and AfterTech",
		},
		{
			name: "a Custom cost arm with no Position",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap", "custom"})
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
					Setting:  tier,
					Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
					Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
					Custom:   &CustomCost{Packs: packs, Count: count, Seconds: seconds},
				}})
			},
			want: "fkrecipes: the technology steel-axes names a Custom cost arm with no Position; the arm places the technology, so it needs a prerequisite ladder",
		},
		{
			name: "a CustomCost naming another plan's packs setting",
			build: func(l *Lib) {
				other := New()
				packs := other.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the technology steel-axes reads its science packs from a setting that this plan never declared",
		},
		{
			name: "a CustomCost naming another plan's count setting",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				other := New()
				count := other.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the technology steel-axes reads its research count from a setting that this plan never declared",
		},
		{
			name: "a CustomCost naming another plan's seconds setting",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				other := New()
				seconds := other.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the technology steel-axes reads its research time from a setting that this plan never declared",
		},
		{
			// The engine refuses a unit count of 0, and it RESETS an
			// out-of-range stored value to the default rather than clamping,
			// so the declared minimum is what makes every readable value legal.
			name: "a count setting with no minimum",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, NumericSpec{HasMax: true, Max: 100})
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-count backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
		},
		{
			name: "a count setting whose minimum is zero",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(0, 100))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-count backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
		},
		{
			name: "a seconds setting whose minimum is zero",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-seconds backs a research time but declares no minimum above 0 (the engine refuses a unit time of 0)",
		},
		{
			// ONE DROPDOWN COMPOSES ONE DESCRIPTION, so the presets it shows
			// can only be one declaration's. Two recipes arming it is two
			// preset lists for one string, and the player would read the
			// other recipe's.
			name: "two recipes arming one dropdown",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				head := l.Item("axe-head", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "custom"})
				axeParts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				headParts := l.IngredientsSetting("head-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
					Setting: style,
					Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
					Custom:  axeParts,
				}})
				l.Recipe(head, RecipeSpec{IngredientsBy: &IngredientChoices{
					Setting: style,
					Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
					Custom:  headParts,
				}})
			},
			want: "fkrecipes: the setting steelworks-style takes a Custom arm from more than one recipe; one dropdown composes one description",
		},
		{
			name: "two technologies arming one dropdown",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap", "custom"})
				axePacks := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				axeCount := l.IntSetting("axe-count", 20, Between(1, 100000))
				axeSeconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				sawPacks := l.PacksSetting("saw-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				sawCount := l.IntSetting("saw-count", 20, Between(1, 100000))
				sawSeconds := l.DoubleSetting("saw-seconds", 10, Between(0.5, 600))
				by := func(packs PacksSettingRef, count IntSettingRef, seconds DoubleSettingRef) *CostChoices {
					return &CostChoices{
						Setting:  tier,
						Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
						Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
						Custom: &CustomCost{
							Packs: packs, Count: count, Seconds: seconds,
							Position: []string{"logistics-2"},
						},
					}
				}
				l.Technology("steel-axes", TechSpec{CostBy: by(axePacks, axeCount, axeSeconds)})
				l.Technology("steel-saws", TechSpec{CostBy: by(sawPacks, sawCount, sawSeconds)})
			},
			want: "fkrecipes: the setting steelworks-tier takes a Custom arm from more than one technology; one dropdown composes one description",
		},
		{
			// A field the player can edit that changes nothing is a promise
			// the settings screen makes and the mod does not keep.
			name: "a text setting nothing reads",
			build: func(l *Lib) {
				l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
			},
			want: "fkrecipes: the setting axe-ingredients is declared and nothing reads it; a text setting must be bound to one recipe or technology",
		},
		{
			name: "a text setting two recipes read",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				head := l.Item("axe-head", ItemSpec{})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsFrom: parts})
				l.Recipe(head, RecipeSpec{IngredientsFrom: parts})
			},
			want: "fkrecipes: the setting axe-ingredients is read by more than one recipe or technology; a text setting serves exactly one",
		},
		{
			// THE SAME RULE FOR A COST ARM'S NUMBERS, and it is the ignored
			// line that needs it: the arm on a preset says the count is
			// ignored, and here the other technology is spending it. A player
			// reads "the number is ignored" about a field that priced the
			// research two declarations down.
			name: "a count setting two technologies read",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap", "custom"})
				fromPacks := l.PacksSetting("from-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				armPacks := l.PacksSetting("arm-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("shared-count", 20, Between(1, 100000))
				fromSeconds := l.DoubleSetting("from-seconds", 10, Between(0.5, 600))
				armSeconds := l.DoubleSetting("arm-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{
					Packs: fromPacks, Count: count, Seconds: fromSeconds,
				}})
				l.Technology("steel-saws", TechSpec{CostBy: &CostChoices{
					Setting:  tier,
					Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
					Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
					Custom: &CustomCost{
						Packs: armPacks, Count: count, Seconds: armSeconds,
						Position: []string{"logistics-2"},
					},
				}})
			},
			want: "fkrecipes: the setting shared-count is read as a research count or time by more than one declaration; a custom cost's number serves exactly one",
		},
		{
			// THE MIXED PAIR IS THE ONE THE PILOT COULD HAVE WRITTEN: one
			// double backing a recipe's crafting time and a research time at
			// once. The recipe reads it whatever the dropdown says, so the
			// ignored line would be false the moment the dropdown left custom.
			name: "a seconds setting a recipe and a cost read",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("shared-seconds", 10, Between(0.5, 600))
				l.Recipe(axe, RecipeSpec{
					CraftTimeFrom: seconds,
					Ingredients:   []Ingredient{IngredientNamed(1, "steel-plate")},
				})
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{
					Packs: packs, Count: count, Seconds: seconds,
				}})
			},
			want: "fkrecipes: the setting shared-seconds is read as a research count or time by more than one declaration; a custom cost's number serves exactly one",
		},
		{
			// BOTH EXACTLY-ONCE RULES BROKEN AT ONCE, and the text one
			// answers, because it is the pass in front. The order is the file
			// order and nothing else: a plan with two problems gets one
			// sentence, and this pins which.
			name: "a shared packs setting beside a shared count",
			build: func(l *Lib) {
				packs := l.PacksSetting("shared-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("shared-count", 20, Between(1, 100000))
				axeSeconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				sawSeconds := l.DoubleSetting("saw-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{
					Packs: packs, Count: count, Seconds: axeSeconds,
				}})
				l.Technology("steel-saws", TechSpec{CostFrom: &CustomCost{
					Packs: packs, Count: count, Seconds: sawSeconds,
				}})
			},
			want: "fkrecipes: the setting shared-packs is read by more than one recipe or technology; a text setting serves exactly one",
		},
		{
			// THE NUMBER RULE STEPS PAST WHAT THE OTHER SENTENCES OWN, and
			// this technology is the one that made it necessary: it names two
			// cost sources, so it has not said what its research costs, and
			// both of the arms it names read the same count. Counting them
			// would answer an undeclared cost with a sentence about sharing.
			name: "two cost sources over one count",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap", "custom"})
				fromPacks := l.PacksSetting("from-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				armPacks := l.PacksSetting("arm-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("shared-count", 20, Between(1, 100000))
				fromSeconds := l.DoubleSetting("from-seconds", 10, Between(0.5, 600))
				armSeconds := l.DoubleSetting("arm-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{
					CostFrom: &CustomCost{Packs: fromPacks, Count: count, Seconds: fromSeconds},
					CostBy: &CostChoices{
						Setting:  tier,
						Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
						Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
						Custom: &CustomCost{
							Packs: armPacks, Count: count, Seconds: armSeconds,
							Position: []string{"logistics-2"},
						},
					},
				})
			},
			want:     "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit, CostBy or CostFrom",
			dataOnly: true,
		},
		{
			// The recipe twin: a crafting time named twice, once by hand and
			// once by a handle a research cost also reads. "Pick one" is what
			// the author has to fix first, and it is the data planner's line.
			name: "CraftTime beside CraftTimeFrom over a shared seconds",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("shared-seconds", 10, Between(0.5, 600))
				l.Recipe(axe, RecipeSpec{
					CraftTime:     2,
					CraftTimeFrom: seconds,
					Ingredients:   []Ingredient{IngredientNamed(1, "steel-plate")},
				})
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{
					Packs: packs, Count: count, Seconds: seconds,
				}})
			},
			want:     "fkrecipes: the recipe steel-axe names both CraftTime and CraftTimeFrom; pick one",
			dataOnly: true,
		},
		{
			name: "a packs setting declaring no pack",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", nil)
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the packs setting axe-packs declares no science pack; research takes at least one",
		},
		{
			// The renderer turns a non-finite amount into text that does not
			// read back, and the settings planner renders DECLARED defaults,
			// so the check has to run before a single amount is formatted.
			name: "a declared default with a fluid amount that is not a number",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{
					FluidIngredient(nan(), "water"),
				})
				l.Recipe(axe, RecipeSpec{Category: "chemistry", IngredientsFrom: parts})
			},
			want: "fkrecipes: the ingredients setting axe-ingredients declares a fluid amount that is not a finite number",
		},
		{
			name: "a declared default with a fluid amount above the engine's ceiling",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{
					FluidIngredient(1e302, "water"),
				})
				l.Recipe(axe, RecipeSpec{Category: "chemistry", IngredientsFrom: parts})
			},
			want: "fkrecipes: the ingredients setting axe-ingredients takes the fluid water at an amount above 1e301, which the game cannot hold",
		},
		{
			// A declared fluid is judged against the category of the recipe
			// that READS the setting, because that is the only recipe whose
			// category the list can be wrong for.
			name: "a declared default with a fluid in a crafting recipe",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{FluidIngredient(10, "water")})
				l.Recipe(axe, RecipeSpec{IngredientsFrom: parts})
			},
			want: "fkrecipes: the ingredients setting axe-ingredients takes the fluid water, and a recipe in the crafting category takes items only",
		},
		{
			// The round trip is what makes the description a text the player
			// can copy back into the field, and it is what answers a TEXT
			// SETTING's declared duplicate: the declaration checks do not see
			// one and the language does. A plain list and a dropdown preset
			// have no round trip, so those are refused by validateNoDuplicates
			// with a sentence of their own (data_test.go); either way the
			// resolver's merge never meets a duplicate that was in the
			// declaration.
			name: "a declared default naming one thing twice",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{
					IngredientNamed(1, "steel-plate"),
					IngredientNamed(2, "steel-plate"),
				})
				l.Recipe(axe, RecipeSpec{IngredientsFrom: parts})
			},
			want: "fkrecipes: the ingredients setting axe-ingredients: entries 1 and 2 both name steel-plate",
		},
		{
			// The DECLARATION check answers now, before the rendering reaches
			// the language: one rule, and the sentence names the setting whose
			// list the author wrote rather than an entry in a text nobody
			// typed.
			name: "a declared default above the engine's item ceiling",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				parts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(70000, "steel-plate")})
				l.Recipe(axe, RecipeSpec{IngredientsFrom: parts})
			},
			want: "fkrecipes: the ingredients setting axe-ingredients takes 70000 of steel-plate, and an item amount goes up to 65535",
		},
		{
			// THE SAME CEILING ON A PLAIN LIST, which used to ship and be
			// refused by the engine, blaming the mod for a number its author
			// wrote. MEASURED: 65536 refuses the load, 65535 loads.
			name: "a plain ingredient list above the engine's item ceiling",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(65536, "steel-plate")}})
			},
			want:     "fkrecipes: the recipe steel-axe takes 65536 of steel-plate, and an item amount goes up to 65535",
			dataOnly: true,
		},
		{
			// This plan's OWN item has no candidate to name, so the sentence
			// takes the declared name and the handle is proved before it is
			// read.
			name: "a plain ingredient list above the ceiling naming this plan's own item",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				head := l.Item("axe-head", ItemSpec{})
				l.Recipe(axe, RecipeSpec{Ingredients: []Ingredient{IngredientOf(head, 65536)}})
			},
			want:     "fkrecipes: the recipe steel-axe takes 65536 of axe-head, and an item amount goes up to 65535",
			dataOnly: true,
		},
		{
			name: "CostFrom beside Unit",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
				l.Technology("steel-axes", TechSpec{
					CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
					Unit:     &UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
				})
			},
			// The data planner's own sentence, because "pick one" is what an
			// author reads best; the binding walk steps past a declaration
			// that names two costs rather than answering in front of it.
			want:     "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit, CostBy or CostFrom",
			dataOnly: true,
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanData(customWorld())
			if err == nil {
				t.Fatalf("the data plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("data plan\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
		// THE SETTINGS PLAN REFUSES THE SAME THINGS, except the ones the data
		// planner's own declaration loops own: it renders these defaults and
		// composes these descriptions, so a plan it accepted and the data
		// stage refused would be a settings screen for a mod that cannot load.
		if c.dataOnly {
			continue
		}
		t.Run(c.name+" at the settings stage", func(t *testing.T) {
			lib := New()
			c.build(lib)
			ops, err := lib.PlanSettings(settingsWorld())
			if err == nil {
				t.Fatalf("the settings plan was accepted, want refusal %q", c.want)
			}
			if err.Error() != c.want {
				t.Errorf("settings plan\n got: %s\nwant: %s", err.Error(), c.want)
			}
			if ops != nil {
				t.Errorf("a refused plan still produced %d ops", len(ops))
			}
		})
	}
}

func nan() float64 {
	zero := 0.0
	return zero / zero
}

// ---------------------------------------------------------------------------
// The data stage: the three text paths.
// ---------------------------------------------------------------------------

// A plan whose one recipe reads its ingredients from a text setting, so each
// path below is one line of world setup.
func rivetPlan() (*Lib, IngredientsSettingRef) {
	lib := New()
	rivet := lib.Item("steel-rivet", ItemSpec{})
	parts := lib.IngredientsSetting("rivet-ingredients", []Ingredient{
		IngredientNamed(2, "tungsten-plate", "steel-plate"),
		IngredientNamed(1, "iron-stick", "iron-plate"),
	})
	lib.Recipe(rivet, RecipeSpec{Name: "steel-rivet-forging", IngredientsFrom: parts})
	return lib, parts
}

// THE WORD IS THE AUTHOR'S LIST, ladders and all, and it says nothing in the
// log: the player did not choose anything, so there is nothing to report.
// TWO RECIPES MAY STILL SHARE ONE CRAFTING TIME, and this is the witness that
// the rule above did not quietly take that away. Nothing ever tells a player a
// crafting time was ignored, so no line about one can lie, and a plan that
// prices two recipes off one slider is a plan somebody meant to write. The
// refusal is about a number a cost arm reads, and neither of these is one.
func TestTwoRecipesMayShareOneCraftingTime(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	head := lib.Item("axe-head", ItemSpec{})
	forging := lib.DoubleSetting("forging-time", 3, NumericSpec{HasMax: true, Max: 120})
	lib.Recipe(axe, RecipeSpec{
		CraftTimeFrom: forging,
		Ingredients:   []Ingredient{IngredientNamed(1, "steel-plate")},
	})
	lib.Recipe(head, RecipeSpec{
		CraftTimeFrom: forging,
		Ingredients:   []Ingredient{IngredientNamed(1, "steel-plate")},
	})

	if _, err := lib.PlanSettings(settingsWorld()); err != nil {
		t.Fatalf("the settings plan was refused: %s", err)
	}
	ops, err := lib.PlanData(customWorld().withSetting("steelworks-forging-time", Num(4)))
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="item", name="steelworks-axe-head", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", energy_required=4, enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="recipe", name="steelworks-axe-head", energy_required=4, enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-axe-head", amount=1}]}`,
	})
}

func TestTextSettingOnTheDefaultWord(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withSetting("steelworks-rivet-ingredients", Str("  default  "))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}, {type="item", name="iron-stick", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
}

// The ladder really is walked on the default path, drop line and all: the word
// selects the pre-existing resolution, not a frozen rendering of it.
func TestTextSettingOnTheDefaultWordDropsAnAbsentLadder(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withoutItem("iron-stick").withoutItem("iron-plate").
		withSetting("steelworks-rivet-ingredients", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-rivet-forging: none of iron-stick, iron-plate is present, so the ingredient is dropped`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
}

// An edited text is taken as written, in the order it was TYPED, and one line
// records the canonical form so a player who wrote "iron-plate x2" learns what
// the library would have written.
func TestTextSettingOnAnEditedText(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withFluid("lubricant").
		withSetting("steelworks-rivet-ingredients", Str("iron-plate x2, 3 [item=steel-plate], 0.5 [fluid=lubricant]"))
	// The recipe's category is what decides whether a typed fluid is legal, so
	// the plan is rebuilt with one that takes fluids.
	lib = New()
	rivet := lib.Item("steel-rivet", ItemSpec{})
	parts := lib.IngredientsSetting("rivet-ingredients", []Ingredient{IngredientNamed(1, "iron-plate")})
	lib.Recipe(rivet, RecipeSpec{Name: "steel-rivet-forging", Category: "chemistry", IngredientsFrom: parts})

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-steel-rivet-forging takes its ingredients from steelworks-rivet-ingredients: 2 iron-plate, 3 steel-plate, 0.5 [fluid=lubricant]`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="iron-plate", amount=2}, {type="item", name="steel-plate", amount=3},` +
			` {type="fluid", name="lubricant", amount=5.0000000000000000e-1}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
}

// An unreadable setting takes the declared default and says so, in the same
// sentence every other unreadable setting in this library gets.
func TestTextSettingThatIsNotReadable(t *testing.T) {
	lib, _ := rivetPlan()

	ops, err := lib.PlanData(customWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-rivet-ingredients was not readable, so its default applies`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}, {type="item", name="iron-stick", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
}

// A readable value that is not a string is REFUSED rather than degraded. The
// engine resets a wrong-typed stored value before any stage runs (measured), so
// this is a hand-edited file and a silent default would hide it.
func TestTextSettingThatIsNotText(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withSetting("steelworks-rivet-ingredients", Num(3))

	_, err := lib.PlanData(w)
	assertRefusal(t, err, "fkrecipes: steelworks-rivet-ingredients is not text")
}

// The language's own refusal is raised VERBATIM: the sentence the corpus pins
// is the sentence the player reads, with no stage and no second prefix.
func TestTextSettingRaisesTheLanguageRefusalVerbatim(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withSetting("steelworks-rivet-ingredients", Str("2 iron-plate, 1 unobtanium"))

	_, err := lib.PlanData(w)
	assertRefusal(t, err,
		`fkrecipes: steelworks-rivet-ingredients, entry 2 ("1 unobtanium"): no item or fluid is named unobtanium`)
}

// A TEXT MAY NAME THIS PLAN'S OWN ITEMS, and it has to: the setting's composed
// description shows the player exactly those prefixed names, so a text copied
// out of it must resolve.
//
// MEASURED (Factorio 2.0.77): "1 fkrecipes-example-steel-rivet, 10 water"
// refused with "no item or fluid is named fkrecipes-example-steel-rivet",
// because the data planner resolves every text BEFORE it extends data.raw with
// its own items. The overlay World is what closes it, and the order emitted is
// the order TYPED.
func TestTextSettingNamesThePlansOwnItem(t *testing.T) {
	lib, _ := ownItemPlan()
	w := customWorld().withSetting("steelworks-axe-ingredients",
		Str("3 steelworks-steel-rivet, 1 steel-plate"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-steel-axe-forging takes its ingredients from steelworks-axe-ingredients:` +
			` 3 steelworks-steel-rivet, 1 steel-plate`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", enabled=true,` +
			` ingredients=[{type="item", name="steelworks-steel-rivet", amount=3}, {type="item", name="steel-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// The one guess the language makes runs against the overlay too, so a player
// who typed the description's name with the shift key held is told the name
// that is really there.
func TestTextSettingSuggestsThePlansOwnItem(t *testing.T) {
	lib, _ := ownItemPlan()
	w := customWorld().withSetting("steelworks-axe-ingredients", Str("3 Steelworks_Steel_Rivet"))

	_, err := lib.PlanData(w)
	assertRefusal(t, err, `fkrecipes: steelworks-axe-ingredients, entry 1 ("3 Steelworks_Steel_Rivet"):`+
		` no item or fluid is named Steelworks_Steel_Rivet; did you mean steelworks-steel-rivet`)
}

// A PACK LIST IS NOT WIDENED BY THE OVERLAY. This plan's own item is an item
// and never a tool, so a pack text naming one is told exactly that rather than
// being told the name does not exist.
func TestPackTextNamingAPlanItemIsToldItIsAnItem(t *testing.T) {
	lib := New()
	lib.Item("steel-rivet", ItemSpec{})
	packs := lib.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("axe-count", 20, Between(1, 100000))
	seconds := lib.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
	lib.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
	w := customWorld().withSetting("steelworks-axe-packs", Str("1 steelworks-steel-rivet"))

	_, err := lib.PlanData(w)
	assertRefusal(t, err, `fkrecipes: steelworks-axe-packs, entry 1 ("1 steelworks-steel-rivet"):`+
		` steelworks-steel-rivet is an item, not a science pack`)
}

// THE DECLARED PATH IS UNCHANGED BY THE OVERLAY. The word default takes the
// author's list, ladders and all, and a ladder is still walked against the game
// rather than against this plan.
func TestOwnItemOverlayLeavesTheDeclaredPathAlone(t *testing.T) {
	lib, _ := ownItemPlan()
	w := customWorld().withoutItem("iron-plate").
		withSetting("steelworks-axe-ingredients", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steel-axe-forging: none of iron-plate is present, so the ingredient is dropped`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", enabled=true,` +
			` ingredients=[{type="item", name="steelworks-steel-rivet", amount=2}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// A plan whose text setting can name an item the plan itself declares: the
// rivet is this plan's, and the declared default is written with its handle
// exactly as the description renders it.
func ownItemPlan() (*Lib, IngredientsSettingRef) {
	lib := New()
	rivet := lib.Item("steel-rivet", ItemSpec{})
	axe := lib.Item("steel-axe", ItemSpec{})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{
		IngredientOf(rivet, 2),
		IngredientNamed(1, "iron-plate"),
	})
	lib.Recipe(axe, RecipeSpec{Name: "steel-axe-forging", IngredientsFrom: parts})
	return lib, parts
}

// ---------------------------------------------------------------------------
// The data stage: a dropdown with a Custom arm.
// ---------------------------------------------------------------------------

func customArmPlan() *Lib {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil", "custom"})
	quench := lib.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	lib.Recipe(plate, RecipeSpec{
		Name:     "plate-quenching",
		Category: "chemistry",
		IngredientsBy: &IngredientChoices{
			Setting: medium,
			Choices: []IngredientChoice{
				{Value: "water", Ingredients: []Ingredient{IngredientNamed(1, "iron-plate")}},
				{Value: "oil", Ingredients: []Ingredient{IngredientNamed(1, "copper-plate")}},
			},
			Custom: quench,
		},
	})
	return lib
}

// On a preset the preset applies, exactly as it did before the arm existed.
func TestCustomArmOnAPreset(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().withSetting("steelworks-quench-medium", Str("oil"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	// AND NOT ONE WORD ABOUT THE TEXT SETTING, which is unreadable here. It is
	// not being read for real, so an unreadable line about it would be noise
	// about a field nothing consulted.
	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// WITHOUT THIS LINE THE PLAYER EDITS A FIELD AND NOTHING HAPPENS. It comes
// before the preset's own lines, so it reads as the reason they are the
// preset's and not the text's.
func TestCustomArmSaysWhenAnEditedTextIsIgnored(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("oil")).
		withSetting("steelworks-quench-ingredients", Str("4 iron-plate"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-quench-ingredients is edited, but steelworks-quench-medium is not on custom, so the text is ignored`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// A text left on the word is not an edit, so nothing is said about it.
func TestCustomArmIsQuietWhenTheTextIsOnTheWord(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("water")).
		withSetting("steelworks-quench-ingredients", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="iron-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// AND THE LANGUAGE IS WHAT DECIDES WHAT THE WORD IS. "default," is a
// tolerated trailing comma the language reads as the marker, so the field is
// untouched and nothing is said about it. Comparing the trimmed text with the
// bare word would have told the player their untouched field was ignored.
func TestCustomArmIsQuietWhenTheTextIsTheWordWithATrailingComma(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("oil")).
		withSetting("steelworks-quench-ingredients", Str("default,"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// On the custom value the text is what the recipe is made of.
func TestCustomArmOnTheCustomValue(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("custom")).
		withSetting("steelworks-quench-ingredients", Str("3 copper-plate, 2 water"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-plate-quenching takes its ingredients from steelworks-quench-ingredients: 3 copper-plate, 2 [fluid=water]`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=3}, {type="fluid", name="water", amount=2}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// On the custom value with the word in the field, the AUTHOR's declared list
// applies: the arm is a switch, and the word means the same thing under it.
func TestCustomArmOnTheCustomValueWithTheWord(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("custom")).
		withSetting("steelworks-quench-ingredients", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// A CustomValue of the mod's own choosing selects the same way, which is what
// lets a mod that already ships a preset named custom take an arm without
// renaming it and discarding every stored preference.
func TestCustomArmUnderItsOwnValue(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "custom", []string{"custom", "mine"})
	quench := lib.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	lib.Recipe(plate, RecipeSpec{
		Name: "plate-quenching",
		IngredientsBy: &IngredientChoices{
			Setting:     medium,
			Choices:     []IngredientChoice{{Value: "custom", Ingredients: []Ingredient{IngredientNamed(1, "iron-plate")}}},
			CustomValue: "mine",
			Custom:      quench,
		},
	})
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("mine")).
		withSetting("steelworks-quench-ingredients", Str("7 copper-plate"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-plate-quenching takes its ingredients from steelworks-quench-ingredients: 7 copper-plate`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=7}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// A DROPDOWN WHOSE CHOICES ALREADY COVER custom KEEPS IT AS AN ORDINARY
// PRESET. The refusal beside it is about a value with nothing behind it, and a
// mod that already ships one named custom must not be made to rename it:
// Factorio keys a stored choice by its value, so a rename discards what every
// player picked.
func TestDropdownWithAPresetNamedCustomNeedsNoArm(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	style := lib.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "custom"})
	lib.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
		Setting: style,
		Choices: []IngredientChoice{
			{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}},
			{Value: "custom", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
		},
	}})
	w := customWorld().withSetting("steelworks-style", Str("custom"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// The cost twin of the rule above: the preset is a ladder like any other.
func TestCostDropdownWithAPresetNamedCustomNeedsNoArm(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap", "custom"})
	lib.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
		Setting: tier,
		Choices: []CostChoice{
			{Value: "cheap", Sources: []string{"logistics-2"}},
			{Value: "custom", Sources: []string{"logistics-2"}},
		},
		Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
	}})
	w := customWorld().withSetting("steelworks-tier", Str("custom"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-steel-axes", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// THE PILOT'S OWN DEFECT, closed. A stored value the dropdown does not offer
// used to find no plan at all: the recipe came out made of nothing and no line
// said so. It is unreachable through the engine, which resets such a value
// before any stage runs, and reachable through a hand-edited file.
func TestDropdownHoldingAValueItDoesNotOffer(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().withSetting("steelworks-quench-medium", Str("brine"))

	_, err := lib.PlanData(w)
	assertRefusal(t, err, `fkrecipes: steelworks-quench-medium holds "brine", which is not one of its values`)
}

// The same rule on a cost dropdown, where the old answer was the fallback cost
// applying under a line naming a value the setting never offered.
func TestCostDropdownHoldingAValueItDoesNotOffer(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap"})
	lib.Technology("steel-axes", TechSpec{CostBy: &CostChoices{
		Setting:  tier,
		Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
		Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
	}})
	w := customWorld().withSetting("steelworks-tier", Str("dear"))

	_, err := lib.PlanData(w)
	assertRefusal(t, err, `fkrecipes: steelworks-tier holds "dear", which is not one of its values`)
}

// ---------------------------------------------------------------------------
// The data stage: a research cost the player writes.
// ---------------------------------------------------------------------------

func chainPlan() *Lib {
	lib := New()
	packs := lib.PacksSetting("chain-packs", []Pack{
		{Name: "automation-science-pack", Amount: 1},
		{Name: "military-science-pack", Amount: 2},
	})
	count := lib.IntSetting("chain-count", 20, Between(1, 100000))
	seconds := lib.DoubleSetting("chain-seconds", 10, Between(0.5, 600))
	lib.Technology("chain-forging", TechSpec{
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		After:    "steel-processing",
	})
	return lib
}

// The unit is the engine's SHORT TUPLE form, the count and the seconds always
// come from their settings, and one line records all three.
func TestCustomResearchCostFromAnEditedText(t *testing.T) {
	lib := chainPlan()
	w := customWorld().
		withSetting("steelworks-chain-count", Num(25)).
		withSetting("steelworks-chain-seconds", Num(12.5)).
		withSetting("steelworks-chain-packs", Str("1 automation-science-pack, 1 logistic-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 25, time 12.5, packs 1 automation-science-pack, 1 logistic-science-pack`,
		`extend {type="technology", name="steelworks-chain-forging", prerequisites=["steel-processing"],` +
			` unit={count=25, time=1.2500000000000000e1, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]]}}`,
	})
}

// On the word, the DECLARED packs apply with their ladders, and the numbers
// still come from their own settings: the two are separate fields and a player
// may have moved either.
func TestCustomResearchCostOnTheDefaultWord(t *testing.T) {
	lib := chainPlan()
	w := customWorld().withSetting("steelworks-chain-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-chain-count was not readable, so its default applies`,
		`log fkrecipes: the setting steelworks-chain-seconds was not readable, so its default applies`,
		`log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 20, time 10, packs 1 automation-science-pack, 2 military-science-pack`,
		`extend {type="technology", name="steelworks-chain-forging", prerequisites=["steel-processing"],` +
			` unit={count=20, time=10, ingredients=[["automation-science-pack", 1], ["military-science-pack", 2]]}}`,
	})
}

// A DECLARED pack whose ladder finds nothing is DROPPED with a line, wherever
// the unit is built: the word default is not an exception to that rule.
func TestCustomResearchCostDropsAnAbsentDeclaredPack(t *testing.T) {
	lib := chainPlan()
	w := customWorld().withoutTool("military-science-pack").
		withSetting("steelworks-chain-count", Num(20)).
		withSetting("steelworks-chain-seconds", Num(10)).
		withSetting("steelworks-chain-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: chain-forging: none of military-science-pack is present, so the science pack is dropped`,
		`log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 20, time 10, packs 1 automation-science-pack`,
		`extend {type="technology", name="steelworks-chain-forging", prerequisites=["steel-processing"],` +
			` unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// Every declared pack dropping is the same refusal a hand-rolled unit gets: a
// research with no packs is a free one, and the engine loads it.
func TestCustomResearchCostRefusesWhenEveryDeclaredPackDrops(t *testing.T) {
	lib := chainPlan()
	w := customWorld().withoutTool("military-science-pack").withoutTool("automation-science-pack").
		withSetting("steelworks-chain-packs", Str("default"))

	_, err := lib.PlanData(w)
	assertRefusal(t, err,
		"fkrecipes: the technology chain-forging has no science pack the game has; research takes at least one")
}

// A NUMBER A World CAN ANSWER AND THE ENGINE CANNOT TAKE. The declared minima
// and the engine's own reset rule keep a player from producing one of these,
// and a World is an interface: a NaN used to reach the amount formatter and
// trap, and an infinity used to be rendered into the unit. The sentence names
// the SETTING, because that is the field somebody would go and fix.
func TestCustomResearchCostRefusesANumberTheEngineWouldNotTake(t *testing.T) {
	cases := []struct {
		name  string
		count Value
		time  Value
		want  string
	}{
		{
			name:  "a count that is not a finite number",
			count: Num(nan()),
			time:  Num(10),
			want:  "fkrecipes: steelworks-chain-count holds a value that is not a finite number",
		},
		{
			name:  "a time that is not a finite number",
			count: Num(20),
			time:  Num(math.Inf(1)),
			want:  "fkrecipes: steelworks-chain-seconds holds a value that is not a finite number",
		},
		{
			// Finiteness is asked first, so this arm is only ever reached by a
			// real number: a NaN is not below 1 and would have gone through.
			name:  "a count below 1",
			count: Num(0),
			time:  Num(10),
			want:  "fkrecipes: steelworks-chain-count holds a research count below 1",
		},
		{
			name:  "a time at or below zero",
			count: Num(20),
			time:  Num(0),
			want:  "fkrecipes: steelworks-chain-seconds holds a research time at or below zero",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			w := customWorld().
				withSetting("steelworks-chain-count", c.count).
				withSetting("steelworks-chain-seconds", c.time).
				withSetting("steelworks-chain-packs", Str("1 automation-science-pack"))

			_, err := chainPlan().PlanData(w)
			assertRefusal(t, err, c.want)
		})
	}
}

// CostFrom is placed by the ORDINARY placement fields, and they degrade the
// ordinary way: the anchor is probed and a missing one is dropped with a line.
func TestCustomResearchCostPlacementDegrades(t *testing.T) {
	lib := chainPlan()
	w := customWorld().withoutTech("steel-processing").
		withSetting("steelworks-chain-count", Num(20)).
		withSetting("steelworks-chain-seconds", Num(10)).
		withSetting("steelworks-chain-packs", Str("1 automation-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 20, time 10, packs 1 automation-science-pack`,
		`log fkrecipes: chain-forging: steel-processing is absent, so the prerequisite is dropped`,
		`extend {type="technology", name="steelworks-chain-forging",` +
			` unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// A cost dropdown's Custom arm carries its own Position ladder, because the
// preset it replaces would have brought a source technology to hang off.
func tipsPlan() *Lib {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-tier", "cheap", []string{"cheap", "custom"})
	packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tips-count", 30, Between(1, 100000))
	seconds := lib.DoubleSetting("tips-seconds", 15, Between(0.5, 600))
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting:  tier,
			Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
			Custom: &CustomCost{
				Packs: packs, Count: count, Seconds: seconds,
				Position: []string{"tungsten-hardening", "logistics-3"},
			},
		},
	})
	return lib
}

// ON THE CUSTOM VALUE NOTHING IS IGNORED AND NOTHING SAYS SO. All three fields
// are moved here and all three are live, so the whole transcript is the one
// research line and the technology; this is the witness that the ignored lines
// belong to the preset side only.
func TestCustomCostArmWalksItsPositionLadder(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("custom")).
		withSetting("steelworks-tips-count", Num(40)).
		withSetting("steelworks-tips-seconds", Num(20)).
		withSetting("steelworks-tips-packs", Str("2 logistic-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 40, time 20, packs 2 logistic-science-pack`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-3"],` +
			` unit={count=40, time=20, ingredients=[["logistic-science-pack", 2]]}}`,
	})
}

// No rung present leaves the technology unattached and says so, in the shape
// every other dropped ladder in this library uses.
func TestCustomCostArmWithNoPositionRungPresent(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().withoutTech("logistics-3").
		withSetting("steelworks-tips-tier", Str("custom")).
		withSetting("steelworks-tips-count", Num(40)).
		withSetting("steelworks-tips-seconds", Num(20)).
		withSetting("steelworks-tips-packs", Str("2 logistic-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs: count 40, time 20, packs 2 logistic-science-pack`,
		`log fkrecipes: hardened-tips: none of tungsten-hardening, logistics-3 is present, so the technology has no prerequisite`,
		`extend {type="technology", name="steelworks-hardened-tips",` +
			` unit={count=40, time=20, ingredients=[["logistic-science-pack", 2]]}}`,
	})
}

// On a preset the arm is not live, and an edited pack text says so rather than
// disappearing. The count and the seconds are unread here, so they say nothing:
// an unreadable value is not an edit.
func TestCustomCostArmOnAPresetIgnoresTheText(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-packs", Str("2 logistic-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-tips-packs is edited, but steelworks-tips-tier is not on custom, so the text is ignored`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// AND THE TWO NUMBERS BESIDE IT SAY THE SAME THING. The pilot moved a count
// under a tier, watched the technology keep the preset's price, and read
// nothing about it: the pack text drew a line and the count drew none, which
// is the field-edited-nothing-happens shape the text line exists to close,
// left open on two thirds of the arm.
func TestCustomCostArmOnAPresetIgnoresAnEditedCount(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(45))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-tips-count is edited, but steelworks-tips-tier is not on custom, so the number is ignored`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// The seconds are the count's twin and get the twin sentence, under its own
// setting's name, because that is the field somebody would go and put back.
func TestCustomCostArmOnAPresetIgnoresEditedSeconds(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-seconds", Num(22))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-tips-seconds is edited, but steelworks-tips-tier is not on custom, so the number is ignored`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// ONE LINE PER EDITED FIELD, IN THE ORDER THE UNIT READS THEM: count, seconds,
// packs. A player who priced the whole thing and left the dropdown alone reads
// all three fields back, in the order the log line on the custom side prints
// them, and reads them before the preset's own lines.
func TestCustomCostArmOnAPresetIgnoresAllThreeInOrder(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(45)).
		withSetting("steelworks-tips-seconds", Num(22)).
		withSetting("steelworks-tips-packs", Str("2 logistic-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-tips-count is edited, but steelworks-tips-tier is not on custom, so the number is ignored`,
		`log fkrecipes: steelworks-tips-seconds is edited, but steelworks-tips-tier is not on custom, so the number is ignored`,
		`log fkrecipes: steelworks-tips-packs is edited, but steelworks-tips-tier is not on custom, so the text is ignored`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// A NUMBER STANDING ON ITS DECLARED DEFAULT IS NOT AN EDIT, and this is the
// case that makes the comparison load-bearing rather than the readability
// check: the engine stores every setting's value including the ones nobody
// touched, so a silent player answers both of these and must read nothing.
func TestCustomCostArmIsQuietWhenTheNumbersAreOnTheirDefaults(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(30)).
		withSetting("steelworks-tips-seconds", Num(15))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// AND THE IGNORED LINES COME BEFORE THE PRESET'S OWN, so they read as the
// reason the lines under them are the preset's and not the player's. The chosen
// ladder finds nothing here, so the preset has a line of its own to sit under.
func TestCustomCostArmIgnoredLinesComeBeforeThePresetsOwn(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().withoutTech("logistics-2").
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(45))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-tips-count is edited, but steelworks-tips-tier is not on custom, so the number is ignored`,
		`log fkrecipes: hardened-tips: no source for the cheap cost carries a unit, so the fallback cost applies and the technology has no prerequisite`,
		`extend {type="technology", name="steelworks-hardened-tips", unit={count=200, time=30, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// A value that is not a number is not an edit either, and neither is one that
// is not there: the same tolerance the text line has, for the same reason. The
// count here is answered as a string and the seconds are not answered at all,
// and the transcript says nothing about either.
func TestCustomCostArmIsQuietWhenANumberIsNotOne(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Str("45"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// A VALUE THE WORLD ANSWERS BESIDE ok=false IS NOT AN EDIT EITHER, for all
// three fields at once. Each of these would be an edit if the flag said the
// setting was there: 45 is not the declared 30, 22 is not the declared 15, and
// the pack text is not the word. The World says none of them is there, and the
// contract is that the flag wins over whatever value rides along, so the
// transcript is the preset's line and nothing else. This is the witness for
// the !ok term of noteIgnoredNumber and of noteIgnoredText.
func TestCustomCostArmOnAPresetIsQuietWhenTheWorldAnswersBesideAbsent(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSettingButAbsent("steelworks-tips-count", Num(45)).
		withSettingButAbsent("steelworks-tips-seconds", Num(22)).
		withSettingButAbsent("steelworks-tips-packs", Str("2 logistic-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// A STORED NaN IS AN EDIT UNDER A PRESET AND A REFUSAL ON CUSTOM, and the two
// halves are deliberate rather than an accident of NaN comparing false against
// everything. A stored value that is not the default is something the player
// did, so under a preset it draws the line naming the field they can go and
// put back; on the custom value the same number is read for real, and it is
// refused by the name of the setting holding it.
func TestCustomCostArmNaNIsAnEditUnderAPresetAndARefusalOnCustom(t *testing.T) {
	preset := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(math.NaN()))

	ops, err := tipsPlan().PlanData(preset)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-tips-count is edited, but steelworks-tips-tier is not on custom, so the number is ignored`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})

	custom := customWorld().
		withSetting("steelworks-tips-tier", Str("custom")).
		withSetting("steelworks-tips-count", Num(math.NaN())).
		withSetting("steelworks-tips-seconds", Num(20)).
		withSetting("steelworks-tips-packs", Str("1 automation-science-pack"))

	_, err = tipsPlan().PlanData(custom)
	assertRefusal(t, err, "fkrecipes: steelworks-tips-count holds a value that is not a finite number")
}

// ON THE CUSTOM ARM THE PACK TEXT ANSWERS BEFORE THE TWO NUMBERS. The count
// and the seconds here are both below what the engine takes and the text names
// a pack no game has, and the text's refusal is the one raised: the numbers are
// checked after the pack text resolves, so the player is answered about the
// field they typed into rather than about two sliders they may never have
// moved.
//
// AND BEHIND THE TEXT, FINITENESS ANSWERS BEFORE EITHER FLOOR, both numbers
// before both floors. A count of 0 is merely too small and a seconds of NaN is
// a value no arithmetic can use, so the seconds are what the player is told
// about; the floors are the questions a finite number can be asked, and asking
// one of them first would answer a world that holds a NaN by the field beside
// it. The Rust half pins the same world in its own pack text test; this is that
// world written here, so the sub ordering cannot drift between the halves.
func TestCustomCostArmRefusesTheTextBeforeTheNumbers(t *testing.T) {
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("custom")).
		withSetting("steelworks-tips-count", Num(0)).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("2 unobtainium"))

	_, err := tipsPlan().PlanData(w)
	assertRefusal(t, err, `fkrecipes: steelworks-tips-packs, entry 1 ("2 unobtainium"): no science pack is named unobtainium`)

	readable := customWorld().
		withSetting("steelworks-tips-tier", Str("custom")).
		withSetting("steelworks-tips-count", Num(0)).
		withSetting("steelworks-tips-seconds", Num(math.NaN())).
		withSetting("steelworks-tips-packs", Str("1 automation-science-pack"))

	_, err = tipsPlan().PlanData(readable)
	assertRefusal(t, err, "fkrecipes: steelworks-tips-seconds holds a value that is not a finite number")
}

// A custom research cost is a price this plan wrote, so it carries no level cap
// of its own: max_level is a field a COPY brings across from its source.
func TestCustomResearchCostCarriesNoMaxLevel(t *testing.T) {
	lib := chainPlan()
	w := customWorld().answeringMaxLevelForAnyName(Str("infinite")).
		withSetting("steelworks-chain-count", Num(20)).
		withSetting("steelworks-chain-seconds", Num(10)).
		withSetting("steelworks-chain-packs", Str("1 automation-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	for _, op := range ops {
		if op.Kind != OpExtend {
			continue
		}
		if _, ok := field(op.Proto, "max_level"); ok {
			t.Errorf("a custom research cost carried a max_level: %s", renderValue(op.Proto))
		}
	}
}

// ---------------------------------------------------------------------------
// The locale checker.
// ---------------------------------------------------------------------------

// A DESCRIPTION IS OPTIONAL EVERYWHERE ELSE and required here, because it is
// where the player learns the format. A free-text field with no explanation is
// a field nobody can fill in.
func TestCheckLocaleRequiresATextSettingDescription(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	lib.Recipe(axe, RecipeSpec{IngredientsFrom: parts})

	cfg := "[mod-setting-name]\nsteelworks-axe-ingredients=What an axe is made of\n"
	assertFindings(t, lib.CheckLocale("steelworks", cfg), []string{
		"the setting steelworks-axe-ingredients has no [mod-setting-description] entry, and a text setting needs one to tell the player the format",
	})

	full := cfg + "\n[mod-setting-description]\nsteelworks-axe-ingredients=Amount, then name, commas between.\n"
	assertFindings(t, lib.CheckLocale("steelworks", full), nil)
}

// A dropdown with a Custom arm has its preset list composed onto its
// description, so an absent one loses the list as well as the tooltip. Its
// custom value needs its own [string-mod-setting] entry under the rule every
// other value already has.
func TestCheckLocaleRequiresACustomArmDropdownDescription(t *testing.T) {
	lib := customArmPlan()

	cfg := `[mod-setting-name]
steelworks-quench-medium=Quenching medium
steelworks-quench-ingredients=Custom quench

[mod-setting-description]
steelworks-quench-ingredients=Amount, then name, commas between.

[string-mod-setting]
steelworks-quench-medium-water=Water
steelworks-quench-medium-oil=Oil
`
	assertFindings(t, lib.CheckLocale("steelworks", cfg), []string{
		"the dropdown setting steelworks-quench-medium has no [mod-setting-description] entry, which the custom arm composes its preset list onto",
		"the dropdown setting steelworks-quench-medium has no [string-mod-setting] entry for its value custom",
	})
}

// A dropdown WITHOUT an arm keeps the old rule: its description stays optional,
// because a missing one there costs a tooltip and nothing else.
func TestCheckLocaleLeavesAPlainDropdownDescriptionOptional(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
	lib.Recipe(plate, RecipeSpec{IngredientsBy: &IngredientChoices{
		Setting: medium,
		Choices: []IngredientChoice{
			{Value: "water", Ingredients: []Ingredient{IngredientNamed(1, "iron-plate")}},
			{Value: "oil", Ingredients: []Ingredient{IngredientNamed(1, "copper-plate")}},
		},
	}})

	cfg := `[mod-setting-name]
steelworks-quench-medium=Quenching medium

[string-mod-setting]
steelworks-quench-medium-water=Water
steelworks-quench-medium-oil=Oil
`
	assertFindings(t, lib.CheckLocale("steelworks", cfg), nil)
}

// ---------------------------------------------------------------------------

// The binding walk STEPS PAST a recipe that names two ingredient sources, so
// the settings planner may not render that recipe's presets: their handles were
// never validated. It says nothing about such a plan instead, and the data
// planner refuses it by name.
func TestSettingsPlanDoesNotRenderAnUnvalidatedChoice(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	other := New()
	other.Item("first-stranger", ItemSpec{})
	// The SECOND item of the other plan, so its index is past the end of this
	// one: an index that merely pointed at the wrong item would render a wrong
	// name rather than reaching for one that is not there, and this test would
	// pass over a guard that does nothing.
	stranger := other.Item("second-stranger", ItemSpec{})
	style := lib.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "custom"})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		IngredientsBy: &IngredientChoices{
			Setting: style,
			Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientOf(stranger, 1)}}},
			Custom:  parts,
		},
	})

	if _, err := lib.PlanSettings(settingsWorld()); err != nil {
		t.Errorf("the settings plan refused instead of staying quiet: %v", err)
	}
	_, err := lib.PlanData(customWorld())
	assertRefusal(t, err, "fkrecipes: the recipe steel-axe names both Ingredients and IngredientsBy; pick one")
}

// THE THREE REFUSAL CHANNELS IN ORDER, over one plan that has all three
// problems at once. The language's own refusal is first, because a text the
// player typed is the thing they can act on and it was found earliest in the
// walk; then the crafting-time floor, which is about a number the mod is bound
// to; then the science packs this game does not have, which is about the
// modpack. Each case repairs the one above it, so the next channel is the one
// that answers.
func TestRefusalChannelOrder(t *testing.T) {
	plan := func() *Lib {
		lib := New()
		axe := lib.Item("steel-axe", ItemSpec{})
		parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
		forging := lib.DoubleSetting("forging-time", 3, NumericSpec{HasMax: true, Max: 120})
		lib.Recipe(axe, RecipeSpec{
			Name:            "steel-axe-forging",
			CraftTimeFrom:   forging,
			IngredientsFrom: parts,
		})
		lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
			Count:   1,
			Seconds: 1,
			Packs:   []Pack{{Name: "military-science-pack", Amount: 1}},
		}})
		return lib
	}

	cases := []struct {
		name string
		text string
		time float64
		want string
	}{
		{
			name: "the language answers first",
			text: "2 unobtanium",
			time: 0,
			want: `fkrecipes: steelworks-axe-ingredients, entry 1 ("2 unobtanium"): no item or fluid is named unobtanium`,
		},
		{
			name: "then the crafting time",
			text: "2 steel-plate",
			time: 0,
			want: "fkrecipes: the recipe steel-axe-forging reads its crafting time from steelworks-forging-time," +
				" which answers at or below the engine floor (energy_required can't be <= 0.001)",
		},
		{
			name: "then the packs the game does not have",
			text: "2 steel-plate",
			time: 2,
			want: "fkrecipes: the technology steel-axes has no science pack the game has; research takes at least one",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			// The pack is absent in every case, so the third channel is armed
			// throughout and only the two in front of it are repaired.
			w := customWorld().withoutTool("military-science-pack").
				withSetting("steelworks-axe-ingredients", Str(c.text)).
				withSetting("steelworks-forging-time", Num(c.time))

			_, err := plan().PlanData(w)
			assertRefusal(t, err, c.want)
		})
	}
}

// The checker steps past exactly what the composition steps past. A recipe
// naming Ingredients beside IngredientsBy is one the settings planner composes
// nothing for, so its dropdown is not reported as needing a description: a
// finding about a string the mod never emits is one nobody can act on.
func TestCheckLocaleSkipsADropdownTheCompositionStepsPast(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	style := lib.DropdownSettingNeedingLocale("style", "plain", []string{"plain", "custom"})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		IngredientsBy: &IngredientChoices{
			Setting: style,
			Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
			Custom:  parts,
		},
	})

	cfg := `[mod-setting-name]
steelworks-style=Axe style
steelworks-axe-ingredients=What an axe is made of

[mod-setting-description]
steelworks-axe-ingredients=Amount, then name, commas between.

[string-mod-setting]
steelworks-style-plain=Plain
steelworks-style-custom=Custom
`
	assertFindings(t, lib.CheckLocale("steelworks", cfg), nil)
}

// ---------------------------------------------------------------------------
// The language seam.
// ---------------------------------------------------------------------------

// THE GUARD OVER A SEAM THE PUBLIC SURFACE CANNOT BREAK, and the reason it is
// tested from inside the package: IngredientsSetting and PacksSetting are the
// only way a consumer declares a text setting and both install the language,
// so a text setting without one arrives only by appending the declaration by
// hand, exactly as these two do. The seam itself is a size decision (see
// Lib.lang: the notext fixture shipped the whole language because the planners
// named it), and a guard nobody has watched fire is a guard nobody has tested.
//
// BOTH PLANNERS ANSWER, because both run validateTextSettings in front of
// their loops and neither may reach a text path with nothing behind it.
func TestTextSettingWithoutTheLanguageIsRefused(t *testing.T) {
	// An ingredients setting with no language at all.
	ingredients := func() *Lib {
		lib := New()
		axe := lib.Item("steel-axe", ItemSpec{})
		lib.settings = append(lib.settings, settingDecl{
			kind:           settingIngredients,
			name:           "axe-ingredients",
			defIngredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		})
		parts := IngredientsSettingRef{lib: lib.id, index: len(lib.settings)}
		lib.Recipe(axe, RecipeSpec{Name: "steel-axe-forging", IngredientsFrom: parts})
		return lib
	}
	// A packs setting WITH the language and without the custom-cost resolver,
	// which is the half packsSetting installs on its own: the two are separate
	// values and a plan holding one of them is still a plan that would call
	// nothing.
	packs := func() *Lib {
		lib := New()
		lib.lang = &language{parse: parseIngredientList, render: renderIngredientList, amount: formatListAmount}
		lib.settings = append(lib.settings, settingDecl{
			kind:     settingPacks,
			name:     "axe-packs",
			defPacks: []Pack{{Name: "automation-science-pack", Amount: 1}},
		})
		list := PacksSettingRef{lib: lib.id, index: len(lib.settings)}
		count := lib.IntSetting("axe-count", 20, Between(1, 100000))
		seconds := lib.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
		lib.Technology("steel-axes", TechSpec{
			CostFrom: &CustomCost{Packs: list, Count: count, Seconds: seconds},
		})
		return lib
	}

	cases := []struct {
		name string
		plan func() *Lib
		want string
	}{
		{
			name: "an ingredients setting with no language",
			plan: ingredients,
			want: "fkrecipes: the text setting axe-ingredients was declared without the ingredient language;" +
				" declare it through IngredientsSetting or PacksSetting",
		},
		{
			name: "a packs setting with no custom-cost resolver",
			plan: packs,
			want: "fkrecipes: the text setting axe-packs was declared without the ingredient language;" +
				" declare it through IngredientsSetting or PacksSetting",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			_, err := c.plan().PlanSettings(settingsWorld())
			assertRefusal(t, err, c.want)
			_, err = c.plan().PlanData(customWorld())
			assertRefusal(t, err, c.want)
		})
	}
}

// A TEXT HANDLE THAT POINTS AT A SETTING OF ANOTHER KIND IS REFUSED, and this
// is what makes the two text validators the only ones that ask the kind. The
// plan id and the index range say the handle came from this plan and lands
// inside its settings; they say nothing about WHAT it lands on, and following
// a handle into a dropdown is what reaches the ingredient language for a plan
// the language guard stepped past. With the language held as function values
// that reach is a call into nothing.
//
// THE SENTENCE IS THE ONE THE BINDING VALIDATOR ALREADY HAS. A handle this
// plan cannot honour is an undeclared setting from the author's side, whatever
// index it carries, and inventing a second sentence for it would give the same
// defect two shapes.
//
// Hand-built, like the guard's own witness above: the constructors issue a
// handle of the right kind, so a crossed one arrives only by writing the
// struct literal, exactly as these two do.
//
// THE DROPDOWN CARRIES A STORED VALUE for the data half, and the two halves
// are separate subtests, because the reach this guard prevents is in the data
// planner and nowhere else: resolveTextList reads the setting first and hands
// back the default when the World has no value for it, so a World without one
// stops short of the language. One shared subtest hid that too, since
// assertRefusal fatals on an accepted plan and the data half never ran.
func TestATextHandleIntoAnotherKindIsRefused(t *testing.T) {
	// A recipe reading its ingredients from a dropdown.
	ingredients := func() *Lib {
		lib := New()
		axe := lib.Item("steel-axe", ItemSpec{})
		style := lib.DropdownSettingNeedingLocale("axe-style", "vanilla", []string{"vanilla", "steel"})
		parts := IngredientsSettingRef{lib: style.lib, index: style.index}
		lib.Recipe(axe, RecipeSpec{Name: "steel-axe-forging", IngredientsFrom: parts})
		return lib
	}
	// A custom research cost reading its packs from the same dropdown.
	packs := func() *Lib {
		lib := New()
		style := lib.DropdownSettingNeedingLocale("axe-style", "vanilla", []string{"vanilla", "steel"})
		count := lib.IntSetting("axe-count", 20, Between(1, 100000))
		seconds := lib.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
		list := PacksSettingRef{lib: style.lib, index: style.index}
		lib.Technology("steel-axes", TechSpec{
			CostFrom: &CustomCost{Packs: list, Count: count, Seconds: seconds},
		})
		return lib
	}

	cases := []struct {
		name string
		plan func() *Lib
		want string
	}{
		{
			name: "a recipe whose IngredientsFrom names a dropdown",
			plan: ingredients,
			want: "fkrecipes: the recipe steel-axe-forging reads its ingredients from a setting that this plan never declared",
		},
		{
			name: "a custom cost whose Packs names a dropdown",
			plan: packs,
			want: "fkrecipes: the technology steel-axes reads its science packs from a setting that this plan never declared",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			t.Run("the settings plan", func(t *testing.T) {
				_, err := c.plan().PlanSettings(settingsWorld())
				assertRefusal(t, err, c.want)
			})
			t.Run("the data plan", func(t *testing.T) {
				w := customWorld().withSetting("steelworks-axe-style", Str("vanilla"))
				_, err := c.plan().PlanData(w)
				assertRefusal(t, err, c.want)
			})
		})
	}
}

// AND THE ORDINARY PLAN INSTALLS BOTH HALVES, which is what makes the guard
// above a guard rather than a wall: a plan that declares its text settings
// through the constructors passes it, and one pack setting is enough for the
// custom-cost resolver.
func TestTheConstructorsInstallTheLanguage(t *testing.T) {
	// A PLAN WITH NO TEXT SETTING CARRIES NEITHER HALF, which is the state the
	// whole seam exists to produce: no field here names the parser, the
	// renderer, the amount formatter or the custom-cost resolver, so nothing
	// in a consumer built this way keeps them alive.
	plain := New()
	plainAxe := plain.Item("steel-axe", ItemSpec{})
	plain.Recipe(plainAxe, RecipeSpec{Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}})
	plain.DropdownSettingNeedingLocale("axe-style", "vanilla", []string{"vanilla", "steel"})
	if plain.lang != nil {
		t.Error("a plan with no text setting installed the language")
	}
	if plain.customCost != nil {
		t.Error("a plan with no text setting installed a custom-cost resolver")
	}

	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	lib.Recipe(axe, RecipeSpec{IngredientsFrom: parts})
	if lib.lang == nil {
		t.Fatal("IngredientsSetting installed no language")
	}
	if lib.customCost != nil {
		t.Error("IngredientsSetting installed a custom-cost resolver, which no plan without a pack setting can reach")
	}

	// A SECOND text setting must not reinstall it: the install is idempotent
	// because the language belongs to the plan and not to the declaration.
	first := lib.lang
	list := lib.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	if lib.lang != first {
		t.Error("PacksSetting replaced the language a previous constructor installed")
	}
	if lib.customCost == nil {
		t.Fatal("PacksSetting installed no custom-cost resolver")
	}
	count := lib.IntSetting("axe-count", 20, Between(1, 100000))
	seconds := lib.DoubleSetting("axe-seconds", 10, Between(0.5, 600))
	lib.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: list, Count: count, Seconds: seconds}})

	if _, err := lib.PlanSettings(settingsWorld()); err != nil {
		t.Errorf("the settings plan refused a plan built through the constructors: %v", err)
	}
	if _, err := lib.PlanData(customWorld()); err != nil {
		t.Errorf("the data plan refused a plan built through the constructors: %v", err)
	}
}

func assertRefusal(t *testing.T, err error, want string) {
	t.Helper()
	if err == nil {
		t.Fatalf("the plan was accepted, want refusal %q", want)
	}
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}

func assertFindings(t *testing.T, got, want []string) {
	t.Helper()
	if strings.Join(got, "\n") != strings.Join(want, "\n") {
		t.Errorf("findings\n got:\n%s\nwant:\n%s", strings.Join(got, "\n"), strings.Join(want, "\n"))
	}
}
