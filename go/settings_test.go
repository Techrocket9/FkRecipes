package fkrecipes

import (
	"math"
	"slices"
	"strconv"
	"strings"
	"sync"
	"testing"
)

func field(v Value, key string) (Value, bool) {
	for _, p := range v.Map {
		if p.Key == key {
			return p.Val, true
		}
	}
	return Nil(), false
}

func TestPlanSettingsPrototypes(t *testing.T) {
	lib := New()
	lib.BoolSetting("hardened-tools", true)
	lib.IntSetting("axe-durability", 250, Between(50, 1000))
	lib.DoubleSetting("axe-craft-time", 2.5, NumericSpec{HasMin: true, Min: 0.5})
	lib.DropdownSettingNeedingLocale("smelting-style", "furnace", []string{"furnace", "foundry"})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="bool-setting", name="steelworks-hardened-tools", setting_type="startup", default_value=true, order="aa"}`,
		`extend {type="int-setting", name="steelworks-axe-durability", setting_type="startup", default_value=250, order="ab", minimum_value=50, maximum_value=1000}`,
		`extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=2.5000000000000000e0, order="ac", minimum_value=5.0000000000000000e-1}`,
		`extend {type="string-setting", name="steelworks-smelting-style", setting_type="startup", default_value="furnace", order="ad", allowed_values=["furnace", "foundry"]}`,
	})
}

// The order strings are what puts the settings screen in the order the
// consumer wrote them, so the second letter has to roll over into the first.
func TestPlanSettingsOrderStringsRollOver(t *testing.T) {
	lib := New()
	for i := 0; i < 28; i++ {
		lib.BoolSetting("toggle-"+strconv.Itoa(i), false)
	}
	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	for _, want := range []struct {
		at    int
		order string
	}{{0, "aa"}, {25, "az"}, {26, "ba"}, {27, "bb"}} {
		got, ok := field(ops[want.at].Proto, "order")
		if !ok || got.Str != want.order {
			t.Errorf("setting %d has order %q, want %q", want.at, got.Str, want.order)
		}
	}
}

func TestPlanSettingsRefusals(t *testing.T) {
	cases := []struct {
		name  string
		build func(*Lib)
		want  string
	}{
		{
			name: "two settings share a name",
			build: func(l *Lib) {
				l.BoolSetting("hardened-tools", true)
				l.IntSetting("hardened-tools", 3, NumericSpec{})
			},
			want: "fkrecipes: two settings share the name steelworks-hardened-tools; the engine keeps the last one silently",
		},
		{
			name: "a setting with an empty name",
			build: func(l *Lib) {
				l.BoolSetting("", true)
			},
			want: "fkrecipes: a setting was declared with an empty name",
		},
		{
			name: "dropdown default is not an allowed value",
			build: func(l *Lib) {
				l.DropdownSettingNeedingLocale("smelting-style", "electric-furnace", []string{"furnace", "foundry"})
			},
			want: "fkrecipes: the dropdown setting smelting-style defaults to electric-furnace, which is not one of its allowed values",
		},
		{
			name: "minimum above maximum",
			build: func(l *Lib) {
				l.IntSetting("axe-durability", 250, Between(1000, 50))
			},
			want: "fkrecipes: the numeric setting axe-durability declares a minimum above its maximum",
		},
		{
			name: "default outside the bounds",
			build: func(l *Lib) {
				l.DoubleSetting("axe-craft-time", 12, Between(0.5, 8))
			},
			want: "fkrecipes: the numeric setting axe-craft-time declares a default outside its own minimum and maximum",
		},
		{
			name: "a default that is not a number",
			build: func(l *Lib) {
				l.DoubleSetting("axe-craft-time", math.NaN(), NumericSpec{})
			},
			want: "fkrecipes: the numeric setting axe-craft-time declares a value that is not a finite number",
		},
		{
			// The declared int64 is the one number the plan converts to a
			// double on the way in, so it is the one the entry point has to
			// check while it is still an integer.
			name: "an int default past what a double holds",
			build: func(l *Lib) {
				l.IntSetting("axe-durability", 9007199254740993, NumericSpec{})
			},
			want: "fkrecipes: the numeric setting axe-durability declares a default a Lua double cannot hold exactly: 9007199254740993",
		},
		{
			name: "an int default past what a double holds, negative",
			build: func(l *Lib) {
				l.IntSetting("axe-durability", -9007199254740993, NumericSpec{})
			},
			want: "fkrecipes: the numeric setting axe-durability declares a default a Lua double cannot hold exactly: -9007199254740993",
		},
		{
			// The auto-minimum is a real bound: a default below it is refused
			// exactly as it would be below a declared one.
			name: "a default below the generated craft-time minimum",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 0.0001, NumericSpec{})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			want: "fkrecipes: the setting axe-craft-time backs a crafting time, so its minimum is 0.002, which is above the declared default",
		},
		{
			// The consumer declared only a maximum, so a refusal blaming a
			// declared minimum would send them looking for a line they never
			// wrote.
			name: "a declared maximum below the generated craft-time minimum",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 0.0015, NumericSpec{HasMax: true, Max: 0.0015})
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			want: "fkrecipes: the setting axe-craft-time backs a crafting time, so its minimum is 0.002, which is above the declared maximum",
		},
		{
			name: "an explicit minimum at the engine floor on a craft-time setting",
			build: func(l *Lib) {
				axe := l.Item("steel-axe", ItemSpec{})
				from := l.DoubleSetting("axe-craft-time", 2.5, Between(0.001, 60))
				l.Recipe(axe, RecipeSpec{CraftTimeFrom: from})
			},
			want: "fkrecipes: the setting axe-craft-time backs a crafting time but declares a minimum at or below the engine floor (energy_required can't be <= 0.001)",
		},
		{
			name: "a bound that is not a number",
			build: func(l *Lib) {
				l.DoubleSetting("axe-craft-time", 2.5, NumericSpec{HasMax: true, Max: math.Inf(1)})
			},
			want: "fkrecipes: the numeric setting axe-craft-time declares a value that is not a finite number",
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

// The prefix comes from the packaged mod, and there is no prefix parameter to
// fall back on: with no mod name there is nothing safe to emit.
func TestPlanSettingsRefusesAnEmptyModName(t *testing.T) {
	lib := New()
	lib.BoolSetting("hardened-tools", true)

	ops, err := lib.PlanSettings(settingsWorld().withModName(""))
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

// A Lib that never went through New carries no id, and every such plan would
// carry the SAME one, so its handles cannot be told apart from another
// plan's. Both entry points refuse rather than validate against an identity
// nothing owns.
func TestPlanningRefusesALibBuiltWithoutNew(t *testing.T) {
	var settingsPlan Lib
	settingsPlan.BoolSetting("hardened-tools", true)

	ops, err := settingsPlan.PlanSettings(settingsWorld())
	if err == nil {
		t.Fatal("the settings plan was accepted from a Lib with no id")
	}
	want := "fkrecipes: this Lib was built without New, so its handles cannot be validated"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}

	var dataPlan Lib
	dataPlan.Item("steel-axe", ItemSpec{})

	ops, err = dataPlan.PlanData(baseWorld())
	if err == nil {
		t.Fatal("the data plan was accepted from a Lib with no id")
	}
	want = "fkrecipes: this Lib was built without New, so its handles cannot be validated"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
	if ops != nil {
		t.Errorf("a refused plan still produced %d ops", len(ops))
	}
}

// The id is what tells two plans apart, so it is never zero and it increases.
// This is the sequential half of the property; the concurrent half, which is
// where a plain increment loses ids, is below.
func TestNewGivesEveryPlanItsOwnID(t *testing.T) {
	first := New()
	second := New()
	if first.id == 0 || second.id == 0 {
		t.Fatalf("New handed out a zero id: %d then %d", first.id, second.id)
	}
	if second.id <= first.id {
		t.Errorf("ids are not increasing: %d then %d", first.id, second.id)
	}
}

// The two stages have to agree on a setting's name to the byte. They derive
// it from the same World, so this asserts the whole round trip: the name the
// settings stage creates is the name the data stage finds, and finding it is
// visible as the technology being hidden with no degradation log.
func TestSettingsAndDataAgreeOnTheSettingName(t *testing.T) {
	lib := New()
	on := lib.BoolSetting("hardened-tools", true)
	lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", EnabledBy: on})

	settingsOps, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	declared, ok := field(settingsOps[0].Proto, "name")
	if !ok {
		t.Fatalf("the setting prototype carries no name: %s", renderValue(settingsOps[0].Proto))
	}

	// The game now holds exactly the setting the settings stage declared,
	// switched off by the player.
	dataOps, err := lib.PlanData(baseWorld().withSetting(declared.Str, Bool(false)))
	assertNoError(t, err)

	assertLines(t, transcript(dataOps), []string{
		`extend {type="technology", name="steelworks-steel-axes", unit=` + steelProcessingUnit + `, enabled=false, hidden=true}`,
	})
}

// The composition, one refusal per validator family: each case runs a real
// plan and holds the SENTENCE THAT COMES OUT to the rule, so what is proven
// here is that a refusal composes into something the host can prefix without
// saying the stage twice. Emit hands that message to fkdata.Raise, whose host
// side prefixes "fklua: at the <stage> stage, " before it.
//
// THESE SIX ARE A SAMPLE, NOT A SWEEP, and the difference matters: this
// package builds some eighty message chunks and six plans cannot reach them
// all. TestNoMessageCarriesItsOwnStage in source_test.go is the sweep, over
// every string literal in the package; a stage put back into a template these
// six never touch is caught there and nowhere else. Neither replaces the
// other: the property cannot tell whether a message composes correctly, and
// these cannot tell whether every template obeys.
func TestRefusalsComposeWithoutTheirOwnStage(t *testing.T) {
	cases := []struct {
		name string
		plan func() (*Lib, World)
	}{
		{"a settings-stage refusal", func() (*Lib, World) {
			lib := New()
			lib.BoolSetting("hardened-tools", true)
			lib.BoolSetting("hardened-tools", false)
			return lib, settingsWorld()
		}},
		{"a declaration refusal", func() (*Lib, World) {
			lib := New()
			lib.Item("steel-axe", ItemSpec{})
			lib.Item("steel-axe", ItemSpec{})
			return lib, baseWorld()
		}},
		{"a world-probe refusal", func() (*Lib, World) {
			lib := New()
			lib.Technology("steel-axes", TechSpec{CostOf: "quarry-drills"})
			return lib, baseWorld()
		}},
		{"a resolved-value refusal", func() (*Lib, World) {
			// A stored dropdown value the setting does not offer. The engine
			// resets one before any stage runs (measured), so this is a
			// hand-edited file, and it is one of the refusals a resolution
			// still carries out now that a player's typed text and numbers
			// fall back instead.
			lib := New()
			axe := lib.Item("steel-axe", ItemSpec{})
			style := lib.DropdownSettingNeedingLocale("axe-style", "plain", []string{"plain", "fancy"})
			lib.Recipe(axe, RecipeSpec{IngredientsBy: &IngredientChoices{
				Setting: style,
				Choices: []IngredientChoice{
					{Value: "plain", Ingredients: []Ingredient{IngredientNamed(1, "steel-plate")}},
					{Value: "fancy", Ingredients: []Ingredient{IngredientNamed(2, "steel-plate")}},
				},
			}})
			return lib, baseWorld().withSetting("steelworks-axe-style", Str("gilded"))
		}},
		{"a cycle refusal", func() (*Lib, World) {
			lib := New()
			lib.Technology("steel-axes", TechSpec{CostOf: "steel-processing", After: "steel-processing"})
			return lib, baseWorld().withPrereqs("logistics-2", "logistics", "logistics-3")
		}},
		{"the empty mod name", func() (*Lib, World) {
			lib := New()
			return lib, baseWorld().withModName("")
		}},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			lib, w := c.plan()
			_, err := lib.PlanData(w)
			if c.name == "a settings-stage refusal" {
				_, err = lib.PlanSettings(w)
			}
			if err == nil {
				t.Fatal("the plan was accepted, want a refusal")
			}
			got := err.Error()
			if !strings.HasPrefix(got, "fkrecipes: ") {
				t.Errorf("a refusal does not open with the library attribution: %s", got)
			}
			if strings.Contains(got, " stage,") {
				t.Errorf("a refusal names a stage the host will name again: %s", got)
			}
		})
	}
}

// The counter is atomic, and this is what says so. A plain increment is a
// read, an add and a write, so two goroutines calling New at once can come
// away with the SAME id, and two plans with one id resolve each other's
// handles in silence. A consumer's own `go test` is parallel by default.
func TestNewGivesDistinctIDsUnderConcurrency(t *testing.T) {
	const workers, each = 8, 500
	ids := make([]uint64, workers*each)

	var wg sync.WaitGroup
	for w := 0; w < workers; w++ {
		wg.Add(1)
		go func(w int) {
			defer wg.Done()
			for i := 0; i < each; i++ {
				ids[w*each+i] = New().id
			}
		}(w)
	}
	wg.Wait()

	slices.Sort(ids)
	if ids[0] == 0 {
		t.Fatal("a plan came out with id 0")
	}
	for i := 1; i < len(ids); i++ {
		if ids[i] == ids[i-1] {
			t.Fatalf("two plans share id %d", ids[i])
		}
	}
}

// The dropdown's allowed values are the consumer's slice; the plan takes a
// copy, so editing it afterwards cannot change what a second plan emits.
func TestDropdownValuesDoNotAliasTheCallerSlice(t *testing.T) {
	values := []string{"furnace", "foundry"}
	lib := New()
	lib.DropdownSettingNeedingLocale("smelting-style", "furnace", values)

	first, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	values[1] = "electric-furnace"

	second, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(second), transcript(first))
	assertLines(t, transcript(second), []string{
		`extend {type="string-setting", name="steelworks-smelting-style", setting_type="startup", default_value="furnace", order="aa", allowed_values=["furnace", "foundry"]}`,
	})
}

// The boundary itself is exact, so it is accepted and comes back unchanged.
func TestIntSettingAcceptsTheExactBoundary(t *testing.T) {
	lib := New()
	lib.IntSetting("axe-durability", 9007199254740992, NumericSpec{})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="int-setting", name="steelworks-axe-durability", setting_type="startup", default_value=9007199254740992, order="aa"}`,
	})
}

