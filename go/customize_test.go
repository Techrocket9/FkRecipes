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

// The lines this library composes onto a text setting's description, after the
// default list: what a name this game does not have costs the list, on the
// standalone shape alone; what to write and how much of it; which field decides
// while this one holds the reserved word; and what a text it cannot use costs.
//
// THE LADDER LINE IS NOT ON EVERY TEXT SETTING. Beside a dropdown the ladder is
// disclosed on the dropdown's own description, over the presets a player is
// choosing between, so wantTextTail and wantPacksTail carry it (they are the
// standalone shape, which is what wantSwitchOwn says) and the tails built
// around wantSwitchBy do not.
//
// THE FORMAT LINE IS TWO SENTENCES ON AN INGREDIENT SETTING AND ONE ON A PACKS
// SETTING. The word none empties an ingredient list; a pack list refuses it
// ("research takes at least one science pack"), so naming it there would be
// telling a player to type a word the library turns down.
//
// SPELLED OUT HERE RATHER THAN TAKEN FROM THE SOURCE, which is the whole point
// of a golden: textFormatLine builds the number from maxListChars, so a test
// that asked it for the sentence would move with any edit to either. These
// bytes are the contract, the Rust twin carries the same ones, and the mirror
// compares the two transcripts.
const (
	// The ladder line, in each of its three vocabularies: an ingredient text
	// setting's, a packs text setting's, and an ingredient dropdown's.
	wantTextLadder = `, "` + "\n" +
		`Where a list this mod chose names something your mods do not have, the next name it offers is used instead; an entry it offers nothing for is left out, and two that land on one name have their amounts added, so what you craft can be a shorter list than the one shown."`
	wantPacksLadder = `, "` + "\n" +
		`Where a list this mod chose names a science pack your mods do not have, the next name it offers is used instead; a pack it offers nothing for is left out, and two that land on one pack have their amounts added, so the research can take fewer packs than the list shows."`
	wantDropdownLadder = `, "` + "\n" +
		`Where an option names something your mods do not have, the next name it offers is used instead; an entry it offers nothing for is left out, and two that land on one name have their amounts added, so what you craft can be a shorter list than the one shown."`
	wantPacksFormat = `, "` + "\n" +
		`Internal names, as on the default line, up to 2000 characters."`
	wantTextFormat = `, "` + "\n" +
		`Internal names, as on the default line, up to 2000 characters. The word none empties the list, so the recipe costs nothing to craft."`
	wantTextFallback = `, "` + "\n" +
		`Text this mod cannot use is set aside as though it said default; the reason is in the log or the load error."`
	// The switch line a text setting with no dropdown beside it carries.
	wantSwitchOwn = `, "` + "\n" + `Leave this as default and this mod's own list applies; anything else applies instead."`
	// wantTextTail is the whole tail of the commonest shape, an INGREDIENT text
	// setting that is the only source its recipe has, and wantPacksTail its
	// packs twin.
	wantTextTail  = wantTextLadder + wantTextFormat + wantSwitchOwn + wantTextFallback
	wantPacksTail = wantPacksLadder + wantPacksFormat + wantSwitchOwn + wantTextFallback
)

// wantSwitchBy is the switch line a text setting beside a dropdown carries, and
// where says which way the settings screen sorts the two.
func wantSwitchBy(where string) string {
	return `, "` + "\n" + `Leave this as default and the option chosen ` + where +
		` decides; anything else applies instead."`
}

// wantDropdownSwitch is the sentence appended to a dropdown that has a text
// setting beside it.
func wantDropdownSwitch(where string) string {
	return `, "` + "\n" + `The setting ` + where + ` applies instead while it does not say default."`
}

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
			` localised_description=["", ["?", ["mod-setting-description.steelworks-rivet-ingredients"], "steelworks-rivet-ingredients"],` +
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
	seconds := lib.IntSetting("axe-seconds", 10, Between(1, 600))
	lib.Technology("steel-axes", TechSpec{
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="steelworks-axe-ingredients", setting_type="startup",` +
			` default_value="default", order="aa", auto_trim=true,` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-axe-ingredients"], "steelworks-axe-ingredients"], "` + "\n" + `default: none"` + wantTextTail + `]}`,
		`extend {type="string-setting", name="steelworks-axe-packs", setting_type="startup",` +
			` default_value="default", order="ab", auto_trim=true,` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-axe-packs"], "steelworks-axe-packs"],` +
			` "` + "\n" + `default: 1 automation-science-pack, 2 military-science-pack"` + wantPacksTail + `]}`,
		`extend {type="int-setting", name="steelworks-axe-count", setting_type="startup", default_value=20, order="ac", minimum_value=1, maximum_value=100000,` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-axe-count"], "steelworks-axe-count"], "` + "\n" + `A whole number from 1 to 100000."]}`,
		`extend {type="int-setting", name="steelworks-axe-seconds", setting_type="startup", default_value=10, order="ad", minimum_value=1, maximum_value=600,` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-axe-seconds"], "steelworks-axe-seconds"], "` + "\n" + `A whole number from 1 to 600."]}`,
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
			` localised_description=["", ["?", ["mod-setting-description.bbb-part-ingredients"], "bbb-part-ingredients"], "` + "\n" + `default: 3 steel-plate"` + wantTextTail + `]}`,
	})
}

