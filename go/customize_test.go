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

// wantTextTail is the two lines this library composes onto EVERY text
// setting's description, after the default list: what to write and how much of
// it, then what a text it cannot use costs.
//
// SPELLED OUT HERE RATHER THAN TAKEN FROM THE SOURCE, which is the whole point
// of a golden: textFormatLine builds the number from maxListChars, so a test
// that asked it for the sentence would move with any edit to either. These
// bytes are the contract, the Rust twin carries the same ones, and the mirror
// compares the two transcripts.
const wantTextTail = `, "` + "\n" +
	`Write internal names, as the default line above does, in at most 2000 characters.", "` + "\n" +
	`A text this mod cannot use is set aside and that default applies instead; the reason is in the log, or in the load error if the load stops anyway."`

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
			` "` + "\n" + `default: 2 tungsten-plate, 4 steelworks-steel-rivet"` + wantTextTail + `]}`,
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
			` localised_description=["", ["mod-setting-description.steelworks-axe-ingredients"], "` + "\n" + `default: none"` + wantTextTail + `]}`,
		`extend {type="string-setting", name="steelworks-axe-packs", setting_type="startup",` +
			` default_value="default", order="ab", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-axe-packs"],` +
			` "` + "\n" + `default: 1 automation-science-pack, 2 military-science-pack"` + wantTextTail + `]}`,
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
			` localised_description=["", ["mod-setting-description.bbb-part-ingredients"], "` + "\n" + `default: 3 steel-plate"` + wantTextTail + `]}`,
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
			` ["", "` + "\n" + `", ["string-mod-setting.steelworks-quench-medium-water"], "` + "\n" + `  type: 2 steel-plate, 10 [fluid=water]"],` +
			` ["", "` + "\n" + `", ["string-mod-setting.steelworks-quench-medium-oil"], "` + "\n" + `  type: 2 steel-plate, 0.5 [fluid=lubricant]"]]}`,
		`extend {type="string-setting", name="steelworks-quench-ingredients", setting_type="startup",` +
			` default_value="default", order="ab", auto_trim=true,` +
			` localised_description=["", ["mod-setting-description.steelworks-quench-ingredients"], "` + "\n" + `default: 2 steel-plate"` + wantTextTail + `]}`,
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

// A readable value that is not a string takes the author's list with ONE line
// saying so. The engine resets a wrong-typed stored value before any stage runs
// (measured), so this is a hand-edited file; the line is what says so, and it
// says it without stopping the game.
func TestTextSettingThatIsNotTextFallsBack(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withSetting("steelworks-rivet-ingredients", Num(3))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-rivet-ingredients is not text.` +
			` The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}, {type="item", name="iron-stick", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
}