// A double setting nothing binds keeps the bounds the consumer gave it, so
// the generated minimum is not imposed on every double in the plan.
func TestAnUnboundDoubleSettingKeepsItsOwnBounds(t *testing.T) {
	lib := New()
	lib.DoubleSetting("axe-craft-time", 2.5, NumericSpec{})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)

	assertLines(t, transcript(ops), []string{
		`extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=2.5000000000000000e0, order="aa"}`,
	})
}

// The marking scan follows only handles THIS plan issued. A handle from
// another plan lands on a live index here, and following it would bind a
// setting the consumer never bound: this plan's setting would pick up a
// generated minimum above the maximum it declares, and a clean settings stage
// would start refusing.
func TestAForeignCraftTimeHandleMarksNothing(t *testing.T) {
	other := New()
	stray := other.DoubleSetting("other-craft-time", 2.5, NumericSpec{})

	lib := New()
	// Index 1 in this plan too, and a maximum the generated minimum of 0.002
	// would exceed.
	lib.DoubleSetting("axe-craft-time", 0.001, NumericSpec{HasMax: true, Max: 0.0015})
	axe := lib.Item("steel-axe", ItemSpec{})
	lib.Recipe(axe, RecipeSpec{CraftTimeFrom: stray})

	ops, err := lib.PlanSettings(settingsWorld())
	assertNoError(t, err)
	assertLines(t, transcript(ops), []string{
		`extend {type="double-setting", name="steelworks-axe-craft-time", setting_type="startup", default_value=1.0000000000000000e-3, order="aa", maximum_value=1.5000000000000000e-3}`,
	})

	// The same handle is refused by name at the data stage, which is where a
	// reference to another plan gets answered.
	_, err = lib.PlanData(baseWorld())
	if err == nil {
		t.Fatal("the data plan was accepted with a handle from another plan")
	}
	want := "fkrecipes: the recipe steel-axe names a crafting-time setting that this plan never declared"
	if err.Error() != want {
		t.Errorf("\n got: %s\nwant: %s", err.Error(), want)
	}
}