// The composed dropdown description: the consumer's own key, then one nested
// string per preset labelled by the value's OWN locale entry, because the
// settings screen shows the player that label and not the raw key.
func TestPlanSettingsComposesADropdownDescription(t *testing.T) {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
	quench := lib.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	lib.Recipe(plate, RecipeSpec{
		Category: "chemistry",
		IngredientsBy: &IngredientChoices{
			Setting: medium,
			Choices: []IngredientChoice{
				{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate"), FluidIngredient(10, "water")}},
				{Value: "oil", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate"), FluidIngredient(0.5, "lubricant")}},
			},
		},
		IngredientsFrom: quench,
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="steelworks-quench-medium", setting_type="startup",` +
			` default_value="water", order="aa", allowed_values=["water", "oil"],` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-quench-medium"], "steelworks-quench-medium"],` +
			` ["", "` + "\n" + `", ["?", ["string-mod-setting.steelworks-quench-medium-water"], "water"], "` + "\n" + `  to type: 2 steel-plate, 10 [fluid=water]"],` +
			` ["", "` + "\n" + `", ["?", ["string-mod-setting.steelworks-quench-medium-oil"], "oil"], "` + "\n" + `  to type: 2 steel-plate, 0.5 [fluid=lubricant]"]` +
			wantDropdownLadder + wantDropdownSwitch("below") + `]}`,
		`extend {type="string-setting", name="steelworks-quench-ingredients", setting_type="startup",` +
			` default_value="default", order="ab", auto_trim=true,` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-quench-ingredients"], "steelworks-quench-ingredients"], "` + "\n" + `default: 2 steel-plate"` +
			wantTextFormat + wantSwitchBy("above") + wantTextFallback + `]}`,
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
//
// AND THE WHOLE VALUE IS PINNED, WHICH IS WHERE THE TWO NEGATIVES LIVE. A cost
// dropdown carries neither listWrapLine nor dropdownLadderLine: its presets
// render no typeable list of internal names, so there is nothing for a wrap to
// cut and nothing for a ladder to shorten. Comparing the composition whole is
// what makes a line added to the wrong dropdown a failure here rather than a
// silent gain, so this want string may not be loosened to a contains check.
func TestPlanSettingsComposesACostDropdownDescription(t *testing.T) {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-tier", "projectile", []string{"projectile", "none"})
	packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tips-count", 0, Between(0, 100000))
	seconds := lib.IntSetting("tips-seconds", 0, Between(0, 600))
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{
				{Value: "projectile", Sources: []string{"mining-productivity-4", "logistics-3"}},
				{Value: "none"},
			},
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
		},
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	got, ok := field(ops[0].Proto, "localised_description")
	if !ok {
		t.Fatalf("the dropdown carries no composed description")
	}
	want := `["", ["?", ["mod-setting-description.steelworks-tips-tier"], "steelworks-tips-tier"],` +
		` ["", "` + "\n" + `", ["?", ["string-mod-setting.steelworks-tips-tier-projectile"], "projectile"], ": cost of ", ["?", ["technology-name.mining-productivity-4"], "mining-productivity-4"]],` +
		` ["", "` + "\n" + `", ["?", ["string-mod-setting.steelworks-tips-tier-none"], "none"], ": the fallback cost"], "` + "\n" +
		`The setting below applies instead while it does not say default."]`
	if renderValue(got) != want {
		t.Errorf("\n got: %s\nwant: %s", renderValue(got), want)
	}
}

// THE TWO SWITCH LINES FOLLOW THE EMITTED ORDER, NOT THE DECLARATION ORDER.
//
// EVERY OTHER FIXTURE IN THIS FILE DECLARES THE DROPDOWN FIRST, so relativeOrder
// answers "above" on the text side and "below" on the dropdown side in all of
// them, and a pair of hard-coded constants would pass every one. This is the
// other half of its domain, and the words it produces here appear nowhere else
// in the repository: a LEGACY dropdown carrying the consumer's own order "z"
// sorts UNDER a generated setting whose order is two letters starting at "a",
// so the text setting points down and the dropdown points up. Whether the
// author declared them in that order is not what either sentence is about.
func TestTheSwitchLinesFollowTheEmittedOrder(t *testing.T) {
	lib := New()
	parts := lib.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	medium := lib.LegacyDropdownSettingNeedingLocale("steelworks-quench-medium", "water",
		[]string{"water", "oil"}, "z")
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	lib.Recipe(plate, RecipeSpec{
		IngredientsFrom: parts,
		IngredientsBy: &IngredientChoices{
			Setting: medium,
			Choices: []IngredientChoice{
				{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
				{Value: "oil", Ingredients: []Ingredient{IngredientNamed(3, "steel-plate")}},
			},
		},
	})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="string-setting", name="steelworks-quench-ingredients", setting_type="startup",` +
			` default_value="default", order="aa", auto_trim=true,` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-quench-ingredients"], "steelworks-quench-ingredients"], "` + "\n" +
			`default: 2 steel-plate"` + wantTextFormat + wantSwitchBy("below") + wantTextFallback + `]}`,
		`extend {type="string-setting", name="steelworks-quench-medium", setting_type="startup",` +
			` default_value="water", order="z", allowed_values=["water", "oil"],` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-quench-medium"], "steelworks-quench-medium"],` +
			` ["", "` + "\n" + `", ["?", ["string-mod-setting.steelworks-quench-medium-water"], "water"], "` + "\n" +
			`  to type: 2 steel-plate"],` +
			` ["", "` + "\n" + `", ["?", ["string-mod-setting.steelworks-quench-medium-oil"], "oil"], "` + "\n" +
			`  to type: 3 steel-plate"]` + wantDropdownLadder + wantDropdownSwitch("above") + `]}`,
	})
}

// deepestTable is how far down the deepest TABLE sits, counting the value
// handed in as level one. Tables are what the engine counts against its
// twenty-level ceiling; a string is not a level.
//
// IT IS THE GO TWIN OF rust/src/tests/customize.rs's deepest_table, and the two
// exist because the wrapper's whole measured cost is ONE LEVEL and nothing
// else: a count of parameters cannot see it, and a golden can only see it where
// somebody wrote the shape out.
func deepestTable(v Value, depth int) int {
	if v.Kind != KindArr {
		return 0
	}
	deepest := depth
	for _, item := range v.Arr {
		if d := deepestTable(item, depth+1); d > deepest {
			deepest = d
		}
	}
	return deepest
}

// PAST NINETEEN PRESETS THE COMPOSITION STILL NESTS, AND THE NEW LINE IS WHAT
// IT NESTS. A preset line is ONE parameter of the group above it however many
// tables sit inside it, so the five-parameter cost line does not eat into the
// ceiling and the flat-then-nested shape is exactly the one a four-parameter
// line produced. The technology-name table rides BESIDE the label, at the same
// level, so a flat cost description is FOUR table levels deep: the description,
// the preset line, the localeRef wrapper around the technology name and the key
// table inside it. The fourth is the one level every localeRef spends, against
// twenty the engine takes.
func TestCostDropdownDescriptionNestsPastNineteenPresets(t *testing.T) {
	composed := func(presets int) Value {
		t.Helper()
		values := make([]string, 0, presets+1)
		choices := make([]CostChoice, 0, presets)
		for i := 0; i < presets; i++ {
			v := "tier" + strconv.Itoa(i)
			values = append(values, v)
			choices = append(choices, CostChoice{Value: v, Sources: []string{"source" + strconv.Itoa(i)}})
		}

		lib := New()
		tier := lib.DropdownSettingNeedingLocale("tips-tier", values[0], values)
		packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
		count := lib.IntSetting("tips-count", 0, Between(0, 100000))
		seconds := lib.IntSetting("tips-seconds", 0, Between(0, 600))
		lib.Technology("hardened-tips", TechSpec{
			CostBy: &CostChoices{
				Setting:  tier,
				Choices:  choices,
				Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
			},
			CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		})

		ops, err := lib.PlanSettings(settingsWorld())
		assertNoError(t, err)
		v, ok := field(ops[0].Proto, "localised_description")
		if !ok {
			t.Fatalf("the dropdown carries no composed description")
		}
		return v
	}

	// Two presets, so nothing nests, and the depth is the shape's own.
	if d := deepestTable(composed(2), 1); d != 4 {
		t.Errorf("a flat cost description is %d tables deep, want 4", d)
	}

	const presets = 21
	got := composed(presets)
	// Twenty-three parameters (the key, twenty-one presets and the switch
	// line), so nineteen stay and the twentieth slot nests.
	if got.Kind != KindArr || len(got.Arr) != maxLocalisedParams+1 {
		t.Fatalf("the top level is %s", renderValue(got))
	}
	if tail := got.Arr[maxLocalisedParams]; tail.Kind != KindArr || len(tail.Arr) != 5 {
		t.Errorf("the last slot is %s, want a nested group of three lines and the switch line", renderValue(tail))
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

// AN INGREDIENT DROPDOWN NESTS ONE PRESET EARLIER THAN A COST ONE, and that
// preset is the whole of what the ladder line costs.
//
// The top table holds the consumer's key, the preset lines, the ladder line and
// the switch line, so seventeen presets fill the measured twenty and the
// eighteenth turns the level into groups of nineteen. A COST dropdown carries
// no ladder line at all (its presets are prose in one vocabulary, with nothing
// in them to copy and no rendered list to shorten) and fits eighteen, which is
// what the test above it pins.
//
// IT IS THE GO TWIN of rust/src/tests/customize.rs's
// an_ingredient_description_nests_past_seventeen_presets.
func TestIngredientDropdownDescriptionNestsPastSeventeenPresets(t *testing.T) {
	composed := func(presets int) Value {
		t.Helper()
		values := make([]string, 0, presets)
		choices := make([]IngredientChoice, 0, presets)
		for i := 0; i < presets; i++ {
			v := "p" + strconv.Itoa(i)
			values = append(values, v)
			choices = append(choices, IngredientChoice{
				Value:       v,
				Ingredients: []Ingredient{IngredientNamed(1, "iron-plate")},
			})
		}

		lib := New()
		tier := lib.DropdownSettingNeedingLocale("tier", values[0], values)
		plate := lib.Item("hardened-steel-plate", ItemSpec{})
		parts := lib.IngredientsSetting("parts", []Ingredient{IngredientNamed(1, "iron-plate")})
		lib.Recipe(plate, RecipeSpec{
			IngredientsBy:   &IngredientChoices{Setting: tier, Choices: choices},
			IngredientsFrom: parts,
		})

		ops, err := lib.PlanSettings(settingsWorld())
		assertNoError(t, err)
		v, ok := field(ops[0].Proto, "localised_description")
		if !ok {
			t.Fatalf("the dropdown carries no composed description")
		}
		return v
	}

	// Seventeen: the key plus seventeen lines plus the ladder line plus the
	// switch line is twenty parameters, the measured ceiling, and nothing
	// nests.
	flat := composed(17)
	if flat.Kind != KindArr || len(flat.Arr) != maxLocalisedParams+1 {
		t.Fatalf("seventeen presets did not stay flat: %s", renderValue(flat))
	}
	// THE TWO TRAILING LINES IN THE ORDER THE DOC COMMENTS CLAIM: the ladder
	// line under the last list, and the switch line last. A ladder line wedged
	// between two presets would point at a list that is not the last one.
	tail := flat.Arr[len(flat.Arr)-2:]
	for i, want := range []string{dropdownLadderLine, dropdownSwitchLine("below")} {
		if got := tail[i]; got.Kind != KindStr || got.Str != want {
			t.Errorf("trailing parameter %d is %s, want %q", i, renderValue(got), want)
		}
	}

	// Eighteen: the level keeps the first nineteen parameters and hands the
	// two last lines to a nested group in the twentieth slot.
	nested := composed(18)
	if nested.Kind != KindArr || len(nested.Arr) != maxLocalisedParams+1 {
		t.Fatalf("the nested form is not one level wide: %s", renderValue(nested))
	}
	group := nested.Arr[maxLocalisedParams]
	if group.Kind != KindArr || len(group.Arr) != 3 {
		t.Fatalf("the nested group is %s, want the ladder line and the switch line", renderValue(group))
	}
	if got := group.Arr[1]; got.Kind != KindStr || got.Str != dropdownLadderLine {
		t.Errorf("the nested group does not open with the ladder line: %s", renderValue(got))
	}
	if got := group.Arr[2]; got.Kind != KindStr || got.Str != dropdownSwitchLine("below") {
		t.Errorf("the nested group does not end with the switch line: %s", renderValue(got))
	}

	// And two presets further, the group holds a preset LINE as well, which is
	// what says the fill keeps going rather than stopping at the two lines.
	deeper := composed(20)
	group = deeper.Arr[maxLocalisedParams]
	if group.Kind != KindArr || len(group.Arr) != 5 {
		t.Fatalf("the nested group is %s, want five elements", renderValue(group))
	}
	if group.Arr[1].Kind != KindArr {
		t.Errorf("the nested group's first member is not a preset line: %s", renderValue(group.Arr[1]))
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
			name: "a CustomCost naming another plan's packs setting",
			build: func(l *Lib) {
				other := New()
				packs := other.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
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
				seconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
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
				seconds := other.IntSetting("axe-seconds", 10, Between(1, 600))
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
				seconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-count backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
		},
		{
			name: "a count setting whose minimum is zero",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(0, 100))
				seconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-count backs a research count but declares no minimum of at least 1 (the engine refuses a unit count of 0)",
		},
		{
			name: "a seconds setting whose minimum is zero",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100))
				seconds := l.IntSetting("axe-seconds", 10, Between(0, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-seconds backs a research time but declares no minimum of at least 1 (the engine refuses a unit time of 0)",
		},
		{
			// A FIELD THE PLAYER TYPES INTO NEEDS A CEILING, and the settings
			// screen has no other one to show them: an int setting with no
			// maximum_value takes any number the engine's own encoding holds.
			name: "a count setting with no maximum",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, NumericSpec{HasMin: true, Min: 1})
				seconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-count backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach",
		},
		{
			name: "a seconds setting with no maximum",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(1, 100))
				seconds := l.IntSetting("axe-seconds", 10, NumericSpec{HasMin: true, Min: 1})
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
			},
			want: "fkrecipes: the setting axe-seconds backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach",
		},
		{
			// A DECLARED MAXIMUM OF ZERO IS THE OTHER ARM OF THE SAME RULE, and
			// it is why the sentence says "no maximum of at least 1" rather
			// than "no maximum": this declaration carries one, and 0 is the
			// only value it allows, which is the word that means "the dropdown
			// decides" and never a cost. A sentence naming an absent maximum
			// would be false of exactly this case. It is written beside a
			// dropdown because that is where a 0 minimum is legal at all; with
			// no dropdown the minimum rule answers first.
			name: "a count setting beside a dropdown whose maximum is zero",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap"})
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 0, Between(0, 0))
				seconds := l.IntSetting("axe-seconds", 0, Between(0, 600))
				l.Technology("steel-axes", TechSpec{
					CostBy:   cheapTier(tier),
					CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
				})
			},
			want: "fkrecipes: the setting axe-count backs a research number but declares no maximum of at least 1; a number the player types needs a ceiling it can reach",
		},
		{
			// BESIDE A DROPDOWN THE RULE INVERTS, because 0 is what a number
			// says instead of the reserved word: a declared default of
			// anything else would price the research out of a field the
			// player never touched and leave the tier saying nothing.
			name: "a count setting beside a dropdown whose default is not zero",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap"})
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 20, Between(0, 100))
				seconds := l.IntSetting("axe-seconds", 0, Between(0, 600))
				l.Technology("steel-axes", TechSpec{
					CostBy:   cheapTier(tier),
					CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
				})
			},
			want: "fkrecipes: the setting axe-count backs a research count beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
		},
		{
			name: "a count setting beside a dropdown whose minimum is not zero",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap"})
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 0, NumericSpec{HasMax: true, Max: 100})
				seconds := l.IntSetting("axe-seconds", 0, Between(0, 600))
				l.Technology("steel-axes", TechSpec{
					CostBy:   cheapTier(tier),
					CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
				})
			},
			want: "fkrecipes: the setting axe-count backs a research count beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
		},
		{
			name: "a seconds setting beside a dropdown whose default is not zero",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap"})
				packs := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("axe-count", 0, Between(0, 100))
				seconds := l.IntSetting("axe-seconds", 10, Between(0, 600))
				l.Technology("steel-axes", TechSpec{
					CostBy:   cheapTier(tier),
					CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
				})
			},
			want: "fkrecipes: the setting axe-seconds backs a research time beside a research dropdown, so its declared default and its minimum must both be 0 (0 means the dropdown decides)",
		},
		{
			// ONE DROPDOWN COMPOSES ONE DESCRIPTION, so the presets it shows
			// can only be one declaration's. Two recipes putting a text
			// setting beside it is two preset lists for one string, and the
			// player would read the other recipe's.
			name: "two recipes putting a text setting beside one dropdown",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				head := l.Item("axe-head", ItemSpec{})
				style := l.DropdownSettingNeedingLocale("style", "plain", []string{"plain"})
				axeParts := l.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
				headParts := l.IngredientsSetting("head-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
				plain := func() *IngredientChoices {
					return &IngredientChoices{
						Setting: style,
						Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
					}
				}
				l.Recipe(axe, RecipeSpec{IngredientsBy: plain(), IngredientsFrom: axeParts})
				l.Recipe(head, RecipeSpec{IngredientsBy: plain(), IngredientsFrom: headParts})
			},
			want: "fkrecipes: the setting steelworks-style takes a text setting from more than one recipe; one dropdown composes one description",
		},
		{
			name: "two technologies putting a text setting beside one dropdown",
			build: func(l *Lib) {
				tier := l.DropdownSettingNeedingLocale("tier", "cheap", []string{"cheap"})
				axePacks := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				axeCount := l.IntSetting("axe-count", 0, Between(0, 100000))
				axeSeconds := l.IntSetting("axe-seconds", 0, Between(0, 600))
				sawPacks := l.PacksSetting("saw-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				sawCount := l.IntSetting("saw-count", 0, Between(0, 100000))
				sawSeconds := l.IntSetting("saw-seconds", 0, Between(0, 600))
				l.Technology("steel-axes", TechSpec{
					CostBy:   cheapTier(tier),
					CostFrom: &CustomCost{Packs: axePacks, Count: axeCount, Seconds: axeSeconds},
				})
				l.Technology("steel-saws", TechSpec{
					CostBy:   cheapTier(tier),
					CostFrom: &CustomCost{Packs: sawPacks, Count: sawCount, Seconds: sawSeconds},
				})
			},
			want: "fkrecipes: the setting steelworks-tier takes a text setting from more than one technology; one dropdown composes one description",
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
			// A RESEARCH NUMBER IS BOUND ONCE, and the composed description is
			// why: it names ONE dropdown as the thing deciding while the field
			// is 0, so a setting two technologies priced themselves with would
			// be described by whichever of them composed last.
			//
			// THE MIXED PAIR IS NOT REPRESENTABLE ANY MORE. One double backing
			// a recipe's crafting time and a research time at once was the
			// case this rule was written for; CustomCost.Seconds is an
			// IntSettingRef now and CraftTimeFrom is a DoubleSettingRef, so no
			// handle fits both slots and the two technologies below are the
			// whole remaining domain.
			name: "a count setting two technologies read",
			build: func(l *Lib) {
				axePacks := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				sawPacks := l.PacksSetting("saw-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("shared-count", 20, Between(1, 100000))
				axeSeconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
				sawSeconds := l.IntSetting("saw-seconds", 10, Between(1, 600))
				l.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{
					Packs: axePacks, Count: count, Seconds: axeSeconds,
				}})
				l.Technology("steel-saws", TechSpec{CostFrom: &CustomCost{
					Packs: sawPacks, Count: count, Seconds: sawSeconds,
				}})
			},
			want: "fkrecipes: the setting shared-count is read as a research count or time by more than one declaration; a custom cost's number serves exactly one",
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
				axeSeconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
				sawSeconds := l.IntSetting("saw-seconds", 10, Between(1, 600))
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
			// the count it reads is one another technology reads too. Counting
			// it would answer an undeclared cost with a sentence about
			// sharing.
			name: "two cost sources over one count",
			build: func(l *Lib) {
				axePacks := l.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				sawPacks := l.PacksSetting("saw-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
				count := l.IntSetting("shared-count", 20, Between(1, 100000))
				axeSeconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
				sawSeconds := l.IntSetting("saw-seconds", 10, Between(1, 600))
				l.Technology("steel-axes", TechSpec{
					CostOf:   "logistics-2",
					CostFrom: &CustomCost{Packs: axePacks, Count: count, Seconds: axeSeconds},
				})
				l.Technology("steel-saws", TechSpec{CostFrom: &CustomCost{
					Packs: sawPacks, Count: count, Seconds: sawSeconds,
				}})
			},
			want:     "fkrecipes: the technology steel-axes must name exactly one of CostOf, Unit, CostBy or CostFrom",
			dataOnly: true,
		},
		{
			// The recipe twin: a crafting time named twice, once by hand and
			// once by a handle. "Pick one" is what the author has to fix
			// first, and it is the data planner's line.
			name: "CraftTime beside CraftTimeFrom",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				seconds := l.DoubleSetting("forge-seconds", 10, Between(0.5, 600))
				l.Recipe(axe, RecipeSpec{
					CraftTime:     2,
					CraftTimeFrom: seconds,
					Ingredients:   []Ingredient{IngredientNamed(1, "steel-plate")},
				})
			},
			want:     "fkrecipes: the recipe steel-axe names both CraftTime and CraftTimeFrom; pick one",
			dataOnly: true,
		},
		{
			name: "a packs setting declaring no pack",
			build: func(l *Lib) {
				packs := l.PacksSetting("axe-packs", nil)
				count := l.IntSetting("axe-count", 20, Between(1, 100000))
				seconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
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
				seconds := l.IntSetting("axe-seconds", 10, Between(1, 600))
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

// cheapTier is a research dropdown with one preset, so a refusal about the
// settings BESIDE it is the sentence under test rather than one about the
// dropdown's own shape.
func cheapTier(setting DropdownSettingRef) *CostChoices {
	return &CostChoices{
		Setting:  setting,
		Choices:  []CostChoice{{Value: "cheap", Sources: []string{"logistics-2"}}},
		Fallback: UnitSpec{Count: 1, Seconds: 1, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
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

// A VALUE BESIDE ok=false IS STILL ABSENT, AND EVERY READER HONOURS THE FLAG.
//
// THIS IS A GO-ONLY HAZARD and it has no Rust twin, which is why it is a test
// rather than a shared one: Go's World answers (Value, bool) and a host fixture
// can hand back a perfectly edit-shaped value beside ok=false, while Rust's
// answers Option<Value> and cannot express the pair at all. A reader that looked
// at the value first would take this text and this number as a player's typing,
// log the lines that name a field to go and fix, and put a note on the recipe;
// what is owed instead is the declared default with the ordinary unreadable
// sentence and nothing else. withSettingButAbsent is the fixture arm that says
// so, and this is what reads it.
func TestASettingAnsweringAValueBesideNotOkIsAbsent(t *testing.T) {
	lib := New()
	axe := lib.Item("steel-axe", ItemSpec{})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	forging := lib.DoubleSetting("forging-time", 3, NumericSpec{HasMax: true, Max: 120})
	lib.Recipe(axe, RecipeSpec{Name: "steel-axe-forging", CraftTimeFrom: forging, IngredientsFrom: parts})

	w := customWorld().
		withSettingButAbsent("steelworks-axe-ingredients", Str("2 iron-stick")).
		withSettingButAbsent("steelworks-forging-time", Num(0))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-forging-time was not readable, so its default applies`,
		`log fkrecipes: the setting steelworks-axe-ingredients was not readable, so its default applies`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", energy_required=3, enabled=true,` +
			` ingredients=[{type="item", name="steel-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
	})
}

// THE NOTE JOINS THE AUTHOR'S OWN DESCRIPTION rather than replacing it, and an
// ITEM NEVER CARRIES ONE.
//
// THREE SHAPES AND THE THIRD ONE IS THE DEFAULT, which is what the other
// transcripts in this file already show: a recipe with no Description of its own
// carries the note alone in the ordinary two-element form. Here the recipe and
// the technology both declare one, so the emitted description is the author's
// sentence and then the library's on a line of its own; and the item beside
// them declares one too and carries it UNCHANGED, because the fallback is about
// what a recipe makes and what a technology costs and an item prototype is
// neither.
func TestAFallbackNoteJoinsTheAuthorsOwnDescription(t *testing.T) {
	lib := New()
	rivet := lib.Item("steel-rivet", ItemSpec{Description: "A small steel rivet."})
	parts := lib.IngredientsSetting("rivet-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	packs := lib.PacksSetting("rivet-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("rivet-count", 20, Between(1, 100000))
	seconds := lib.IntSetting("rivet-seconds", 10, Between(1, 600))
	lib.Recipe(rivet, RecipeSpec{Name: "steel-rivet-forging", Description: "Forged from plate.", IngredientsFrom: parts})
	lib.Technology("riveting", TechSpec{
		Description: "Teaches riveting.",
		CostFrom:    &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})

	w := customWorld().
		withSetting("steelworks-rivet-ingredients", Str("2 unobtanium")).
		withSetting("steelworks-rivet-packs", Str("1 unobtanium")).
		withSetting("steelworks-rivet-count", Num(20)).
		withSetting("steelworks-rivet-seconds", Num(10))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-rivet-ingredients, entry 1 ("2 unobtanium"): no item or fluid is named unobtanium.` +
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.` + recipeFallbackTail,
		`log fkrecipes: ERROR: steelworks-rivet-packs, entry 1 ("1 unobtanium"): no science pack is named unobtanium.` +
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: steelworks-riveting takes its research cost from steelworks-rivet-packs:` +
			` count 20, time 10, packs 1 automation-science-pack`,
		`extend {type="item", name="steelworks-steel-rivet",` +
			` localised_description=["", "A small steel rivet."], stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging",` +
			` localised_description=["", "Forged from plate.", ` +
			chunkedParams("\n"+fallbackNote("steelworks-rivet-ingredients", true)) + `],` +
			` enabled=true, ingredients=[{type="item", name="steel-plate", amount=2}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
		`extend {type="technology", name="steelworks-riveting",` +
			` localised_description=["", "Teaches riveting.", ` +
			chunkedParams("\n"+fallbackNote("steelworks-rivet-packs", false)) + `],` +
			` unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}`,
	})
}

// A RECIPE THE ENVIRONMENT EMPTIED SAYS SO, AND A RECIPE SOMEBODY EMPTIED ON
// PURPOSE SAYS NOTHING, which is one degradation and two negatives.
//
// THE TWO CASES ARE DIFFERENT IN KIND RATHER THAN IN DEGREE. A free craft
// nobody chose is a balance change nobody was told about, so it carries an
// ERROR line and a trailing note exactly as a research the game left with no
// science pack does. A free craft SOMEBODY chose is a declaration or a typed
// word, and both are disclosed where the choice was made: the author's own
// declaration, and ingredientNoneClause on the field the player typed into.
// Writing the note there would be telling a player their own instruction had
// degraded.
//
// THE NEGATIVES ARE THE HALF THAT CAN ROT. A trigger written as "the emitted
// list is empty" passes the positive here and fires on both negatives, so the
// positive alone would not see it.
func TestAnEmptiedRecipeSaysSoAndADeliberateOneDoesNot(t *testing.T) {
	plan := func() (*Lib, IngredientsSettingRef) {
		lib := New()
		plate := lib.Item("hardened-steel-plate", ItemSpec{})
		from := lib.IngredientsSetting("quench-ingredients", []Ingredient{
			IngredientNamed(2, "titanium-plate", "unobtanium"),
		})
		lib.Recipe(plate, RecipeSpec{IngredientsFrom: from})
		return lib, from
	}

	emptied := `log fkrecipes: ERROR: hardened-steel-plate: this game has none of the ingredients this recipe names,` +
		` so it is emitted with no ingredients and costs nothing to craft`
	note := `localised_description=["", ` + descriptionRefIn("recipe", "steelworks-hardened-steel-plate") + `, ` +
		chunkedParams(ingredientlessNote()+` Changing a recipe empties an assembling machine's input slots of anything the new list does not use.`) + `], `

	// THE ENVIRONMENT EMPTIED IT: the declared entry's whole ladder was put to
	// the game and neither rung is there.
	lib, _ := plan()
	ops, err := lib.PlanData(baseWorld())
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-quench-ingredients was not readable, so its default applies`,
		`log fkrecipes: hardened-steel-plate: none of titanium-plate, unobtanium is present, so the ingredient is dropped`,
		emptied,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", ` + note +
			`enabled=true, ingredients=[], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})

	// THE PLAYER EMPTIED IT, by typing the one word that means an empty list.
	// The same recipe, the same world, and neither the line nor the note.
	lib, _ = plan()
	ops, err = lib.PlanData(baseWorld().withSetting("steelworks-quench-ingredients", Str("none")))
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-steel-plate takes its ingredients from steelworks-quench-ingredients: none`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-hardened-steel-plate", enabled=true, ingredients=[], results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})

	// AND THE AUTHOR EMPTIED IT, by declaring no ingredients at all. Nothing
	// was resolved from anything, so nothing was dropped.
	author := New()
	axe := author.Item("steel-axe", ItemSpec{})
	author.Recipe(axe, RecipeSpec{IngredientsFrom: author.IngredientsSetting("axe-ingredients", nil)})
	ops, err = author.PlanData(baseWorld())
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-axe-ingredients was not readable, so its default applies`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe", enabled=true, ingredients=[], results=[{type="item", name="steelworks-steel-axe", amount=1}]}`,
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
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.` + recipeFallbackTail,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", ` +
			noteIn("recipe", "steelworks-steel-rivet-forging", "steelworks-rivet-ingredients", true) +
			`enabled=true,` +
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
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.` + recipeFallbackTail,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", ` +
			noteIn("recipe", "steelworks-steel-rivet-forging", "steelworks-rivet-ingredients", true) +
			`enabled=true,` +
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
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.` + recipeFallbackTail,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", ` +
			noteIn("recipe", "steelworks-steel-rivet-forging", "steelworks-rivet-ingredients", true) +
			`enabled=true,` +
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
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.` + recipeFallbackTail,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", ` +
			noteIn("recipe", "steelworks-steel-axe-forging", "steelworks-axe-ingredients", true) +
			`enabled=true,` +
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
	seconds := lib.IntSetting("axe-seconds", 10, Between(1, 600))
	lib.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})
	w := customWorld().withSetting("steelworks-axe-packs", Str("1 steelworks-steel-rivet"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-axe-count was not readable, so its default applies`,
		`log fkrecipes: the setting steelworks-axe-seconds was not readable, so its default applies`,
		`log fkrecipes: ERROR: steelworks-axe-packs, entry 1 ("1 steelworks-steel-rivet"):` +
			` steelworks-steel-rivet is an item, not a science pack.` +
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: steelworks-steel-axes takes its research cost from steelworks-axe-packs:` +
			` count 20, time 10, packs 1 automation-science-pack`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="technology", name="steelworks-steel-axes", ` +
			noteIn("technology", "steelworks-steel-axes", "steelworks-axe-packs", false) +
			`unit={count=20, time=10, ingredients=[["automation-science-pack", 1]]}}`,
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
// The data stage: a text setting beside a dropdown.
// ---------------------------------------------------------------------------

func textBesideDropdownPlan() *Lib {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
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
		},
		IngredientsFrom: quench,
	})
	return lib
}

// A TEXT LEFT ON THE RESERVED WORD LETS THE DROPDOWN DECIDE, which is exactly
// where a player who never opened the settings screen lands.
func TestTextOnTheWordLetsTheDropdownDecide(t *testing.T) {
	lib := textBesideDropdownPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("oil")).
		withSetting("steelworks-quench-ingredients", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// AND THE LANGUAGE IS WHAT DECIDES WHAT THE WORD IS. "default," is a tolerated
// trailing comma the language reads as the marker, so the dropdown still
// decides and nothing is said. Comparing the trimmed text with the bare word
// would have taken an untouched field for an edit.
func TestTextOnTheWordWithATrailingCommaLetsTheDropdownDecide(t *testing.T) {
	lib := textBesideDropdownPlan()
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

// AN UNREADABLE TEXT IS THE WORD, and the line saying so is the one every
// unreadable setting gets. The dropdown decides, exactly as it does for the
// word itself.
func TestAnUnreadableTextLetsTheDropdownDecide(t *testing.T) {
	lib := textBesideDropdownPlan()
	w := customWorld().withSetting("steelworks-quench-medium", Str("oil"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: the setting steelworks-quench-ingredients was not readable, so its default applies`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// THE TEXT IS THE SWITCH: anything but the word takes the list over, and the
// line says which choice was set aside so a player who forgot the field reads
// why the dropdown did nothing.
func TestTextBesideADropdownTakesTheListOver(t *testing.T) {
	lib := textBesideDropdownPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("oil")).
		withSetting("steelworks-quench-ingredients", Str("3 copper-plate, 2 water"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-plate-quenching takes its ingredients from steelworks-quench-ingredients:` +
			` 3 copper-plate, 2 [fluid=water]; the steelworks-quench-medium choice oil is set aside`,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=3}, {type="fluid", name="water", amount=2}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// A TEXT THE LANGUAGE REFUSES FALLS BACK EXACTLY AS THE WORD BEHAVES: the
// dropdown decides, and the ERROR line names the field the player fixes. The
// clause about a choice being set aside must NOT appear, because none was.
func TestARefusedTextBesideADropdownFallsBackToTheDropdown(t *testing.T) {
	lib := textBesideDropdownPlan()
	w := customWorld().
		withSetting("steelworks-quench-medium", Str("oil")).
		withSetting("steelworks-quench-ingredients", Str("4 unobtanium"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-quench-ingredients, entry 1 ("4 unobtanium"):` +
			` no item or fluid is named unobtanium.` +
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.` + recipeFallbackTail,
		`extend {type="item", name="steelworks-hardened-steel-plate", stack_size=50}`,
		`extend {type="recipe", name="steelworks-plate-quenching", ` +
			noteIn("recipe", "steelworks-plate-quenching", "steelworks-quench-ingredients", true) +
			`category="chemistry", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=1}],` +
			` results=[{type="item", name="steelworks-hardened-steel-plate", amount=1}]}`,
	})
}

// A TEXT WITH NO DROPDOWN BESIDE IT CARRIES NO CLAUSE, which is what says the
// clause is about a choice that really was set aside rather than decoration on
// the line.
func TestTextWithNoDropdownCarriesNoSetAsideClause(t *testing.T) {
	lib, _ := rivetPlan()
	w := customWorld().withSetting("steelworks-rivet-ingredients", Str("3 copper-plate"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-steel-rivet-forging takes its ingredients from steelworks-rivet-ingredients: 3 copper-plate`,
		`extend {type="item", name="steelworks-steel-rivet", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-rivet-forging", enabled=true,` +
			` ingredients=[{type="item", name="copper-plate", amount=3}],` +
			` results=[{type="item", name="steelworks-steel-rivet", amount=1}]}`,
	})
}

// THE WORD custom IS AN ORDINARY DROPDOWN VALUE NOW. The library adds nothing
// to a dropdown's option list and reserves no value in it, so a mod that
// already ships a preset spelled custom keeps it and keeps every stored choice
// with it.
func TestAPresetNamedCustomIsAnOrdinaryPreset(t *testing.T) {
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
func TestACostPresetNamedCustomIsAnOrdinaryPreset(t *testing.T) {
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
	lib := textBesideDropdownPlan()
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
	seconds := lib.IntSetting("chain-seconds", 10, Between(1, 600))
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

// Every declared pack dropping is the same degradation a hand-rolled unit gets:
// a research with no packs is a FREE one, the engine loads it (measured in play
// on 2.0.77), and the player is told so in the log and in the technology's own
// tooltip rather than being locked out of the game.
func TestCustomResearchCostGoesPacklessWhenEveryDeclaredPackDrops(t *testing.T) {
	lib := chainPlan()
	w := customWorld().withoutTool("military-science-pack").withoutTool("automation-science-pack").
		withSetting("steelworks-chain-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)
	assertHasLine(t, transcript(ops),
		packlessLog("chain-forging", "automation-science-pack", "military-science-pack"))
	// AND THE COST LINE DOES NOT SPELL THE EMPTY LIST WITH THE WORD THE FIELD
	// REFUSES. The renderer's answer for an empty list is the language's
	// reserved word; a PACK field will not take that word, and printing it in a
	// line about that very field invites the player to paste back the one text
	// it refuses.
	assertHasLine(t, transcript(ops),
		`log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs: count 20, time 10, packs no science pack`)
	assertHasLine(t, transcript(ops),
		`extend {type="technology", name="steelworks-chain-forging", `+
			`localised_description=["", `+descriptionRefIn("technology", "steelworks-chain-forging")+`, "`+packlessTooltip+`"], `+
			`prerequisites=["steel-processing"], unit={count=20, time=10, ingredients=[]}}`)
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
		name string
		// setting is the field the note on the emitted technology names, which
		// is the FIRST the walk set aside: the count is read before the time,
		// so a case that spoils both would still name the count.
		setting string
		count   Value
		time    Value
		want    string
	}{
		{
			name:    "a count that is not a finite number",
			setting: "steelworks-chain-count",
			count:   Num(nan()),
			time:    Num(10),
			want:    "steelworks-chain-count holds a value that is not a finite number",
		},
		{
			name:    "a time that is not a finite number",
			setting: "steelworks-chain-seconds",
			count:   Num(20),
			time:    Num(math.Inf(1)),
			want:    "steelworks-chain-seconds holds a value that is not a finite number",
		},
		{
			// Finiteness is asked first within each number, so this arm is
			// only ever reached by a real number: a NaN is not below 1 and
			// would have gone through.
			name:    "a count below 1",
			setting: "steelworks-chain-count",
			count:   Num(0),
			time:    Num(10),
			want:    "steelworks-chain-count holds a research count below 1",
		},
		{
			name:    "a time at or below zero",
			setting: "steelworks-chain-seconds",
			count:   Num(20),
			time:    Num(0),
			want:    "steelworks-chain-seconds holds a research time at or below zero",
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
					`. The mod loaded as though that number had been left alone; fix the number under Settings > Mod settings > Startup, then restart.`,
				`log fkrecipes: steelworks-chain-forging takes its research cost from steelworks-chain-packs:` +
					` count 20, time 10, packs 1 automation-science-pack`,
				`extend {type="technology", name="steelworks-chain-forging", ` +
					noteIn("technology", "steelworks-chain-forging", c.setting, false) +
					`prerequisites=["steel-processing"],` +
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
// withFallbackFact is the ONE sentence a refusal raised after resolution
// carries when a stored value fell back on the way to it, composed here so the
// tests that assert it cannot drift apart from each other.
//
// IT IS A FACT AND NAMES NO SCREEN. An earlier round appended a route to
// Settings > Mod settings > Startup and the client cannot reach it from an
// "Error loading mods" dialog; what survives is the half that was true, which
// is that the player's stored value was set aside. See resolution.fallbackFact.
func withFallbackFact(message, setting string) string {
	return message + ". The stored value of " + setting +
		" could not be used and was set aside, so what applied is what that field gives when it is left alone."
}

func TestCustomResearchCostRefusesADeclaredDefaultTheEngineWouldNotTake(t *testing.T) {
	plan := func() *Lib {
		lib := New()
		packs := lib.PacksSetting("chain-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
		// A minimum of 1, which validateCustomCost demands, beside a declared
		// default of 0, which validateSettings refuses at the settings stage.
		count := lib.IntSetting("chain-count", 0, NumericSpec{HasMin: true, Min: 1, HasMax: true, Max: 100000})
		seconds := lib.IntSetting("chain-seconds", 10, Between(1, 600))
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
	assertRefusal(t, err, withFallbackFact(
		"fkrecipes: steelworks-chain-count declares a default research count below 1",
		"steelworks-chain-count"))
}

// TWO DECLARED DEFAULTS WRONG AT ONCE, AND THE COUNT IS THE ONE THAT ANSWERS.
//
// THIS IS THE ONLY WITNESS TO THAT ORDER. refuseCostNumbers walks the count and
// then the seconds and keeps the first answer; every other test declares at
// most one bad default, so swapping its two arms changed no sentence anywhere
// and the order was free to drift between the halves. A world wrong in both
// places is what pins it.
//
// NOTHING IS TYPED HERE: both numbers ARE the declared defaults rather than
// values something fell back onto.
func TestRefusedCostNumbersAnswerTheCountBeforeTheSeconds(t *testing.T) {
	lib := New()
	packs := lib.PacksSetting("chain-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	// Both outside what the engine takes, and both refused by validateSettings
	// at the settings stage: PlanData reaches them only on its own.
	count := lib.IntSetting("chain-count", 0, NumericSpec{HasMin: true, Min: 1, HasMax: true, Max: 100000})
	seconds := lib.IntSetting("chain-seconds", 0, NumericSpec{HasMin: true, Min: 1, HasMax: true, Max: 600})
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

// A research cost with a TIER BESIDE IT: the dropdown chooses a source, and the
// three settings overwrite that source's numbers one field at a time.
func tipsPlan() *Lib {
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-tier", "cheap", []string{"cheap", "formula"})
	packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tips-count", 0, Between(0, 100000))
	seconds := lib.IntSetting("tips-seconds", 0, Between(0, 600))
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{
				{Value: "cheap", Sources: []string{"logistics-2"}},
				{Value: "formula", Sources: []string{"mining-productivity-4"}},
			},
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
		},
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})
	return lib
}

// EVERY FIELD AT ITS DEFAULT LEAVES THE TIER ALONE, byte for byte, which is the
// load a player who never opened the settings screen gets: no cost line, no
// clause, the source's own unit and the source as the prerequisite.
func TestEveryResearchFieldAtItsDefaultLeavesTheTierAlone(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(0)).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// ALL THREE MOVED IS THE WHOLE COST THE PLAYER'S, and the tier still places the
// technology: the source that would have paid for it is the rung it hangs off
// whatever the settings say.
func TestAllThreeResearchFieldsOverrideTheTier(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(40)).
		withSetting("steelworks-tips-seconds", Num(20)).
		withSetting("steelworks-tips-packs", Str("2 logistic-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 40, time 20, packs 2 logistic-science-pack;` +
			` the steelworks-tips-tier choice cheap supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=40, ingredients=[["logistic-science-pack", 2]], time=20}}`,
	})
}

// ONE FIELD MOVED IS THE ONE THE OLD SHAPE COULD NOT EXPRESS. The count is the
// player's and the time and the packs are the tier's, in one unit, and the
// clause says so rather than pretending the whole cost was overridden.
func TestAPartialCustomCostTakesTheRestFromTheTier(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(40)).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 40, time 30, packs 1 automation-science-pack, 1 logistic-science-pack;` +
			` the steelworks-tips-tier choice cheap supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=40, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// THE TIME ALONE IS THE MIRROR OF THE CASE ABOVE, and it is what says the merge
// is per field rather than count-shaped: the count stays the tier's number.
func TestATimeAloneOverridesTheTier(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(0)).
		withSetting("steelworks-tips-seconds", Num(45)).
		withSetting("steelworks-tips-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 200, time 45, packs 1 automation-science-pack, 1 logistic-science-pack;` +
			` the steelworks-tips-tier choice cheap supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=45}}`,
	})
}

// A PACK TEXT ALONE OVERRIDES THE INGREDIENTS AND NOTHING ELSE, so the tier's
// count and time are what the line reports and what the unit carries.
func TestAPackTextAloneOverridesTheTier(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(0)).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("3 automation-science-pack"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 200, time 30, packs 3 automation-science-pack;` +
			` the steelworks-tips-tier choice cheap supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 3]], time=30}}`,
	})
}

// A REFUSED PACK TEXT FALLS BACK EXACTLY AS THE WORD BEHAVES, which beside a
// tier means the TIER'S packs rather than the setting's declared list: the
// fallback lands where a player who typed nothing lands.
func TestARefusedPackTextBesideATierTakesTheTiersPacks(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(40)).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("2 unobtainium"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-tips-packs, entry 1 ("2 unobtainium"): no science pack is named unobtainium.` +
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 40, time 30, packs 1 automation-science-pack, 1 logistic-science-pack;` +
			` the steelworks-tips-tier choice cheap supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips", ` +
			noteIn("technology", "steelworks-hardened-tips", "steelworks-tips-packs", false) +
			`prerequisites=["logistics-2"],` +
			` unit={count=40, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// A NUMBER THE ENGINE WOULD NOT TAKE FALLS BACK TO ITS DECLARED DEFAULT, which
// beside a tier is 0, which means the tier decides. So a stored value the
// library cannot use behaves exactly as an untouched field does, and the only
// thing that changes is the ERROR line.
func TestABadNumberBesideATierLeavesTheTierDeciding(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(math.NaN())).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: ERROR: steelworks-tips-count holds a value that is not a finite number.` +
			` The mod loaded as though that number had been left alone; fix the number under Settings > Mod settings > Startup, then restart.`,
		`extend {type="technology", name="steelworks-hardened-tips", ` +
			noteIn("technology", "steelworks-hardened-tips", "steelworks-tips-count", false) +
			`prerequisites=["logistics-2"],` +
			` unit={count=200, ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1]], time=30}}`,
	})
}