// A TEXT THE LANGUAGE REFUSES LOADS THE MOD, and this is the headline of the
// round. The player gets the author's own list, and one ERROR line carrying the
// language's sentence VERBATIM: the setting, the entry and the problem, exactly
// as the corpus pins it, with the shared prefix trimmed off because the line it
// sits in already opens with one.
//
// MEASURED (Factorio 2.0.77, build 84539): the refusal this replaces was
// permanent. The engine rewrites mod-settings.dat on every successful load and
// on no failed one, the client's error dialog cannot reach the Mod Settings
// screen, and disabling the mod does not drop its stored settings. See
// playerFallback.
func TestTextSettingFallsBackOnTheLanguageRefusal(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withSetting("steelworks-rivet-ingredients", Str("2 iron-plate, 1 unobtanium"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-rivet-ingredients, entry 2 ("1 unobtanium"): no item or fluid is named unobtanium.` +
			` The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}, {type="item", name="iron-stick", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
}

// THE AUTHOR'S DECLARED LADDERS ARE WHAT THE FALLBACK LANDS ON, drops and all.
// A refused text is not a text at all as far as the rest of the walk is
// concerned: it takes the same path the reserved word takes, so the modpack
// tolerance the author wrote still applies and its drop lines still print.
func TestRefusedTextFallsBackOntoTheDeclaredLadders(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withoutItem("iron-stick").
		withSetting("steelworks-rivet-ingredients", Str("2 unobtanium"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-rivet-ingredients, entry 1 ("2 unobtanium"): no item or fluid is named unobtanium.` +
			` The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=2}, {type="item", name="iron-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
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

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	// THE SUGGESTION SURVIVES THE FALLBACK, which is the whole value of it: the
	// line the player reads still names the name that is really there.
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-axe-ingredients, entry 1 ("3 Steelworks_Steel_Rivet"):` +
			` no item or fluid is named Steelworks_Steel_Rivet; did you mean steelworks-steel-rivet.` +
			` The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", enabled=true,` +
			` ingredients=[{type="item", name="steelworks-steel-rivet", amount=2}, {type="item", name="iron-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
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

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies`,
		`log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies`,
		`log fkrecipes: ERROR: steelworks-axe-packs, entry 1 ("1 steelworks-steel-rivet"):` +
			` steelworks-steel-rivet is an item, not a science pack.` +
			` The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs:` +
			` count 20, time 10, packs 1 automation-science-pack`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="technology", name="steelworks-steel-axes",` +
			` unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}`,
	})
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

// A TEXT THE LANGUAGE REFUSES, BEHIND A PRESET, IS STILL ONLY AN EDIT. It gets
// the ignored line and NOT the ERROR line, because nothing read it for real:
// the dropdown beside it is on a preset, so the text is not the recipe's list
// and there is no default for it to have fallen back to. Telling the player to
// go and fix a field the mod is not using would send them after the wrong
// thing.
func TestARefusedTextBehindAPresetGetsTheIgnoredLineAndNotTheErrorLine(t *testing.T) {
	lib := customArmPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("oil")).
		withSetting("steelworks-quench-ingredients", Str("4 unobtanium"))

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
// and the engine's own reset rule keep a player from producing one of these
// through the settings screen, and a World is an interface: a NaN used to reach
// the amount formatter and trap, and an infinity used to be rendered into the
// unit.
//
// A NUMBER IS A FIELD THE PLAYER OWNS, so it falls back to the setting's
// DECLARED DEFAULT with one line rather than stopping the load, exactly as the
// pack text beside it does. The line names the SETTING, because that is the
// field somebody would go and fix, and the unit that comes out is the declared
// one: count 20, time 10.
func TestCustomResearchCostFallsBackOnANumberTheEngineWouldNotTake(t *testing.T) {
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
			want:  "steelworks-chain-count holds a value that is not a finite number",
		},
		{
			name:  "a time that is not a finite number",
			count: Num(20),
			time:  Num(math.Inf(1)),
			want:  "steelworks-chain-seconds holds a value that is not a finite number",
		},
		{
			// Finiteness is asked first within each number, so this arm is
			// only ever reached by a real number: a NaN is not below 1 and
			// would have gone through.
			name:  "a count below 1",
			count: Num(0),
			time:  Num(10),
			want:  "steelworks-chain-count holds a research count below 1",
		},
		{
			name:  "a time at or below zero",
			count: Num(20),
			time:  Num(0),
			want:  "steelworks-chain-seconds holds a research time at or below zero",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			w := customWorld().
				withSetting("steelworks-chain-count", c.count).
				withSetting("steelworks-chain-seconds", c.time).
				withSetting("steelworks-chain-packs", Str("1 automation-science-pack"))

			ops, err := chainPlan().PlanData(w)
			assertNoError(t, err)

			assertLines(t, transcript(ops), []string{
				`log fkrecipes: ERROR: ` + c.want +
					`. The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.`,
				`log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs:` +
					` count 20, time 10, packs 1 automation-science-pack`,
				`extend {type="technology", name="steelworks-chain-forging", prerequisites=["steel-processing"],` +
					` unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}`,
			})
		})
	}
}

// AND THE DECLARED DEFAULT THE FALLBACK LANDS ON IS STILL HELD TO THE ENGINE'S
// RULE, which is the author's half of the same pair.
//
// validateSettings refuses a default outside its setting's own bounds at the
// SETTINGS stage, and the engine runs that stage before the data stage, so this
// world exists only for a host test that calls PlanData on its own. It is a
// refusal because a declaration is not a typed value, and it is what keeps the
// invariant that no unit this library emits carries a count the engine refuses.
func TestCustomResearchCostRefusesADeclaredDefaultTheEngineWouldNotTake(t *testing.T) {
	plan := func() *Lib {
		lib := New()
		packs := lib.PacksSetting("chain-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
		// A minimum of 1, which validateCustomCost demands, beside a declared
		// default of 0, which validateSettings refuses at the settings stage.
		count := lib.IntSetting("chain-count", 0, NumericSpec{HasMin: true, Min: 1})
		seconds := lib.DoubleSetting("chain-seconds", 10, Between(0.5, 600))
		lib.Technology("chain-forging", TechSpec{
			CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		})
		return lib
	}
	// The settings stage is where this belongs, and it says so.
	_, err := plan().PlanSettings(settingsWorld())
	assertRefusal(t, err, "fkrecipes: the numeric setting chain-count declares a default outside its own minimum and maximum")

	// And the data stage, reached on its own, refuses rather than emitting a
	// unit the engine would not take. The stored value falls back first, and
	// the sentence is about the DECLARED default it fell back onto: the NaN the
	// setting answered is gone, and the 0 the plan wrote is what is left. The
	// note is the player's half of the same refusal, because a stored value WAS
	// set aside on the way here.
	w := customWorld().
		withSetting("steelworks-chain-count", Num(nan())).
		withSetting("steelworks-chain-seconds", Num(10)).
		withSetting("steelworks-chain-packs", Str("1 automation-science-pack"))

	_, err = plan().PlanData(w)
	assertRefusal(t, err, "fkrecipes: steelworks-chain-count declares a default research count below 1"+
		". The stored value of steelworks-chain-count could not be used, so the mod's own declaration applied;"+
		" correcting it under Settings > Mod settings > Startup is what a player can change here.")
}

// TWO DECLARED DEFAULTS WRONG AT ONCE, AND THE COUNT IS THE ONE THAT ANSWERS.
//
// THIS IS THE ONLY WITNESS TO THAT ORDER. refuseCostNumbers walks the count and
// then the seconds and keeps the first answer; every other test declares at
// most one bad default, so swapping its two arms changed no sentence anywhere
// and the order was free to drift between the halves. A world wrong in both
// places is what pins it.
//
// NOTHING IS TYPED HERE, so the refusal carries no fallback note: both numbers
// ARE the declared defaults rather than values something fell back onto.
func TestRefusedCostNumbersAnswerTheCountBeforeTheSeconds(t *testing.T) {
	lib := New()
	packs := lib.PacksSetting("chain-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	// Both outside what the engine takes, and both refused by validateSettings
	// at the settings stage: PlanData reaches them only on its own.
	count := lib.IntSetting("chain-count", 0, NumericSpec{HasMin: true, Min: 1})
	seconds := lib.DoubleSetting("chain-seconds", 0, NumericSpec{HasMin: true, Min: 0.5})
	lib.Technology("chain-forging", TechSpec{
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	_, err := lib.PlanData(customWorld())
	assertRefusal(t, err, "fkrecipes: steelworks-chain-count declares a default research count below 1")
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

// A STORED NaN IS AN EDIT UNDER A PRESET AND A FALLBACK ON CUSTOM, and the two
// halves are deliberate rather than an accident of NaN comparing false against
// everything. A stored value that is not the default is something the player
// did, so under a preset it draws the ignored line naming the field they can go
// and put back; on the custom value the same number is read for real, cannot be
// used, and takes the declared default with the ERROR line naming the setting
// that holds it. Two different lines, because the two say different things: one
// is "your edit is not live", the other is "your edit is not usable".
func TestCustomCostArmNaNIsAnEditUnderAPresetAndAFallbackOnCustom(t *testing.T) {
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

	ops, err = tipsPlan().PlanData(custom)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-tips-count holds a value that is not a finite number.` +
			` The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 30, time 20, packs 1 automation-science-pack`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-3"],` +
			` unit={count=30, time=20, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// ALL THREE FIELDS WRONG AT ONCE, AND ALL THREE ANSWERED. The count and the
// seconds are both below what the engine takes and the text names a pack no
// game has, so a player who got everything wrong is told about everything:
// three lines, then the cost the mod declared.
//
// THE ORDER IS THE ORDER THE VALUES ARE READ, count then seconds then the pack
// text, which is also the order the cost line names them. This used to be the
// place the two halves fixed which SINGLE refusal came out of a world with more
// than one problem; with a line per field there is nothing to choose between,
// and the ordering that is left is the walk's own.
//
// THE STORED VALUES ARE THE RUST HALF'S, BYTE FOR BYTE: the same pack text, the
// same count, the same seconds, so the three sentences are the same three
// sentences. The two plans behind them are not identical (that one's dropdown
// is named tips-research-tier and it declares two packs), which is why the cost
// line and the unit differ; what a parity pin has to hold still is the input
// and the wording, and those are what match.
func TestCustomCostArmFallsBackOnTheTextAndBothNumbers(t *testing.T) {
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("custom")).
		withSetting("steelworks-tips-count", Num(0)).
		withSetting("steelworks-tips-seconds", Num(math.NaN())).
		withSetting("steelworks-tips-packs", Str("2 unobtainium"))

	ops, err := tipsPlan().PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-tips-count holds a research count below 1.` +
			` The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: ERROR: steelworks-tips-seconds holds a value that is not a finite number.` +
			` The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: ERROR: steelworks-tips-packs, entry 1 ("2 unobtainium"): no science pack is named unobtainium.` +
			` The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 30, time 15, packs 1 automation-science-pack`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-3"],` +
			` unit={count=30, time=15, ingredients=[["automation-science-pack", 1]]}}`,
	})
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

// THE DRIFT GUARD OVER WHAT THE LIBRARY ITSELF COMPOSES. The consumer's entry
// is checked above; these two lines are this library's, so no locale file can
// put one back and no plan can leave one out. What the rule can see is a
// composition that stopped carrying a line, which is why the composition is
// what it is handed.
//
// THE SENTENCE NAMES THE LIBRARY AND NOT A SETTING, because the rule's input
// does not vary with the setting: both lines are constants, and the only
// per-setting part of a text description is the consumer's key, which the rule
// does not look at. One defect is therefore one finding.
//
// THE HEALTHY PATH FIRST, so a rule that fired on everything would be caught
// here rather than in a golden somewhere: the real composition reports
// nothing.
func TestComposedTextLinesMissingGuardsTheComposedLines(t *testing.T) {
	const full = "steelworks-axe-ingredients"
	whole := textDescription(full, "1 steel-plate")
	if got := composedTextLinesMissing(whole); len(got) != 0 {
		t.Fatalf("the real composition reported %v", got)
	}

	// The same composition with one line taken out of it, which is the only
	// way to reach the finding: nothing a consumer declares composes a
	// description missing a line.
	without := func(line string) Value {
		out := make([]Value, 0, len(whole.Arr))
		for _, v := range whole.Arr {
			if v.Kind == KindStr && v.Str == line {
				continue
			}
			out = append(out, v)
		}
		return Arr(out...)
	}
	assertFindings(t, composedTextLinesMissing(without(textFormatLine())), []string{
		"the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
	assertFindings(t, composedTextLinesMissing(without(textFallbackLine)), []string{
		"the library composes no line about what happens to a text this mod cannot use onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
	// Both gone: both reported, format first, which is the order the lines sit
	// in and the order every other rule here reports in.
	stripped := Arr(Str(""), localeRef("mod-setting-description", full))
	assertFindings(t, composedTextLinesMissing(stripped), []string{
		"the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
		"the library composes no line about what happens to a text this mod cannot use onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
}

// THE GUARD IS HANDED ONE COMPOSITION, NOT ONE PER SETTING, and it is the
// first text setting's in declaration order. A plan that declares no text
// setting composes no text description at all, so there is nothing for a line
// to have gone missing from and the guard has nothing to inspect.
//
// THE INPUT IS WHAT THIS PINS, because the finding itself is unreachable from
// a healthy library: the sentences are pinned above and the cap defect was
// about how many times this input is taken, not about what the rule says. Run
// once per text setting it turned one library defect into five findings on the
// example guest.
func TestTheDriftGuardInspectsTheFirstTextSettingOnly(t *testing.T) {
	lib := New()
	lib.BoolSetting("hint", true)
	if _, ok := lib.guardedTextDescription("steelworks-"); ok {
		t.Fatal("a plan with no text setting handed the guard a composition")
	}

	axe := lib.Item("steel-axe", ItemSpec{})
	lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientOf(axe, 1)})
	lib.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	desc, ok := lib.guardedTextDescription("steelworks-")
	if !ok {
		t.Fatal("a plan with two text settings handed the guard nothing")
	}
	// The FIRST one's, and with the list left out: the packs setting declared
	// after it is not what the guard reads, and neither is any rendering.
	want := renderValue(textDescription("steelworks-axe-ingredients", ""))
	if got := renderValue(desc); got != want {
		t.Errorf("\n got: %s\nwant: %s", got, want)
	}
}

// THE TWO LINES, BYTE FOR BYTE, and the number in the first one comes from the
// same constant the parser refuses on. A sentence promising a limit the parser
// does not keep is the drift this pins; the Rust twin carries the same bytes
// and the mirror compares the two transcripts.
func TestTheComposedTextLinesAreTheStatedOnes(t *testing.T) {
	if maxListChars != 2000 {
		t.Fatalf("maxListChars is %d; the sentence below and both halves' docs say 2000", maxListChars)
	}
	if got, want := textFormatLine(),
		"\nWrite internal names, as the default line above does, in at most 2000 characters."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	if got, want := textFallbackLine,
		"\nA text this mod cannot use is set aside and that default applies instead; the reason is in the log, or in the load error if the load stops anyway."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	// A LINE, NOT A SEPARATOR, and the word a player acts on opens it. The
	// dropdown label beside this is the consumer's prose and the client
	// truncates it at about 37 characters; the internal names are on their own
	// line so the copyable half of the tooltip is never the truncated half.
	if got, want := ingredientPresetHead, "\n  type: "; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
}

// A DESCRIPTION IS OPTIONAL EVERYWHERE ELSE and required here, because it is
// where the player learns what the setting is for. The format, the limits and
// the fallback are the library's own lines under it, so an absent entry loses
// all of them at once.
func TestCheckLocaleRequiresATextSettingDescription(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	lib.Recipe(axe, RecipeSpec{IngredientsFrom: parts})

	cfg := "[mod-setting-name]\nsteelworks-axe-ingredients=What an axe is made of\n"
	assertFindings(t, lib.CheckLocale("steelworks", cfg), []string{
		"the setting steelworks-axe-ingredients has no [mod-setting-description] entry, and a text setting needs one to say what the setting is for; the library composes the format, the limits and the fallback onto it",
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

// THE TWO SIDES, OVER ONE PLAN THAT IS WRONG ON BOTH AT ONCE. The player's text
// names nothing the game has and the player's crafting time is at the engine
// floor; the mod is priced in a science pack this game does not carry. Neither
// player field stops the load: both fall back to what the author declared and
// both say so, in the walk's own order (the crafting time is read before the
// ingredients). What stops the load is the pack, which is the author's own
// declaration against the modpack.
//
// THIS IS THE SEPARATION WRITTEN AS ONE TEST, and the claim it holds is the
// narrow one: a value the player TYPED never introduces a refusal that a player
// who never typed would not also have hit. It is not "a player can never be
// refused". The same modpack refuses the same plan with the fields untouched,
// which is the third world below, and the only difference the typing makes is
// the added sentence: the log ops never reach the host on a refused load, so
// the refusal itself is the only place left to say that a stored value was set
// aside. See resolution.withFallbackNote.
func TestPlayerFieldsFallBackWhileTheAuthorChannelStillRefuses(t *testing.T) {
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

	w := customWorld().withoutTool("military-science-pack").
		withSetting("steelworks-axe-ingredients", Str("2 unobtanium")).
		withSetting("steelworks-forging-time", Num(0))

	_, err := plan().PlanData(w)
	assertRefusal(t, err, "fkrecipes: the technology steel-axes has no science pack the game has; research takes at least one"+
		". The stored value of steelworks-forging-time could not be used, so the mod's own declaration applied;"+
		" correcting it under Settings > Mod settings > Startup is what a player can change here.")

	// AND THE SAME REFUSAL WITH NOTHING TYPED, which is what makes the note a
	// fact about this player rather than boilerplate: a player who never opened
	// the settings screen meets the identical modpack problem, and is told only
	// about the mod. The note names the crafting time rather than the text
	// because the crafting time is read first; the pair is the walk's order,
	// which is the same every run.
	_, err = plan().PlanData(customWorld().withoutTool("military-science-pack"))
	assertRefusal(t, err, "fkrecipes: the technology steel-axes has no science pack the game has; research takes at least one")

	// The same plan with the pack put back loads, and the two player fields are
	// the whole log: the declared list, the declared crafting time, two lines.
	ok := customWorld().
		withSetting("steelworks-axe-ingredients", Str("2 unobtanium")).
		withSetting("steelworks-forging-time", Num(0))

	ops, err := plan().PlanData(ok)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: the recipe steel-axe-forging reads its crafting time from steelworks-forging-time,` +
			` which answers at or below the engine floor (energy_required can't be <= 0.001).` +
			` The mod loaded with its own default instead; fix the number under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: ERROR: steelworks-axe-ingredients, entry 1 ("2 unobtanium"): no item or fluid is named unobtanium.` +
			` The mod loaded with its own default instead; fix the text under Settings > Mod settings > Startup, then restart.`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", energy_required=3, enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
		`extend {type="technology", name="steelworks-steel-axes",` +
			` unit={count=1, time=1, ingredients=[["military-science-pack", 1]]}}`,
	})
}

// AND THE CHANNELS THAT ARE LEFT KEEP THEIR ORDER. A dropdown holding a value
// it does not offer is carried out of the walk and answered before the packs,
// which is the same "carried refusal first" rule the merged-amount ceiling
// rides on. Both are hand-edited files or author declarations, never a player's
// typing.
func TestRefusalChannelOrder(t *testing.T) {
	plan := func() *Lib {
		lib := New()
		axe := lib.Item("steel-axe", ItemSpec{})
		style := lib.DropdownSettingNeedingLocale("axe-style", "plain", []string{"plain", "fancy"})
		lib.Recipe(axe, RecipeSpec{
			Name: "steel-axe-forging",
			IngredientsBy: &IngredientChoices{
				Setting: style,
				Choices: []IngredientChoice{
					{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}},
					{Value: "fancy", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
				},
			},
		})
		lib.Technology("steel-axes", TechSpec{Unit: &UnitSpec{
			Count:   1,
			Seconds: 1,
			Packs:   []Pack{{Name: "military-science-pack", Amount: 1}},
		}})
		return lib
	}

	cases := []struct {
		name  string
		style string
		want  string
	}{
		{
			name:  "the carried refusal answers first",
			style: "gilded",
			want:  `fkrecipes: steelworks-axe-style holds "gilded", which is not one of its values`,
		},
		{
			name:  "then the packs the game does not have",
			style: "fancy",
			want:  "fkrecipes: the technology steel-axes has no science pack the game has; research takes at least one",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			// The pack is absent in both cases, so the second channel is armed
			// throughout and only the one in front of it is repaired.
			w := customWorld().withoutTool("military-science-pack").
				withSetting("steelworks-axe-style", Str(c.style))

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
	//
	// THOSE FOUR, and not everything: the description prose the settings
	// planner composes for a text setting is named from a runtime branch every
	// guest compiles in, so that prose ships whether or not a text setting is
	// declared. Measured, under "The claim gets its adjective" in Fix round 1b
	// of agents/implementation-notes.md.
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

// EVERY SENTENCE THIS LAYER BUILDS OPENS WITH THE SHARED PREFIX, EXACTLY ONCE,
// and this is the half of that property which is not the corpus.
//
// WHAT EACH ASSERTION CATCHES, because the two are about different failures and
// neither is "the line looks wrong". strings.TrimPrefix is unconditional, so a
// sentence that forgot the prefix produces a perfectly well formed line; what
// it breaks is the sentence's OTHER use, as a refusal raised on its own, where
// a message with no library name on it is one nobody can trace. That is the
// HasPrefix assertion. The second assertion is the opposite mistake: a sentence
// that carries the prefix TWICE survives a single trim and reaches the player
// as "fkrecipes: ERROR: fkrecipes: ...".
//
// EVERY PRODUCER IS HERE BECAUSE EVERY ONE IS A FUNCTION, and the faults are
// taken from the fault functions rather than written down, so a rule a number
// can fail with no sentence behind it comes out as an empty string and fails
// the first assertion. A sentence spelled inline at a call site would be one
// this test cannot see, and the reviews would have to catch it instead.
func TestFallbackSentencesCarryThePrefix(t *testing.T) {
	sentences := []struct {
		name string
		text string
	}{
		{"not text", notTextSentence("mymod-parts")},
		{"a stored count that is not finite", storedNumberProblem("mymod-count", countFault(nan()))},
		{"a stored count below 1", storedNumberProblem("mymod-count", countFault(0))},
		{"a stored time that is not finite", storedNumberProblem("mymod-seconds", secondsFault(nan()))},
		{"a stored time at or below zero", storedNumberProblem("mymod-seconds", secondsFault(0))},
		{"a declared count that is not finite", declaredNumberProblem("mymod-count", countFault(nan()))},
		{"a declared count below 1", declaredNumberProblem("mymod-count", countFault(0))},
		{"a declared time that is not finite", declaredNumberProblem("mymod-seconds", secondsFault(nan()))},
		{"a declared time at or below zero", declaredNumberProblem("mymod-seconds", secondsFault(0))},
		{"a stored crafting time that is not finite", storedCraftTimeProblem("axe", "mymod-craft-time", craftTimeFault(nan()))},
		{"a stored crafting time at the floor", storedCraftTimeProblem("axe", "mymod-craft-time", craftTimeFault(0))},
		{"a declared crafting time that is not finite", declaredCraftTimeProblem("axe", "mymod-craft-time", craftTimeFault(nan()))},
		{"a declared crafting time at the floor", declaredCraftTimeProblem("axe", "mymod-craft-time", craftTimeFault(0))},
	}
	for _, s := range sentences {
		t.Run(s.name, func(t *testing.T) {
			if !strings.HasPrefix(s.text, messagePrefix) {
				t.Fatalf("a sentence the fallback composer quotes does not open with %q: %s", messagePrefix, s.text)
			}
			line := playerFallback(s.text, "text")
			if strings.Contains(line, messagePrefix+"ERROR: "+messagePrefix) {
				t.Errorf("the prefix was not stripped, so the line carries it twice: %s", line)
			}
		})
	}
}

// AND THE TWO FIELD WORDS ARE THE WHOLE SHAPE OF A LINE, written out once so
// the sentence itself is pinned here as well as inside every transcript that
// carries one.
func TestPlayerFallbackLineShape(t *testing.T) {
	got := playerFallback("fkrecipes: mymod-parts is not text", "text")
	want := "fkrecipes: ERROR: mymod-parts is not text." +
		" The mod loaded with its own default instead;" +
		" fix the text under Settings > Mod settings > Startup, then restart."
	if got != want {
		t.Errorf("\n got: %s\nwant: %s", got, want)
	}
	got = numberFallback("fkrecipes: mymod-count holds a research count below 1")
	want = "fkrecipes: ERROR: mymod-count holds a research count below 1." +
		" The mod loaded with its own default instead;" +
		" fix the number under Settings > Mod settings > Startup, then restart."
	if got != want {
		t.Errorf("\n got: %s\nwant: %s", got, want)
	}
	if textFallback("fkrecipes: mymod-parts is not text") != playerFallback("fkrecipes: mymod-parts is not text", "text") {
		t.Error("textFallback and playerFallback disagree about the field word")
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