// A COUNT THE PLAYER TYPED REPLACES A count_formula, because the engine prices
// a unit carrying both by the FORMULA and the number would be read by nobody.
// One line says which setting took it, and everything else the tier carried,
// including the key no version of this library knows about and the level cap
// beside the unit, comes through untouched.
func TestACountReplacesTheTiersCountFormula(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("formula")).
		withSetting("steelworks-tips-count", Num(40)).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: hardened-tips: steelworks-tips-count replaces the count_formula the formula cost carries`,
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 40, time 60, packs 1 automation-science-pack, 1 logistic-science-pack, 1 chemical-science-pack;` +
			` the steelworks-tips-tier choice formula supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["mining-productivity-4"],` +
			` unit={ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]],` +
			` mod_cost_tier="mid-game", time=60, count=40}, max_level="infinite"}`,
	})
}

// AND A TIME ALONE LEAVES THE FORMULA WHERE IT IS, which is what says the drop
// is about the count and not about a player having touched anything at all.
//
// THE LINE SAYS `count by formula` RATHER THAN A NUMBER, because there is no
// number: the tier prices itself with count_formula and carries no count key,
// the count setting deferred, and the emitted unit holds neither a count nor
// anything the setting's declared default (0) describes. Printing that 0 would
// be a figure nothing in the prototype is using.
func TestATimeBesideACountFormulaLeavesItAlone(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().
		withSetting("steelworks-tips-tier", Str("formula")).
		withSetting("steelworks-tips-count", Num(0)).
		withSetting("steelworks-tips-seconds", Num(45)).
		withSetting("steelworks-tips-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count by formula, time 45, packs 1 automation-science-pack, 1 logistic-science-pack, 1 chemical-science-pack;` +
			` the steelworks-tips-tier choice formula supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips", prerequisites=["mining-productivity-4"],` +
			` unit={count_formula="2^(L-4)*1000", ingredients=[["automation-science-pack", 1], ["logistic-science-pack", 1], ["chemical-science-pack", 1]],` +
			` mod_cost_tier="mid-game", time=45}, max_level="infinite"}`,
	})
}

// THE FALLBACK COST IS A TIER LIKE ANY OTHER. No source in the chosen ladder
// carries a unit, so the author's own fallback is what the settings write over
// and what they leave alone; the line saying why comes first, because it is the
// reason the numbers under it are the fallback's. The Rust half holds the same
// transcript.
//
// THE TIER ARM'S NOTE STAYS, because the PACK TEXT was left alone: the snapshot
// that takes back everything the tier arm wrote is the typed pack list's, and
// here only a number was typed. That is the same rule the packless and the
// unreadable arms beside it follow.
//
// AND THIS TRANSCRIPT IS WHY ALL THREE OF THOSE SENTENCES STATE AN
// ENVIRONMENTAL FACT AND NAME NO PRICE. The declared Fallback here is
// count 200; the emitted unit is count=45, which is the player's own. A note
// saying this mod's own declared cost applied would be a false sentence in a
// tooltip beside a number the player chose, which is finding 19 one arm over.
// What every one of the three says instead is true whatever the numbers beside
// it are: the copied cost did not price this research, and this one adds that
// there is no prerequisite either, which no setting touches.
func TestASettingOverridesTheFallbackTier(t *testing.T) {
	lib := tipsPlan()
	w := customWorld().withoutTech("logistics-2").
		withSetting("steelworks-tips-tier", Str("cheap")).
		withSetting("steelworks-tips-count", Num(45)).
		withSetting("steelworks-tips-seconds", Num(0)).
		withSetting("steelworks-tips-packs", Str("default"))

	ops, err := lib.PlanData(w)
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`log fkrecipes: hardened-tips: no source for the cheap cost carries a unit,` +
			` so the fallback cost applies and the technology has no prerequisite`,
		`log fkrecipes: steelworks-hardened-tips takes its research cost from steelworks-tips-packs:` +
			` count 45, time 30, packs 1 automation-science-pack;` +
			` the steelworks-tips-tier choice cheap supplies what the settings leave at default`,
		`extend {type="technology", name="steelworks-hardened-tips",` +
			` localised_description=["", ` + descriptionRefIn("technology", "steelworks-hardened-tips") + `, "` + unpricedTooltip + `"],` +
			` unit={count=45, time=30, ingredients=[["automation-science-pack", 1]]}}`,
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
// is checked above; these lines are this library's, so no locale file can put
// one back and no plan can leave one out. What the rule can see is a
// composition that stopped carrying a line, which is why the composition is
// what it is handed.
//
// THE SENTENCE NAMES THE LIBRARY AND NOT A SETTING, because the rule's input
// does not vary with the setting in any way the rule reads: one line is a
// constant, the others are the switch line and the two the KIND decides, the
// ladder line and the format line, all of which the composition was built with
// and which are handed in beside it, and the only per-setting part of a text
// description is the consumer's key, which the rule does not look at. One
// defect is therefore one finding.
//
// THE LADDER FLAG IS THE FOURTH INPUT AND BOTH OF ITS ARMS ARE WALKED HERE.
// A standalone text setting carries the ladder line and the rule looks for it;
// one beside a dropdown does not, and a rule that looked for it anyway would
// report a defect on every plan the customizer was designed for.
//
// THE HEALTHY PATH FIRST, so a rule that fired on everything would be caught
// here rather than in a golden somewhere: the real composition reports
// nothing.
func TestComposedTextLinesMissingGuardsTheComposedLines(t *testing.T) {
	const full = "steelworks-axe-ingredients"
	const switchLine = "\nLeave this as default and this mod's own list applies; anything else applies instead."
	whole := textDescription(full, "1 steel-plate", switchLine, true, true)
	if got := composedTextLinesMissing(whole, switchLine, true, true); len(got) != 0 {
		t.Fatalf("the real composition reported %v", got)
	}
	// AND THE PACKS COMPOSITION IS CLEAN UNDER THE PACKS RULE. The ladder line
	// and the format line are the two lines whose bytes depend on the kind, so
	// a rule asked about the wrong kind reports a healthy description as
	// broken.
	packs := textDescription("steelworks-axe-packs", "1 automation-science-pack", switchLine, false, true)
	if got := composedTextLinesMissing(packs, switchLine, false, true); len(got) != 0 {
		t.Fatalf("the real packs composition reported %v", got)
	}
	// AND THE COMPOSITION BESIDE A DROPDOWN IS CLEAN UNDER THE RULE THAT KNOWS
	// IT HAS NO LADDER LINE. This is the shape the customizer was designed for,
	// so a guard that got this wrong would fire on the commonest plan there is.
	const besideLine = "\nLeave this as default and the option chosen above decides; anything else applies instead."
	beside := textDescription(full, "1 steel-plate", besideLine, true, false)
	if got := composedTextLinesMissing(beside, besideLine, true, false); len(got) != 0 {
		t.Fatalf("the real composition beside a dropdown reported %v", got)
	}
	// AND THE SAME COMPOSITION UNDER THE OTHER ARM IS NOT CLEAN, which is what
	// says the flag is read at all: a rule told to expect a ladder line over a
	// description that correctly has none reports exactly that line.
	assertFindings(t, composedTextLinesMissing(beside, besideLine, true, true), []string{
		"the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})

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
	assertFindings(t, composedTextLinesMissing(without(textLadderLine(true)), switchLine, true, true), []string{
		"the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
	assertFindings(t, composedTextLinesMissing(without(textFormatLine(true)), switchLine, true, true), []string{
		"the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
	assertFindings(t, composedTextLinesMissing(without(switchLine), switchLine, true, true), []string{
		"the library composes no line about which field decides while the text says default onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
	assertFindings(t, composedTextLinesMissing(without(textFallbackLine), switchLine, true, true), []string{
		"the library composes no line about what happens to a text this mod cannot use onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
	// THE LADDER LINE AND THE FORMAT LINE ARE PER KIND, and the rule asked
	// about the wrong kind sees two lines it does not recognise: an ingredient
	// composition read as a packs one is missing the packs ladder line and the
	// packs format line, exactly as if both had been deleted.
	assertFindings(t, composedTextLinesMissing(whole, switchLine, false, true), []string{
		"the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
		"the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
	})
	// All four gone: all four reported, in the order the lines sit in, which is
	// the order every other rule here reports in.
	stripped := Arr(Str(""), localeRef("mod-setting-description", full, full))
	assertFindings(t, composedTextLinesMissing(stripped, switchLine, true, true), []string{
		"the library composes no line about a name in the list this game does not have onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
		"the library composes no line about the format and the length limit onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
		"the library composes no line about which field decides while the text says default onto a text setting's description; a text setting's description carries one, so this is a defect in fkrecipes and not in this locale file",
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
	if _, _, _, _, ok := lib.guardedTextDescription("steelworks-"); ok {
		t.Fatal("a plan with no text setting handed the guard a composition")
	}

	axe := lib.Item("steel-axe", ItemSpec{})
	lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientOf(axe, 1)})
	lib.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	desc, switchLine, ingredients, ladder, ok := lib.guardedTextDescription("steelworks-")
	if !ok {
		t.Fatal("a plan with two text settings handed the guard nothing")
	}
	// The FIRST one's, and with the list left out: the packs setting declared
	// after it is not what the guard reads, and neither is any rendering.
	const own = "\nLeave this as default and this mod's own list applies; anything else applies instead."
	want := renderValue(textDescription("steelworks-axe-ingredients", "", own, true, true))
	if got := renderValue(desc); got != want {
		t.Errorf("\n got: %s\nwant: %s", got, want)
	}
	// AND THE LINE COMES BACK BESIDE IT, because the rule cannot recompute a
	// line that names a neighbouring setting.
	if switchLine != own {
		t.Errorf("the guard was handed the switch line %q, want %q", switchLine, own)
	}
	// AND SO DOES THE KIND, for the same reason: the format line's bytes
	// depend on it and the rule cannot recompute it from the table.
	if !ingredients {
		t.Error("the guard was told the first text setting is not an ingredient list")
	}
	// AND SO DOES THE LADDER FLAG. This plan's text setting has no dropdown
	// beside it, so it carries the ladder line and the rule must look for it.
	if !ladder {
		t.Error("the guard was told a standalone text setting carries no ladder line")
	}

	// AND THE OTHER ARM, off a real plan: a text setting with a dropdown beside
	// it hands the guard a composition with no ladder line in it, and the flag
	// that says so. Deriving that from the switch line's wording instead would
	// be a second spelling of textSwitchDropdown.
	beside := textBesideDropdownPlan()
	desc, switchLine, ingredients, ladder, ok = beside.guardedTextDescription("steelworks-")
	if !ok {
		t.Fatal("a plan with a text setting beside a dropdown handed the guard nothing")
	}
	if ladder {
		t.Error("the guard was told a text setting beside a dropdown carries the ladder line")
	}
	if !ingredients {
		t.Error("the guard was told the text setting beside the dropdown is not an ingredient list")
	}
	if localisedCarries(desc, textLadderLine(true)) {
		t.Errorf("the composition handed to the guard carries the ladder line: %s", renderValue(desc))
	}
	if got := renderValue(desc); got != renderValue(
		textDescription("steelworks-quench-ingredients", "", switchLine, true, false)) {
		t.Errorf("the guard was handed %s", got)
	}
}

// THE RESEARCH NUMBER'S TWO SENTENCES, BYTE FOR BYTE, and they are here
// because nothing else in this half pins the arm beside a dropdown: the
// prototype goldens above carry a research cost with no CostBy, and the arm a
// player meets most often is the other one.
//
// THE SENTENCE BESIDE A DROPDOWN LEADS WITH THE SENTINEL, in textSwitchLine's
// own shape: 0 there is not the bottom of a range a player might pick, it is
// the way this field says "not customised", and a sentence that opened with the
// range said the opposite. Without a dropdown there is no sentinel and the
// sentence is the range alone.
//
// THE NUMBERS COME OUT OF THE LANGUAGE'S AMOUNT FORMATTER, which the corpus
// pins byte for byte across the two halves; the Rust twin carries the same
// bytes and the mirror compares the two transcripts.
func TestTheResearchNumberSentencesAreTheStatedOnes(t *testing.T) {
	// BESIDE A DROPDOWN, which is what CostBy makes this.
	lib := New()
	tier := lib.DropdownSettingNeedingLocale("tips-tier", "cheap", []string{"cheap", "dear"})
	packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tips-count", 0, Between(0, 100000))
	seconds := lib.IntSetting("tips-seconds", 0, Between(0, 600))
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting:  tier,
			Choices:  []CostChoice{{Value: "cheap"}, {Value: "dear"}},
			Fallback: UnitSpec{Count: 200, Seconds: 30, Packs: []Pack{{Name: "automation-science-pack", Amount: 1}}},
		},
		CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds},
	})
	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	want := map[string]string{
		"steelworks-tips-count":   "\nLeave this at 0 and the option chosen above supplies the number; otherwise a whole number up to 100000.",
		"steelworks-tips-seconds": "\nLeave this at 0 and the option chosen above supplies the number; otherwise a whole number up to 600.",
	}
	seen := 0
	for _, op := range ops {
		name, ok := field(op.Proto, "name")
		if !ok {
			continue
		}
		w, wanted := want[name.Str]
		if !wanted {
			continue
		}
		seen++
		desc, ok := field(op.Proto, "localised_description")
		if !ok {
			t.Fatalf("%s carries no composed description", name.Str)
		}
		if got := desc.Arr[2]; got.Kind != KindStr || got.Str != w {
			t.Errorf("%s:\n got: %s\nwant: %q", name.Str, renderValue(got), w)
		}
	}
	if seen != len(want) {
		t.Fatalf("the plan emitted %d of the %d research numbers", seen, len(want))
	}

	// AND WITH NO DROPDOWN THE SENTENCE IS THE RANGE ALONE, both bounds of it,
	// because there is no sentinel to explain and the minimum is a real floor.
	bare := New()
	barePacks := bare.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	bareCount := bare.IntSetting("tips-count", 30, Between(1, 100000))
	bareSeconds := bare.IntSetting("tips-seconds", 15, Between(1, 600))
	bare.Technology("hardened-tips", TechSpec{
		CostFrom: &CustomCost{Packs: barePacks, Count: bareCount, Seconds: bareSeconds},
	})
	ops, err = bare.PlanSettings(settingsWorld())
	assertNoError(t, err)
	want = map[string]string{
		"steelworks-tips-count":   "\nA whole number from 1 to 100000.",
		"steelworks-tips-seconds": "\nA whole number from 1 to 600.",
	}
	seen = 0
	for _, op := range ops {
		name, ok := field(op.Proto, "name")
		if !ok {
			continue
		}
		w, wanted := want[name.Str]
		if !wanted {
			continue
		}
		seen++
		desc, ok := field(op.Proto, "localised_description")
		if !ok {
			t.Fatalf("%s carries no composed description", name.Str)
		}
		if got := desc.Arr[2]; got.Kind != KindStr || got.Str != w {
			t.Errorf("%s:\n got: %s\nwant: %q", name.Str, renderValue(got), w)
		}
	}
	if seen != len(want) {
		t.Fatalf("the plan emitted %d of the %d research numbers", seen, len(want))
	}
}

// THE COMPOSED LINES, BYTE FOR BYTE, and the number in the first one comes from
// the same constant the parser refuses on. A sentence promising a limit the parser
// does not keep is the drift this pins; the Rust twin carries the same bytes
// and the mirror compares the two transcripts.
func TestTheComposedTextLinesAreTheStatedOnes(t *testing.T) {
	if maxListChars != 2000 {
		t.Fatalf("maxListChars is %d; the sentence below and both halves' docs say 2000", maxListChars)
	}
	if got, want := textFormatLine(false),
		"\nInternal names, as on the default line, up to 2000 characters."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	// THE WORD none ON AN INGREDIENT LIST AND NOWHERE ELSE. A packs list
	// refuses it, so the clause above is the whole packs sentence and this is
	// the whole ingredient one.
	if got, want := textFormatLine(true),
		"\nInternal names, as on the default line, up to 2000 characters. The word none empties the list, so the recipe costs nothing to craft."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	if strings.Contains(textFormatLine(false), "none") {
		t.Errorf("a packs setting's format line names the word none: %q", textFormatLine(false))
	}
	if got, want := textFallbackLine,
		"\nText this mod cannot use is set aside as though it said default; the reason is in the log or the load error."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	// BOTH PLACES THE REASON CAN BE, which is the narrow claim this line is
	// allowed to make: on a refused load the accumulated log ops never reach
	// the host, so the load error is the only place it can be, and naming one
	// of the two would be false half the time.
	for _, where := range []string{"log", "load error"} {
		if !strings.Contains(textFallbackLine, where) {
			t.Errorf("the fallback line does not name the %s: %q", where, textFallbackLine)
		}
	}
	// THE LADDER, IN ITS THREE VOCABULARIES, and every one of them carries ALL
	// THREE clauses: the next name the entry offers where there is one, the
	// entry left out where there is not, and two entries landing on one name
	// having their amounts added. A sentence promising only the substitution
	// would be false on every plan whose ladder can run out, which is every
	// plan, because a bare name with no ladder behind it is a ladder of one;
	// and one promising only those two says nothing about a merge, which is a
	// number in no tooltip and in no declaration.
	if got, want := textLadderLine(true),
		"\nWhere a list this mod chose names something your mods do not have, the next name it offers is used instead; an entry it offers nothing for is left out, and two that land on one name have their amounts added, so what you craft can be a shorter list than the one shown."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	if got, want := textLadderLine(false),
		"\nWhere a list this mod chose names a science pack your mods do not have, the next name it offers is used instead; a pack it offers nothing for is left out, and two that land on one pack have their amounts added, so the research can take fewer packs than the list shows."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	if got, want := dropdownLadderLine,
		"\nWhere an option names something your mods do not have, the next name it offers is used instead; an entry it offers nothing for is left out, and two that land on one name have their amounts added, so what you craft can be a shorter list than the one shown."; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
	// AND THE PACKS ARM SAYS "SCIENCE PACK" WHERE THE INGREDIENT ARM SAYS
	// "SOMETHING", because a packs field takes nothing else and a player
	// reading "something" there would be told a wider rule than the field has.
	if strings.Contains(textLadderLine(true), "science pack") {
		t.Errorf("an ingredient setting's ladder line names a science pack: %q", textLadderLine(true))
	}
	if !strings.Contains(textLadderLine(false), "science pack") {
		t.Errorf("a packs setting's ladder line names no science pack: %q", textLadderLine(false))
	}
	// A LINE, NOT A SEPARATOR, and the instruction a player acts on opens it.
	// The dropdown label beside this is the consumer's prose and the client
	// truncates it at about 37 characters; the internal names are on their own
	// line so the copyable half of the tooltip is never the truncated half. The
	// words are "to type" rather than "type", which alone reads as the noun.
	if got, want := ingredientPresetHead, "\n  to type: "; got != want {
		t.Errorf("\n got: %q\nwant: %q", got, want)
	}
}

// WHERE THE LADDER LINE SITS, in both compositions that carry it, and that the
// composition which does not carry it does not.
//
// THE LINE IS ABOUT THE LIST DIRECTLY ABOVE IT, which is what makes its
// placement a property rather than a preference: on a standalone text setting
// that list is the DEFAULT LINE, and on an ingredient dropdown it is the last
// PRESET LINE. And textFormatLine says "as on the default line", which is true
// one line down and would stop being true if the ladder line were moved above
// the default line instead.
//
// THE NEGATIVE ARM IS THE OTHER HALF OF THE SAME RULE. A text setting beside a
// dropdown carries NO ladder line, because the lists a player chooses between
// are that dropdown's presets and dropdownLadderLine discloses the ladder
// there; a composition that carried both would say one thing twice on one
// screen. Every other assertion in this file compares whole transcripts built
// from the same constants and would move with the source, so a reordering or a
// duplication that keeps every line is caught here and nowhere else.
func TestTheLadderLineSitsUnderTheListItIsAbout(t *testing.T) {
	// The STANDALONE TEXT composition: default line, ladder line, format line.
	for _, ingredients := range []bool{true, false} {
		desc := textDescription("steelworks-axe-parts", "1 iron-plate", "\nswitch", ingredients, true)
		want := []string{
			"\ndefault: 1 iron-plate",
			textLadderLine(ingredients),
			textFormatLine(ingredients),
		}
		for i, w := range want {
			if got := desc.Arr[2+i]; got.Kind != KindStr || got.Str != w {
				t.Errorf("ingredients=%v: parameter %d is %s, want %q",
					ingredients, 2+i, renderValue(got), w)
			}
		}
		// BESIDE A DROPDOWN: the format line follows the default line with
		// nothing between them, and the ladder line is nowhere in the table.
		beside := textDescription("steelworks-axe-parts", "1 iron-plate", "\nswitch", ingredients, false)
		if got, w := beside.Arr[3], textFormatLine(ingredients); got.Kind != KindStr || got.Str != w {
			t.Errorf("ingredients=%v: the line under the default line is %s, want the format line %q",
				ingredients, renderValue(got), w)
		}
		if localisedCarries(beside, textLadderLine(ingredients)) {
			t.Errorf("ingredients=%v: a text setting beside a dropdown carries the ladder line: %s",
				ingredients, renderValue(beside))
		}
	}

	// The DROPDOWN composition: the last preset line, the ladder line, the
	// switch line.
	lib := New()
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
	quench := lib.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	lib.Recipe(plate, RecipeSpec{
		IngredientsBy: &IngredientChoices{
			Setting: medium,
			Choices: []IngredientChoice{
				{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
				{Value: "oil", Ingredients: []Ingredient{IngredientNamed(3, "steel-plate")}},
			},
		},
		IngredientsFrom: quench,
	})
	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	desc, ok := field(ops[0].Proto, "localised_description")
	if !ok {
		t.Fatal("the dropdown carries no composed description")
	}
	n := len(desc.Arr)
	// The preset line before the two: a table, not a string, which is what
	// says the ladder line still sits directly under a rendered list.
	if got := desc.Arr[n-3]; got.Kind != KindArr {
		t.Errorf("the parameter above the ladder line is %s, want a preset line", renderValue(got))
	}
	for i, w := range []string{dropdownLadderLine, dropdownSwitchLine("below")} {
		if got := desc.Arr[n-2+i]; got.Kind != KindStr || got.Str != w {
			t.Errorf("trailing parameter %d is %s, want %q", i, renderValue(got), w)
		}
	}
	// AND THE TEXT SETTING BESIDE IT SAYS NOTHING ABOUT A LADDER, which is the
	// same rule read off a real plan rather than off a hand-built composition.
	text, ok := field(ops[1].Proto, "localised_description")
	if !ok {
		t.Fatal("the text setting carries no composed description")
	}
	if localisedCarries(text, textLadderLine(true)) {
		t.Errorf("the text setting beside the dropdown carries the ladder line: %s", renderValue(text))
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

// A dropdown with a text setting beside it has its preset list composed onto
// its description, so an absent one loses the list as well as the tooltip.
func TestCheckLocaleRequiresADropdownDescriptionBesideAText(t *testing.T) {
	lib := textBesideDropdownPlan()

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
		"the dropdown setting steelworks-quench-medium has no [mod-setting-description] entry, and the library composes its preset list onto that entry",
	})
}

// A RESEARCH NUMBER'S DESCRIPTION IS REQUIRED TOO, for the reason a text
// setting's is: the range and what 0 means are composed onto that entry, and an
// absent one loses both.
func TestCheckLocaleRequiresAResearchNumberDescription(t *testing.T) {
	lib := New()
	packs := lib.PacksSetting("axe-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("axe-count", 20, Between(1, 100000))
	seconds := lib.IntSetting("axe-seconds", 10, Between(1, 600))
	lib.Technology("steel-axes", TechSpec{CostFrom: &CustomCost{Packs: packs, Count: count, Seconds: seconds}})

	cfg := `[mod-setting-name]
steelworks-axe-packs=Science packs
steelworks-axe-count=Research count
steelworks-axe-seconds=Research seconds

[mod-setting-description]
steelworks-axe-packs=Amount, then name, commas between.
`
	assertFindings(t, lib.CheckLocale("steelworks", cfg), []string{
		"the setting steelworks-axe-count has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
		"the setting steelworks-axe-seconds has no [mod-setting-description] entry, and a research number needs one to say what the number is for; the library composes the range onto it",
	})

	full := cfg + "steelworks-axe-count=How many units.\nsteelworks-axe-seconds=Seconds per unit.\n"
	assertFindings(t, lib.CheckLocale("steelworks", full), nil)
}

// A BARE INGREDIENT DROPDOWN COMPOSES THE LADDER LINE AND NOTHING ELSE, and
// that one line is what makes its description required and what its finding
// names. Saying "its preset list" here would name a list this dropdown composes
// nothing of: there is no text setting beside it, so there is no language to
// render a preset in.
//
// THE COMPOSITION IS PINNED BESIDE THE FINDING, because a finding about a line
// nothing emits is the defect the two rules are meant to prevent between them.
// It is the consumer's own key and the ladder line, in that order, and no wrap
// line and no switch line: nothing typeable is rendered for a wrap to be about
// and there is no second field to name.
func TestABareIngredientDropdownComposesTheLadderLineAlone(t *testing.T) {
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

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	assertLines(t, transcript(ops)[:1], []string{
		`extend {type="string-setting", name="steelworks-quench-medium", setting_type="startup",` +
			` default_value="water", order="aa", allowed_values=["water", "oil"],` +
			` localised_description=["", ["?", ["mod-setting-description.steelworks-quench-medium"], "steelworks-quench-medium"]` +
			wantDropdownLadder + `]}`,
	})

	cfg := `[mod-setting-name]
steelworks-quench-medium=Quenching medium

[string-mod-setting]
steelworks-quench-medium-water=Water
steelworks-quench-medium-oil=Oil
`
	assertFindings(t, lib.CheckLocale("steelworks", cfg), []string{
		"the dropdown setting steelworks-quench-medium has no [mod-setting-description] entry," +
			" and the library composes onto that entry the line saying what a name this game does not have costs the list",
	})
	full := cfg + "\n[mod-setting-description]\nsteelworks-quench-medium=Which medium.\n"
	assertFindings(t, lib.CheckLocale("steelworks", full), nil)
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
	style := lib.DropdownSettingNeedingLocale("style", "plain", []string{"plain"})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		IngredientsBy: &IngredientChoices{
			Setting: style,
			Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientOf(stranger, 1)}}},
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
// which is the second world below, and the only difference the typing makes is
// one added FACT: the log ops never reach the host on a refused load, so the
// refusal itself is the only place left to say that a stored value was set
// aside. It says that and nothing else, because the screen the old sentence
// routed to cannot be reached from the error dialog. See PlanData.
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

	// THE MODPACK PROBLEM IS A RING IN SOMEBODY ELSE'S TREE, which is the one
	// this library still refuses by name: logistics-2 requires logistics-3 and
	// logistics-3 already requires logistics-2, so the ring holds no edge this
	// plan made and there is nothing of ours to take back. The pack this plan
	// is priced in is present, because a pack the game lacks is a degradation
	// now and would not stop anything.
	ring := func() *fixtureWorld {
		return customWorld().withPrereqs("logistics-2", "logistics", "logistics-3")
	}
	cycle := "fkrecipes: a prerequisite cycle: logistics-2 -> logistics-3 -> logistics-2"

	w := ring().
		withSetting("steelworks-axe-ingredients", Str("2 unobtanium")).
		withSetting("steelworks-forging-time", Num(0))

	_, err := plan().PlanData(w)
	assertRefusal(t, err, withFallbackFact(cycle, "steelworks-forging-time"))

	// AND THE SAME REFUSAL WITH NOTHING TYPED, which is what makes the added
	// fact a fact about THIS player rather than boilerplate: a player who never
	// opened the settings screen meets the identical modpack problem and is
	// told only about the mod. The fact names the crafting time rather than the
	// text because the crafting time is read first; the pair is the walk's
	// order, which is the same every run. Neither sentence sends anybody to a
	// screen the error dialog cannot reach.
	_, err = plan().PlanData(ring())
	assertRefusal(t, err, cycle)

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
			` The mod loaded as though that number had been left alone; fix the number under Settings > Mod settings > Startup, then restart.`,
		`log fkrecipes: ERROR: steelworks-axe-ingredients, entry 1 ("2 unobtanium"): no item or fluid is named unobtanium.` +
			` The mod loaded as though that text had been left alone; fix the text under Settings > Mod settings > Startup, then restart.` + recipeFallbackTail,
		`extend {type="item", name="steelworks-steel-axe", stack_size=50}`,
		`extend {type="recipe", name="steelworks-steel-axe-forging", ` +
			noteIn("recipe", "steelworks-steel-axe-forging", "steelworks-forging-time", false) +
			`energy_required=3, enabled=true,` +
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
			name:  "then the ring in somebody else's tree",
			style: "fancy",
			want:  "fkrecipes: a prerequisite cycle: logistics-2 -> logistics-3 -> logistics-2",
		},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			// The ring is there in both cases, so the second channel is armed
			// throughout and only the one in front of it is repaired.
			w := customWorld().withPrereqs("logistics-2", "logistics", "logistics-3").
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
	style := lib.DropdownSettingNeedingLocale("style", "plain", []string{"plain"})
	parts := lib.IngredientsSetting("axe-ingredients", []Ingredient{IngredientNamed(1, "steel-plate")})
	lib.Recipe(axe, RecipeSpec{
		Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")},
		IngredientsBy: &IngredientChoices{
			Setting: style,
			Choices: []IngredientChoice{{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}}},
		},
		IngredientsFrom: parts,
	})

	cfg := `[mod-setting-name]
steelworks-style=Axe style
steelworks-axe-ingredients=What an axe is made of

[mod-setting-description]
steelworks-axe-ingredients=Amount, then name, commas between.

[string-mod-setting]
steelworks-style-plain=Plain
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
		seconds := lib.IntSetting("axe-seconds", 10, Between(1, 600))
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
		seconds := lib.IntSetting("axe-seconds", 10, Between(1, 600))
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
	seconds := lib.IntSetting("axe-seconds", 10, Between(1, 600))
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
			line := playerFallback(s.text, "text", "")
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
	got := playerFallback("fkrecipes: mymod-parts is not text", "text", "")
	want := "fkrecipes: ERROR: mymod-parts is not text." +
		" The mod loaded as though that text had been left alone;" +
		" fix the text under Settings > Mod settings > Startup, then restart."
	if got != want {
		t.Errorf("\n got: %s\nwant: %s", got, want)
	}
	got = numberFallback("fkrecipes: mymod-count holds a research count below 1")
	want = "fkrecipes: ERROR: mymod-count holds a research count below 1." +
		" The mod loaded as though that number had been left alone;" +
		" fix the number under Settings > Mod settings > Startup, then restart."
	if got != want {
		t.Errorf("\n got: %s\nwant: %s", got, want)
	}
	if textFallback("fkrecipes: mymod-parts is not text") != playerFallback("fkrecipes: mymod-parts is not text", "text", "") {
		t.Error("textFallback and playerFallback disagree about the field word")
	}
}

// A RECIPE'S INGREDIENT TEXT IS THE ONE FALLBACK WITH A TAIL, and the tail is
// the engine's own cost rather than anything this library does. The pack text
// and the two numbers keep the line byte for byte, which is what says the tail
// is chosen by the prototype the setting is bound to and not by the message.
func TestARecipeTextFallbackNamesWhatChangingARecipeCosts(t *testing.T) {
	reason := "fkrecipes: mymod-parts is not text"
	want := textFallback(reason) + " " +
		"Changing a recipe empties an assembling machine's input slots of anything the new list does not use."
	if got := recipeTextFallback(reason); got != want {
		t.Errorf("\n got: %s\nwant: %s", got, want)
	}
	if got := textFallbackFor(noteTarget{index: 0}, reason); got != want {
		t.Errorf("a recipe target did not pick the recipe line:\n got: %s\nwant: %s", got, want)
	}
	if got := textFallbackFor(noteTarget{tech: true, index: 0}, reason); got != textFallback(reason) {
		t.Errorf("a technology target picked up a recipe's tail:\n got: %s", got)
	}
	if strings.Contains(numberFallback(reason), "assembling machine") {
		t.Error("a number fallback carries the recipe tail")
	}
}

// noteIn is the localised_description one fallen-back prototype carries, in the
// shape a transcript shows it, and every transcript below composes it rather
// than retyping the sentence, exactly as the fallback LINES are composed
// through playerFallback. The sentence itself is pinned by TestFallbackNoteShape
// and the line by TestPlayerFallbackLineShape, so a drift in either is one
// failure with the whole text in it rather than thirty.
// THE KIND AND THE EMITTED NAME ARE THE FIRST TWO ARGUMENTS because a note
// with no declared Description opens with descriptionRef's wrapper, which
// carries the prototype's own [<kind>-description] key: the shape is the
// prototype's and not the note's, so a transcript that hard-coded one shape
// would pass a recipe's key onto a technology.
func noteIn(kind, name, setting string, destroysInputs bool) string {
	return `localised_description=["", ` + descriptionRefIn(kind, name) + `, ` +
		chunkedParams(fallbackNote(setting, destroysInputs)) + `], `
}

// descriptionRefIn is descriptionRef's wrapper as a transcript prints it, which
// is one spelling shared by every expectation that carries a note with no
// declared Description.
func descriptionRefIn(kind, name string) string {
	return `["?", ["", ["` + kind + `-description.` + name + `"], "` + "\n" + `"], ""]`
}

// chunkedParams is one sentence as the PARAMETERS appendLocalised splits it
// into, quoted and comma separated the way a transcript prints them.
//
// IT ASKS THE CHUNKER RATHER THAN SPELLING THE CUT, for the reason noteIn gives
// about the sentence itself: a transcript here is pinning WHICH SENTENCE a
// prototype carries, and the split it is carried in is pinned once, with the
// pieces written out by hand, by TestTheChunkerSplitsOnSpacesWithinTheBudget.
// Thirty transcripts carrying a hand-copied cut point would be thirty failures
// the day the budget moves, and none of them would be about what they test.
func chunkedParams(text string) string {
	pieces := chunkLocalised(text)
	out := make([]string, 0, len(pieces))
	for _, p := range pieces {
		out = append(out, `"`+p+`"`)
	}
	return strings.Join(out, ", ")
}

// recipeFallbackTail is what an ingredient text's ERROR line carries past the
// line every other fallback shares.
var recipeFallbackTail = " " + recipeChangeSentence

// THE NOTE IS THE OTHER READER OF THE SAME SENTENCE, and the two shapes are
// written out here so a change to either is a change to this test.
//
// THE TAIL IS SCOPED BY WHAT MOVED, NOT BY THE PROTOTYPE KIND. A recipe whose
// CRAFTING TIME fell back gets the head alone, because its ingredient list is
// byte for byte what it would have been; only a fallback that changes the list
// itself can empty an assembler. See noteOn, and
// TestPlayerFieldsFallBackWhileTheAuthorChannelStillRefuses for the recipe that proves it end to end.
func TestFallbackNoteShape(t *testing.T) {
	head := "The stored value of mymod-parts could not be used," +
		" so the game loaded as though that setting had been left alone. The reason is in the log."
	if got := fallbackNote("mymod-parts", false); got != head {
		t.Errorf("a fallback that moved no ingredient list\n got: %s\nwant: %s", got, head)
	}
	want := head + " Changing a recipe empties an assembling machine's input slots of anything the new list does not use."
	if got := fallbackNote("mymod-parts", true); got != want {
		t.Errorf("a fallback that moved the ingredient list\n got: %s\nwant: %s", got, want)
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

// ---------------------------------------------------------------------------
// The alternatives form: every composed locale reference, and the rule that
// the raw fallback is last.
// ---------------------------------------------------------------------------

// composedSettingSections is every locale section this library composes a key
// under AT THE SETTINGS STAGE. Written out rather than derived, because the
// property being asserted is "these and nothing else go out bare", and a
// section added to the library is an entry somebody has to type here on
// purpose.
var composedSettingSections = []string{
	"mod-setting-description.",
	"string-mod-setting.",
	"technology-name.",
}

// composedPrototypeSections is the DATA stage's two, and they are a separate
// list because the two stages compose disjoint sets and the test asserts that:
// a settings key on a prototype or a prototype key on a setting is a defect
// neither a golden nor a count over one merged list could see.
//
// THEY ARE THE AUTHOR'S OWN OPTIONAL ENTRY. A recipe or a technology carrying a
// note with no declared Description references [recipe-description] or
// [technology-description] under its own emitted name, so an author who wrote
// their description in a .cfg keeps it and the note joins it. See
// descriptionRef.
var composedPrototypeSections = []string{
	"recipe-description.",
	"technology-description.",
}

// composedLocaleSections is both lists, which is what the string classifier
// walks: a key of either kind in the wrong place is still a key.
var composedLocaleSections = append(append([]string{}, composedSettingSections...), composedPrototypeSections...)

// composedLocaleSection is the section a string names, or the empty string
// when it is ordinary prose.
func composedLocaleSection(s string) string {
	for _, sec := range composedLocaleSections {
		if strings.HasPrefix(s, sec) {
			return sec
		}
	}
	return ""
}

// isKeyTable reports whether a value is the one-element table a locale key is
// referenced through: {"section.key"}.
func isKeyTable(v Value) bool {
	return v.Kind == KindArr && len(v.Arr) == 1 && v.Arr[0].Kind == KindStr &&
		composedLocaleSection(v.Arr[0].Str) != ""
}

// localeRefFaults walks one composed value and reports every place the
// alternatives rule is broken, along with what it saw.
//
// THE TWO RULES ARE THE TWO MEASURED FACTS. A key table that is not wrapped is
// a reference that costs the whole tooltip where the game does not define it;
// and inside a wrapper a plain string is always a SUCCESSFUL alternative, so
// one anywhere but the last slot short circuits every alternative after it and
// the key is never consulted, while a wrapper not ending in a plain string
// falls back onto its last alternative's own `Unknown key: "..."` marker. Both
// are silent on a headless run, which is why they are held by a source
// property here rather than by a golden alone.
//
// GUARDED IS THE THIRD THING IT TRACKS, and it is a measured fact rather than a
// loosening. descriptionRef's first alternative is a concatenation GROUP
// holding the key and the newline that follows it, not the bare key table, and
// a group holding an undefined key IS ITSELF A FAILED ALTERNATIVE (measured on
// 2.0.77: the whole group renders empty rather than leaving the newline
// behind). So a key table anywhere under a wrapper's non-last alternative is as
// safe as one sitting directly in that slot, and guarded is what says which
// slots those are.
//
// WHAT GUARDED IS AND IS NOT PROVED BY, recorded because a relaxation that
// LOOKS checked and is not is worse than one that says so. It is load-bearing
// rather than dead: recursing with a flat false instead goes RED here, naming
// the bare key table under each of the two prototype sections. But it is a pure
// WIDENING, and widening it further cannot go red at all: recursing with a flat
// true leaves the whole suite green, because the only thing guarded exempts is
// a key table under a wrapper's non-last alternative, which is the
// measured-safe slot, and no composition this library builds puts one anywhere
// a wider exemption would newly reach. So this clause has no red proof of its
// own and cannot have one. THE DETECTOR AROUND IT DOES, two ways, both taken
// with the guarded clause exactly as it stands: a localeRef returning a bare
// key table goes red naming all three settings sections
// (mod-setting-description., string-mod-setting., technology-name.), and a
// descriptionRef returning one goes red naming both prototype sections
// (recipe-description., technology-description.).
func localeRefFaults(v Value) []string {
	var out []string
	var walk func(Value, bool)
	walk = func(v Value, guarded bool) {
		if v.Kind == KindMap {
			for _, kv := range v.Map {
				walk(kv.Val, false)
			}
			return
		}
		if v.Kind != KindArr {
			return
		}
		wrapper := len(v.Arr) > 0 && v.Arr[0].Kind == KindStr && v.Arr[0].Str == "?"
		if wrapper {
			switch {
			case len(v.Arr) < 3:
				out = append(out, "a wrapper offers fewer than two alternatives: "+renderValue(v))
			default:
				for i := 1; i < len(v.Arr)-1; i++ {
					if v.Arr[i].Kind != KindArr {
						out = append(out, "a raw fallback sits before the last alternative, "+
							"which short circuits every alternative after it: "+renderValue(v))
						break
					}
				}
				if last := v.Arr[len(v.Arr)-1]; last.Kind != KindStr {
					out = append(out, "a wrapper does not end in a raw fallback, "+
						"so a game defining none of its keys renders the last one's Unknown key marker: "+renderValue(v))
				}
			}
		}
		for i, item := range v.Arr {
			if item.Kind == KindStr {
				if composedLocaleSection(item.Str) != "" && !(i == 0 && len(v.Arr) == 1) {
					out = append(out, "the locale key "+item.Str+" is not referenced through a key table: "+renderValue(v))
				}
				continue
			}
			alternative := wrapper && i >= 1 && i < len(v.Arr)-1
			if isKeyTable(item) && !alternative && !guarded {
				out = append(out, "the composed reference "+renderValue(item)+
					" is a bare key rather than the alternatives form: "+renderValue(v))
			}
			walk(item, alternative || (guarded && !wrapper))
		}
	}
	walk(v, false)
	return out
}

// localisedDescriptions counts the localised_description fields in a value,
// which is what turns "no bare key was found" into "descriptions were composed
// and no bare key was in them". Without it the data-stage half of the wrapper
// test below is green over a stage that stopped composing descriptions at all.
func localisedDescriptions(v Value) int {
	n := 0
	switch v.Kind {
	case KindMap:
		for _, kv := range v.Map {
			if kv.Key == "localised_description" {
				n++
			}
			n += localisedDescriptions(kv.Val)
		}
	case KindArr:
		for _, item := range v.Arr {
			n += localisedDescriptions(item)
		}
	}
	return n
}

// localeRefCounts is how many references each section contributed, so a walk
// that found nothing cannot read as a walk that found nothing wrong.
func localeRefCounts(v Value, into map[string]int) {
	if v.Kind == KindMap {
		for _, kv := range v.Map {
			localeRefCounts(kv.Val, into)
		}
		return
	}
	if v.Kind != KindArr {
		return
	}
	if isKeyTable(v) {
		into[composedLocaleSection(v.Arr[0].Str)]++
	}
	for _, item := range v.Arr {
		localeRefCounts(item, into)
	}
}

// EVERY COMPOSED LOCALE REFERENCE GOES THROUGH THE ALTERNATIVES FORM, and the
// raw fallback is LAST in each one.
//
// THIS IS THE PROPERTY A GOLDEN CANNOT HOLD. A golden pins the shapes somebody
// thought to write down; this walks everything both stages emit for a plan that
// exercises all three key shapes and asserts the rule over each. It is also the
// only reader of the rule that can see a REORDERED wrapper: `["?", "raw",
// {key}]` renders identically to `["?", {key}, "raw"}` in every dump and in
// every transcript, because the engine's own dump holds the table verbatim, and
// differs only on a client, where the raw string wins every time and the key is
// never consulted.
//
// BOTH STAGES, because the data stage composes descriptions too. What it must
// carry is the OPPOSITE property: decision C's trailing notes are English
// literals with no key at all, and the counts below say so.
// everyComposedShape is a plan that reaches ALL THREE composed key shapes at the
// settings stage and carries a literal description at the data stage, which is
// exactly what TestEveryComposedLocaleReferenceIsWrapped walks.
//
// IT IS THE SAME FIXTURE AS THE RUST HALF'S every_composed_shape, declaration
// for declaration, and the two walks run it against the same two worlds. They
// were different fixtures until the round that added the description-count
// witness, and the difference hid the same defect twice: neither plan composed a
// single data-stage description, so both halves asserted a count of zero over
// nothing at all.
func everyComposedShape() *Lib {
	lib := New()
	plate := lib.Item("hardened-steel-plate", ItemSpec{})
	medium := lib.DropdownSettingNeedingLocale("quench-medium", "water", []string{"water", "oil"})
	quench := lib.IngredientsSetting("quench-ingredients", []Ingredient{IngredientNamed(2, "steel-plate")})
	tier := lib.DropdownSettingNeedingLocale("tips-research-tier", "projectile", []string{"projectile"})
	packs := lib.PacksSetting("tips-packs", []Pack{{Name: "automation-science-pack", Amount: 1}})
	count := lib.IntSetting("tips-count", 0, Between(0, 100000))
	seconds := lib.IntSetting("tips-seconds", 0, Between(0, 600))
	lib.Recipe(plate, RecipeSpec{
		IngredientsBy: &IngredientChoices{
			Setting: medium,
			Choices: []IngredientChoice{
				{Value: "water", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
				{Value: "oil", Ingredients: []Ingredient{IngredientNamed(3, "steel-plate")}},
			},
		},
		IngredientsFrom: quench,
		// A LITERAL DESCRIPTION, because the data stage's half of the wrapper
		// test needs something to walk: without one this plan composes no
		// localised_description at all and every assertion over it is vacuous.
		// It is also the shape decision C fixed, plain English with no key,
		// which is what the zero counts assert.
		Description: "Quenched in whatever the medium setting says.",
	})
	lib.Technology("hardened-tips", TechSpec{
		CostBy: &CostChoices{
			Setting: tier,
			Choices: []CostChoice{{Value: "projectile", Sources: []string{"logistics-2"}}},
			Fallback: UnitSpec{
				Count: 200, Seconds: 30,
				Packs: []Pack{{Name: "automation-science-pack", Amount: 1}},
			},
		},
		CostFrom:    &CustomCost{Packs: packs, Count: count, Seconds: seconds},
		Description: "Priced from the tier the research setting says.",
	})
	// THE TWO PROTOTYPE KEY SHAPES, which ONLY a note with NO declared
	// Description composes. One of each kind, because the section is the
	// PROTOTYPE'S OWN and a fixture carrying one of them proves nothing about
	// the other.
	//
	// The recipe's note comes from a stored text the language refuses (the data
	// half's world supplies it); the technology's from a science pack the
	// fixture game does not have, which needs no stored value at all.
	rivet := lib.Item("steel-rivet", ItemSpec{})
	parts := lib.IngredientsSetting("rivet-ingredients", []Ingredient{IngredientNamed(1, "iron-plate")})
	lib.Recipe(rivet, RecipeSpec{Name: "steel-rivet-forging", IngredientsFrom: parts})
	lib.Technology("steel-riveting", TechSpec{
		Unit: &UnitSpec{Count: 10, Seconds: 15, Packs: []Pack{{Name: "space-science-pack", Amount: 1}}},
	})
	return lib
}

func TestEveryComposedLocaleReferenceIsWrapped(t *testing.T) {
	lib := everyComposedShape()

	settingOps, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	counts := map[string]int{}
	for _, op := range settingOps {
		for _, f := range localeRefFaults(op.Proto) {
			t.Errorf("settings: %s", f)
		}
		for _, f := range localeRefFaults(op.Val) {
			t.Errorf("settings: %s", f)
		}
		localeRefCounts(op.Proto, counts)
		localeRefCounts(op.Val, counts)
	}
	for _, sec := range composedSettingSections {
		if counts[sec] == 0 {
			t.Errorf("the walk saw no %s reference, so it proves nothing about one", sec)
		}
	}
	// AND THE SETTINGS STAGE COMPOSES NONE OF THE PROTOTYPE'S TWO. The two
	// stages reference disjoint sets of sections, and a merged list would let
	// one stand in for the other in either direction.
	for _, sec := range composedPrototypeSections {
		if counts[sec] != 0 {
			t.Errorf("the settings stage composed %d %s references; that section belongs to a prototype", counts[sec], sec)
		}
	}

	dataOps, err := everyComposedShape().PlanData(
		baseWorld().withSetting("steelworks-rivet-ingredients", Str("1 unobtanium")))
	assertNoError(t, err)
	dataCounts := map[string]int{}
	described := 0
	for _, op := range dataOps {
		for _, f := range localeRefFaults(op.Proto) {
			t.Errorf("data: %s", f)
		}
		for _, f := range localeRefFaults(op.Val) {
			t.Errorf("data: %s", f)
		}
		localeRefCounts(op.Proto, dataCounts)
		localeRefCounts(op.Val, dataCounts)
		described += localisedDescriptions(op.Proto) + localisedDescriptions(op.Val)
	}
	// THE ZERO BELOW IS ONLY WORTH SOMETHING IF THERE WAS SOMETHING TO FIND. A
	// stage that stopped composing descriptions altogether would satisfy every
	// assertion in this half, so the walk says how many it saw first.
	if described == 0 {
		t.Errorf("the data stage composed no localised_description, so the counts below prove nothing")
	}
	// THE NOTE ITSELF IS STILL AN ENGLISH LITERAL, which is the measurement
	// that has not changed: an undefined key in a prototype's composition
	// deletes the WHOLE description on the client, silently, so the library
	// composes no key it would have to ask the consumer to define. The two it
	// does compose are the AUTHOR'S OWN OPTIONAL entry, behind an empty
	// alternative, which is why they cost nothing where nobody wrote one.
	for _, sec := range composedSettingSections {
		if dataCounts[sec] != 0 {
			t.Errorf("the data stage composed %d %s references; that section belongs to a setting", dataCounts[sec], sec)
		}
	}
	for _, sec := range composedPrototypeSections {
		if dataCounts[sec] == 0 {
			t.Errorf("the walk saw no %s reference, so it proves nothing about one", sec)
		}
	}
}
